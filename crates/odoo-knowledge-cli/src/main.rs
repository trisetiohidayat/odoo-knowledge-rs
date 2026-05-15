use std::collections::{HashMap, VecDeque};
use std::io::{self, BufRead};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::DefaultBodyLimit;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use clap::{Parser, Subcommand};
use odoo_knowledge_core::codebase::{add_codebase, list_codebases};
use odoo_knowledge_core::indexer::index_codebase;
use odoo_knowledge_core::search::search;
use odoo_knowledge_core::services;
use odoo_knowledge_core::storage::open_database;
use odoo_knowledge_core::{AppConfig, Result};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

#[derive(Debug, Parser)]
#[command(name = "odoo-knowledge")]
#[command(about = "Odoo-aware codebase index and MCP tooling")]
struct Cli {
    #[arg(long, env = "ODOO_KNOWLEDGE_CONFIG")]
    config: Option<PathBuf>,

    #[arg(long, env = "ODOO_KNOWLEDGE_DB")]
    db: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    AddCodebase {
        #[arg(long)]
        name: String,

        #[arg(long)]
        path: PathBuf,
    },
    ListCodebases,
    Index {
        #[arg(long)]
        codebase: String,
    },
    Validate {
        #[arg(long)]
        codebase: Option<String>,
    },
    MaterializeContexts {
        #[arg(long)]
        codebase: Option<String>,
    },
    ValidateMaterialized {
        #[arg(long)]
        codebase: Option<String>,

        #[arg(long, default_value_t = 25)]
        limit: usize,
    },
    Search {
        query: String,

        #[arg(long)]
        codebase: Option<String>,

        #[arg(long)]
        module: Option<String>,

        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Tool {
        name: String,
        arguments: Option<String>,
    },
    Mcp,
    Serve,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "odoo_knowledge=info,tower_http=info".to_string()),
        )
        .with_writer(std::io::stderr)
        .try_init()
        .ok();
    if let Err(err) = run().await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    let mut config = AppConfig::load(cli.config.as_deref())?;
    if let Some(db) = cli.db {
        config.database_path = db;
    }
    let con = open_database(&config.database_path)?;

    match cli.command {
        Command::AddCodebase { name, path } => {
            let id = add_codebase(&con, &name, &path)?;
            let payload = serde_json::json!({
                "codebase_id": id,
                "name": name,
                "path": path.canonicalize()?.to_string_lossy(),
            });
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        Command::ListCodebases => {
            let codebases = list_codebases(&con)?;
            println!("{}", serde_json::to_string_pretty(&codebases)?);
        }
        Command::Index { codebase } => {
            let stats = index_codebase(&con, &codebase)?;
            let payload = serde_json::json!({ "codebase": codebase, "stats": stats });
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        Command::Validate { codebase } => {
            let mut sql = String::from(
                r#"
                SELECT severity, kind, message, file_path, line_start
                FROM index_diagnostics
                "#,
            );
            let rows = if let Some(codebase) = codebase.as_deref() {
                sql.push_str(
                    r#"
                    WHERE codebase_id = (SELECT id FROM codebases WHERE name = ?1)
                    ORDER BY severity, kind
                    LIMIT 500
                    "#,
                );
                diagnostics_rows(&con, &sql, Some(codebase))?
            } else {
                sql.push_str(" ORDER BY severity, kind LIMIT 500");
                diagnostics_rows(&con, &sql, None)?
            };
            let payload = serde_json::json!({
                "codebase": codebase,
                "diagnostics": rows,
            });
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        Command::MaterializeContexts { codebase } => {
            let stats = services::materialize_contexts(&con, codebase.as_deref())?;
            println!("{}", serde_json::to_string_pretty(&stats)?);
        }
        Command::ValidateMaterialized { codebase, limit } => {
            let validation =
                services::validate_materialized_contexts(&con, codebase.as_deref(), limit)?;
            println!("{}", serde_json::to_string_pretty(&validation)?);
        }
        Command::Search {
            query,
            codebase,
            module,
            limit,
        } => {
            let response = search(&con, &query, codebase.as_deref(), module.as_deref(), limit)?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        Command::Tool { name, arguments } => {
            let args: serde_json::Value =
                serde_json::from_str(&arguments.unwrap_or_else(|| "{}".to_string()))?;
            let payload = call_tool(&con, &name, &args)?;
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        Command::Mcp => {
            run_mcp_stdio(&con)?;
        }
        Command::Serve => {
            run_http_server(config).await?;
        }
    }

    Ok(())
}

#[derive(Clone)]
struct CacheEntry {
    payload: serde_json::Value,
    expires_at: Instant,
}

struct ResponseCache {
    capacity: usize,
    entries: HashMap<String, CacheEntry>,
    order: VecDeque<String>,
}

impl ResponseCache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn get(&mut self, key: &str) -> Option<serde_json::Value> {
        let entry = self.entries.get(key)?;
        if Instant::now() >= entry.expires_at {
            self.entries.remove(key);
            return None;
        }
        Some(entry.payload.clone())
    }

    fn insert(&mut self, key: String, payload: serde_json::Value, ttl: Duration) {
        if self.capacity == 0 {
            return;
        }
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(
            key,
            CacheEntry {
                payload,
                expires_at: Instant::now() + ttl,
            },
        );
        while self.entries.len() > self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            } else {
                break;
            }
        }
    }
}

#[derive(Clone)]
struct HttpState {
    con_pool: Arc<Vec<Mutex<rusqlite::Connection>>>,
    next_con: Arc<AtomicUsize>,
    bearer_token: Option<String>,
    request_timeout: Duration,
    tool_schemas: Arc<Vec<serde_json::Value>>,
    tool_names: Arc<Vec<serde_json::Value>>,
    response_cache: Arc<Mutex<ResponseCache>>,
    cache_hits: Arc<AtomicUsize>,
    cache_misses: Arc<AtomicUsize>,
}

async fn run_http_server(config: AppConfig) -> Result<()> {
    let pool_size = config.indexer.parallelism.max(1);
    let mut con_pool = Vec::with_capacity(pool_size);
    for _ in 0..pool_size {
        con_pool.push(Mutex::new(open_database(&config.database_path)?));
    }
    let tool_schemas = Arc::new(tool_schemas());
    let tool_names = Arc::new(
        tool_schemas
            .iter()
            .map(|tool| tool["name"].clone())
            .collect::<Vec<_>>(),
    );
    let bearer_token = config
        .server
        .bearer_token_env
        .as_deref()
        .and_then(|name| std::env::var(name).ok())
        .filter(|value| !value.is_empty());
    let state = HttpState {
        con_pool: Arc::new(con_pool),
        next_con: Arc::new(AtomicUsize::new(0)),
        bearer_token,
        request_timeout: Duration::from_secs(config.server.request_timeout_secs),
        tool_schemas,
        tool_names,
        response_cache: Arc::new(Mutex::new(ResponseCache::new(512))),
        cache_hits: Arc::new(AtomicUsize::new(0)),
        cache_misses: Arc::new(AtomicUsize::new(0)),
    };
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
        ])
        .allow_origin(
            config
                .server
                .cors_allow_origin
                .parse::<HeaderValue>()
                .map_err(|err| {
                    odoo_knowledge_core::Error::InvalidConfig(format!("invalid CORS origin: {err}"))
                })?,
        );
    let app = Router::new()
        .route("/", get(http_root))
        .route("/health", get(http_health))
        .route("/mcp", post(http_mcp))
        .layer(DefaultBodyLimit::max(
            config.server.request_body_limit_bytes,
        ))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);
    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .map_err(|err| {
            odoo_knowledge_core::Error::InvalidConfig(format!("invalid listen address: {err}"))
        })?;
    println!(
        "{}",
        serde_json::json!({
            "server": "odoo-knowledge-rs",
            "transport": "http-jsonrpc",
            "host": config.server.host,
            "port": config.server.port,
            "endpoint": "/mcp"
        })
    );
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|err| odoo_knowledge_core::Error::InvalidConfig(format!("bind failed: {err}")))?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|err| {
            odoo_knowledge_core::Error::InvalidConfig(format!("server failed: {err}"))
        })?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

async fn http_root(State(state): State<HttpState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": "odoo-knowledge-rs",
        "transport": "http-jsonrpc",
        "mcp_endpoint": "/mcp",
        "health": "/health",
        "tools": state.tool_names.as_ref().clone()
    }))
}

async fn http_health(State(state): State<HttpState>) -> Json<serde_json::Value> {
    let indexed_codebases = with_connection(&state, |con| {
        con.query_row(
            "SELECT COUNT(*) FROM codebases WHERE indexed_at IS NOT NULL",
            [],
            |row| row.get::<_, i64>(0),
        )
        .ok()
    })
    .ok()
    .flatten()
    .unwrap_or(0);
    Json(serde_json::json!({
        "status": "ok",
        "server": "odoo-knowledge-rs",
        "tools": state.tool_schemas.len(),
        "indexed_codebases": indexed_codebases,
        "cache": {
            "hits": state.cache_hits.load(Ordering::Relaxed),
            "misses": state.cache_misses.load(Ordering::Relaxed)
        }
    }))
}

async fn http_mcp(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(request): Json<serde_json::Value>,
) -> std::result::Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let request_started_at = std::time::Instant::now();
    let method = request.get("method").and_then(serde_json::Value::as_str);
    let tool_name = request
        .get("params")
        .and_then(|params| params.get("name"))
        .and_then(serde_json::Value::as_str);
    if !authorized(&state, &headers) {
        tracing::warn!(
            method = method.unwrap_or(""),
            tool = tool_name.unwrap_or(""),
            elapsed_ms = request_started_at.elapsed().as_millis() as u64,
            "mcp request unauthorized"
        );
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "unauthorized"})),
        ));
    }
    let request_id = request
        .get("id")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    if method == Some("notifications/initialized") {
        tracing::info!(
            method = "notifications/initialized",
            tool = "",
            is_error = false,
            elapsed_ms = request_started_at.elapsed().as_millis() as u64,
            "mcp request completed"
        );
        return Ok(StatusCode::ACCEPTED.into_response());
    }
    let response = match method {
        Some("initialize") => rpc_result(
            request_id,
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "odoo-knowledge-rs", "version": "0.1.0"}
            }),
        ),
        Some("tools/list") => rpc_result(
            request_id,
            serde_json::json!({"tools": state.tool_schemas.as_ref().clone()}),
        ),
        Some("tools/call") => {
            let started_at = std::time::Instant::now();
            let params = request.get("params").unwrap_or(&serde_json::Value::Null);
            let name = params
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let args = params.get("arguments").unwrap_or(&serde_json::Value::Null);
            let cache_key = cache_key(name, args);
            if let Some(payload) = cache_get(&state, &cache_key) {
                tracing::debug!(tool = name, "mcp response cache hit");
                rpc_result(request_id, tool_call_result(payload))
            } else {
                let result =
                    with_connection(&state, |con| call_tool(con, name, args)).map_err(|err| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({"error": err})),
                        )
                    })?;
                if started_at.elapsed() > state.request_timeout {
                    return Ok(Json(rpc_error(request_id, -32000, "request timeout")).into_response());
                }
                match result {
                    Ok(payload) => {
                        if payload.get("error").is_none() {
                            cache_insert(&state, cache_key, payload.clone(), cache_ttl(name));
                        }
                        rpc_result(request_id, tool_call_result(payload))
                    }
                    Err(err) => rpc_result(
                        request_id,
                        serde_json::json!({
                            "content": [{"type": "text", "text": serde_json::json!({"error": err.to_string()}).to_string()}],
                            "isError": true
                        }),
                    ),
                }
            }
        }
        Some("ping") => rpc_result(request_id, serde_json::json!({})),
        Some(other) => rpc_error(request_id, -32601, &format!("unknown method: {other}")),
        None => rpc_error(request_id, -32600, "missing method"),
    };
    let is_error = response.get("error").is_some()
        || response
            .get("result")
            .and_then(|result| result.get("isError"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
    tracing::info!(
        method = method.unwrap_or(""),
        tool = tool_name.unwrap_or(""),
        is_error,
        elapsed_ms = request_started_at.elapsed().as_millis() as u64,
        "mcp request completed"
    );
    Ok(Json(response).into_response())
}

fn tool_call_result(payload: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "content": [{"type": "text", "text": serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())}],
        "isError": payload.get("error").is_some()
    })
}

fn cache_key(name: &str, args: &serde_json::Value) -> String {
    format!(
        "{name}:{}",
        serde_json::to_string(args).unwrap_or_else(|_| "null".to_string())
    )
}

fn cache_ttl(name: &str) -> Duration {
    if name == "odoo_search" {
        Duration::from_secs(60)
    } else {
        Duration::from_secs(3600)
    }
}

fn cache_get(state: &HttpState, key: &str) -> Option<serde_json::Value> {
    let payload = state
        .response_cache
        .lock()
        .ok()
        .and_then(|mut cache| cache.get(key));
    if payload.is_some() {
        state.cache_hits.fetch_add(1, Ordering::Relaxed);
    } else {
        state.cache_misses.fetch_add(1, Ordering::Relaxed);
    }
    payload
}

fn cache_insert(state: &HttpState, key: String, payload: serde_json::Value, ttl: Duration) {
    if let Ok(mut cache) = state.response_cache.lock() {
        cache.insert(key, payload, ttl);
    }
}

fn with_connection<T>(
    state: &HttpState,
    f: impl FnOnce(&rusqlite::Connection) -> T,
) -> std::result::Result<T, String> {
    let pool_len = state.con_pool.len();
    let index = state.next_con.fetch_add(1, Ordering::Relaxed) % pool_len;
    let con = state.con_pool[index]
        .lock()
        .map_err(|err| err.to_string())?;
    Ok(f(&con))
}

fn authorized(state: &HttpState, headers: &HeaderMap) -> bool {
    let Some(token) = &state.bearer_token else {
        return true;
    };
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == format!("Bearer {token}"))
}

fn call_tool(
    con: &rusqlite::Connection,
    name: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let get = |key: &str| args.get(key).and_then(serde_json::Value::as_str);
    let payload = match name {
        "odoo_search" => {
            let filters = args.get("filters").unwrap_or(&serde_json::Value::Null);
            let limit = filters
                .get("limit")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(20) as usize;
            let response = search(
                con,
                required(get("query"), "query")?,
                filters.get("codebase").and_then(serde_json::Value::as_str),
                filters.get("module").and_then(serde_json::Value::as_str),
                limit,
            )?;
            serde_json::to_value(response)?
        }
        "odoo_module_context" => services::module_context(
            con,
            required(get("module_name"), "module_name")?,
            get("codebase"),
        )?,
        "odoo_model_context" => services::model_context(
            con,
            required(get("model_name"), "model_name")?,
            get("codebase"),
        )?,
        "odoo_field_context" => services::field_context(
            con,
            required(get("model_name"), "model_name")?,
            required(get("field_name"), "field_name")?,
            get("codebase"),
        )?,
        "odoo_method_chain" => services::method_chain(
            con,
            required(get("model_name"), "model_name")?,
            required(get("method_name"), "method_name")?,
            get("codebase"),
        )?,
        "odoo_view_chain" => services::view_chain(
            con,
            required(get("xmlid_or_model"), "xmlid_or_model")?,
            get("codebase"),
        )?,
        "odoo_xmlid_lookup" => {
            services::xmlid_lookup(con, required(get("xmlid"), "xmlid")?, get("codebase"))?
        }
        "odoo_impact_analysis" => {
            services::impact_analysis(con, required(get("target"), "target")?, get("codebase"))?
        }
        "odoo_context_bundle" => {
            let limit = args
                .get("limit")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(10) as usize;
            services::context_bundle(
                con,
                required(get("query"), "query")?,
                get("codebase"),
                get("module"),
                limit,
            )?
        }
        "odoo_trace_business_flow" => services::trace_business_flow(
            con,
            required(get("model_name"), "model_name")?,
            required(get("method_name"), "method_name")?,
            get("codebase"),
        )?,
        "odoo_find_extension_point" => services::find_extension_point(
            con,
            required(get("goal"), "goal")?,
            get("codebase"),
            get("module"),
        )?,
        "odoo_debug_hypotheses" => services::debug_hypotheses(
            con,
            required(get("symptom"), "symptom")?,
            get("codebase"),
            get("module"),
        )?,
        "odoo_compare_symbol" => services::compare_symbol(
            con,
            required(get("symbol"), "symbol")?,
            required(get("left_codebase"), "left_codebase")?,
            required(get("right_codebase"), "right_codebase")?,
        )?,
        _ => serde_json::json!({
            "error": format!("unknown or unimplemented tool: {name}"),
            "available_tools": [
                "odoo_impact_analysis",
                "odoo_context_bundle",
                "odoo_trace_business_flow",
                "odoo_find_extension_point",
                "odoo_debug_hypotheses",
                "odoo_compare_symbol",
                "odoo_module_context",
                "odoo_model_context",
                "odoo_field_context",
                "odoo_method_chain",
                "odoo_view_chain",
                "odoo_xmlid_lookup"
            ]
        }),
    };
    Ok(payload)
}

fn required<'a>(value: Option<&'a str>, name: &str) -> Result<&'a str> {
    value.ok_or_else(|| {
        odoo_knowledge_core::Error::InvalidConfig(format!("missing required argument: {name}"))
    })
}

fn run_mcp_stdio(con: &rusqlite::Connection) -> Result<()> {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(err) => {
                println!(
                    "{}",
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": null,
                        "error": {"code": -32700, "message": err.to_string()}
                    })
                );
                continue;
            }
        };
        let method = request.get("method").and_then(serde_json::Value::as_str);
        let request_id = request
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        if method == Some("notifications/initialized") {
            continue;
        }
        let response = match method {
            Some("initialize") => rpc_result(
                request_id,
                serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "odoo-knowledge-rs", "version": "0.1.0"}
                }),
            ),
            Some("tools/list") => {
                rpc_result(request_id, serde_json::json!({"tools": tool_schemas()}))
            }
            Some("tools/call") => {
                let params = request.get("params").unwrap_or(&serde_json::Value::Null);
                let name = params
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                let args = params.get("arguments").unwrap_or(&serde_json::Value::Null);
                match call_tool(con, name, args) {
                    Ok(payload) => rpc_result(
                        request_id,
                        serde_json::json!({
                            "content": [{"type": "text", "text": serde_json::to_string_pretty(&payload)?}],
                            "isError": payload.get("error").is_some()
                        }),
                    ),
                    Err(err) => rpc_result(
                        request_id,
                        serde_json::json!({
                            "content": [{"type": "text", "text": serde_json::to_string_pretty(&serde_json::json!({"error": err.to_string()}))?}],
                            "isError": true
                        }),
                    ),
                }
            }
            Some("ping") => rpc_result(request_id, serde_json::json!({})),
            Some(other) => rpc_error(request_id, -32601, &format!("unknown method: {other}")),
            None => rpc_error(request_id, -32600, "missing method"),
        };
        println!("{}", response);
    }
    Ok(())
}

fn rpc_result(id: serde_json::Value, result: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn rpc_error(id: serde_json::Value, code: i64, message: &str) -> serde_json::Value {
    serde_json::json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn tool_schemas() -> Vec<serde_json::Value> {
    vec![
        tool_schema(
            "odoo_search",
            "Hybrid lexical/metadata search over indexed Odoo codebase.",
            serde_json::json!({
                "query": string_schema("Search query."),
                "filters": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "codebase": codebase_schema(),
                        "module": string_schema("Optional module filter."),
                        "limit": {"type": "integer", "minimum": 1, "maximum": 100, "description": "Maximum result count."}
                    }
                }
            }),
            &["query"],
        ),
        tool_schema(
            "odoo_impact_analysis",
            "Return graph edges and symbols related to a target symbol, file, or XMLID.",
            common_props(serde_json::json!({
                "target": string_schema("Symbol qualname/name, file path, model name, or XMLID to inspect.")
            })),
            &["target"],
        ),
        tool_schema(
            "odoo_context_bundle",
            "Build a compact context bundle for debugging, implementation, review, or tracing.",
            common_props(serde_json::json!({
                "query": string_schema("Topic, symbol, model, method, or symptom to bundle context for."),
                "module": string_schema("Optional module filter."),
                "limit": {"type": "integer", "minimum": 1, "maximum": 50, "description": "Maximum search result count."}
            })),
            &["query"],
        ),
        tool_schema(
            "odoo_trace_business_flow",
            "Trace an Odoo business entrypoint using method chain and graph edges.",
            common_props(serde_json::json!({
                "model_name": string_schema("Odoo model name, for example sale.order."),
                "method_name": string_schema("Method/entrypoint name, for example action_confirm.")
            })),
            &["model_name", "method_name"],
        ),
        tool_schema(
            "odoo_find_extension_point",
            "Find candidate extension points for a development goal.",
            common_props(serde_json::json!({
                "goal": string_schema("Development goal or target symbol to find extension points for."),
                "module": string_schema("Optional module filter.")
            })),
            &["goal"],
        ),
        tool_schema(
            "odoo_debug_hypotheses",
            "Build debugging hypotheses and relevant context for a symptom.",
            common_props(serde_json::json!({
                "symptom": string_schema("Bug symptom, error text, model, method, or behavior to investigate."),
                "module": string_schema("Optional module filter.")
            })),
            &["symptom"],
        ),
        tool_schema(
            "odoo_compare_symbol",
            "Compare a symbol across two indexed Odoo codebases.",
            serde_json::json!({
                "symbol": string_schema("Symbol name, qualname, or file path to compare."),
                "left_codebase": string_schema("Left indexed Odoo source codebase, for example `odoo-17`. Use the Odoo CE/core version, not the local project/addons directory name."),
                "right_codebase": string_schema("Right indexed Odoo source codebase, for example `odoo-19`. Use the Odoo CE/core version, not the local project/addons directory name.")
            }),
            &["symbol", "left_codebase", "right_codebase"],
        ),
        tool_schema(
            "odoo_module_context",
            "Return manifest, dependencies, models, and views for an Odoo module.",
            common_props(serde_json::json!({
                "module_name": string_schema("Odoo addon module name.")
            })),
            &["module_name"],
        ),
        tool_schema(
            "odoo_model_context",
            "Return model contributors, fields, methods, and views.",
            common_props(serde_json::json!({
                "model_name": string_schema("Odoo model name.")
            })),
            &["model_name"],
        ),
        tool_schema(
            "odoo_method_chain",
            "Return static override chain for an Odoo model method.",
            common_props(serde_json::json!({
                "model_name": string_schema("Odoo model name."),
                "method_name": string_schema("Method name.")
            })),
            &["model_name", "method_name"],
        ),
        tool_schema(
            "odoo_field_context",
            "Return field definitions, origins, and related view usage.",
            common_props(serde_json::json!({
                "model_name": string_schema("Odoo model name."),
                "field_name": string_schema("Field name.")
            })),
            &["model_name", "field_name"],
        ),
        tool_schema(
            "odoo_view_chain",
            "Return view records by XMLID or model and inheritance links.",
            common_props(serde_json::json!({
                "xmlid_or_model": string_schema("View XMLID or target model name.")
            })),
            &["xmlid_or_model"],
        ),
        tool_schema(
            "odoo_xmlid_lookup",
            "Lookup XMLID records, views, actions, and menus.",
            common_props(serde_json::json!({
                "xmlid": string_schema("Fully qualified XMLID.")
            })),
            &["xmlid"],
        ),
    ]
}

fn tool_schema(
    name: &str,
    description: &str,
    properties: serde_json::Value,
    required: &[&str],
) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "additionalProperties": false,
            "properties": properties,
            "required": required
        }
    })
}

fn common_props(mut properties: serde_json::Value) -> serde_json::Value {
    if let Some(object) = properties.as_object_mut() {
        object.insert(
            "codebase".to_string(),
            codebase_schema(),
        );
    }
    properties
}

fn codebase_schema() -> serde_json::Value {
    string_schema(
        "Optional indexed Odoo source codebase. Use the Odoo CE/core version this project runs on, for example `odoo-17`, `odoo-18`, or `odoo-19`; do not use the local project/addons directory name unless that exact name is indexed. Version-like values such as `17`, `17.0`, or `Odoo 17 CE` may resolve when exactly one indexed codebase matches.",
    )
}

fn string_schema(description: &str) -> serde_json::Value {
    serde_json::json!({"type": "string", "description": description})
}

fn diagnostics_rows(
    con: &rusqlite::Connection,
    sql: &str,
    codebase: Option<&str>,
) -> Result<Vec<serde_json::Value>> {
    let mut stmt = con.prepare(sql)?;
    let mut rows = if let Some(codebase) = codebase {
        stmt.query([codebase])?
    } else {
        stmt.query([])?
    };
    let mut diagnostics = Vec::new();
    while let Some(row) = rows.next()? {
        diagnostics.push(serde_json::json!({
            "severity": row.get::<_, String>(0)?,
            "kind": row.get::<_, String>(1)?,
            "message": row.get::<_, String>(2)?,
            "file_path": row.get::<_, Option<String>>(3)?,
            "line_start": row.get::<_, Option<i64>>(4)?,
        }));
    }
    Ok(diagnostics)
}
