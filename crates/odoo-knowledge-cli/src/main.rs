use std::io::{self, BufRead};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use clap::{Parser, Subcommand};
use odoo_knowledge_core::codebase::{add_codebase, list_codebases};
use odoo_knowledge_core::indexer::index_codebase;
use odoo_knowledge_core::search::search;
use odoo_knowledge_core::services;
use odoo_knowledge_core::storage::open_database;
use odoo_knowledge_core::{AppConfig, Result};

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
struct HttpState {
    con: Arc<Mutex<rusqlite::Connection>>,
    bearer_token: Option<String>,
}

async fn run_http_server(config: AppConfig) -> Result<()> {
    let con = open_database(&config.database_path)?;
    let bearer_token = config
        .server
        .bearer_token_env
        .as_deref()
        .and_then(|name| std::env::var(name).ok())
        .filter(|value| !value.is_empty());
    let state = HttpState {
        con: Arc::new(Mutex::new(con)),
        bearer_token,
    };
    let app = Router::new()
        .route("/", get(http_root))
        .route("/health", get(http_health))
        .route("/mcp", post(http_mcp))
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
    axum::serve(listener, app).await.map_err(|err| {
        odoo_knowledge_core::Error::InvalidConfig(format!("server failed: {err}"))
    })?;
    Ok(())
}

async fn http_root(State(_state): State<HttpState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": "odoo-knowledge-rs",
        "transport": "http-jsonrpc",
        "mcp_endpoint": "/mcp",
        "health": "/health",
        "tools": tool_schemas().into_iter().map(|tool| tool["name"].clone()).collect::<Vec<_>>()
    }))
}

async fn http_health(State(_state): State<HttpState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "server": "odoo-knowledge-rs",
        "tools": tool_schemas().len()
    }))
}

async fn http_mcp(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(request): Json<serde_json::Value>,
) -> std::result::Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if !authorized(&state, &headers) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "unauthorized"})),
        ));
    }
    let method = request.get("method").and_then(serde_json::Value::as_str);
    let request_id = request
        .get("id")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let response = match method {
        Some("initialize") => rpc_result(
            request_id,
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "odoo-knowledge-rs", "version": "0.1.0"}
            }),
        ),
        Some("tools/list") => rpc_result(request_id, serde_json::json!({"tools": tool_schemas()})),
        Some("tools/call") => {
            let params = request.get("params").unwrap_or(&serde_json::Value::Null);
            let name = params
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let args = params.get("arguments").unwrap_or(&serde_json::Value::Null);
            let con = state.con.lock().map_err(|err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": err.to_string()})),
                )
            })?;
            match call_tool(&con, name, args) {
                Ok(payload) => rpc_result(
                    request_id,
                    serde_json::json!({
                        "content": [{"type": "text", "text": serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())}],
                        "isError": payload.get("error").is_some()
                    }),
                ),
                Err(err) => rpc_result(
                    request_id,
                    serde_json::json!({
                        "content": [{"type": "text", "text": serde_json::json!({"error": err.to_string()}).to_string()}],
                        "isError": true
                    }),
                ),
            }
        }
        Some("ping") => rpc_result(request_id, serde_json::json!({})),
        Some(other) => rpc_error(request_id, -32601, &format!("unknown method: {other}")),
        None => rpc_error(request_id, -32600, "missing method"),
    };
    Ok(Json(response))
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
        _ => serde_json::json!({
            "error": format!("unknown or unimplemented tool: {name}"),
            "available_tools": [
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
    [
        (
            "odoo_search",
            "Hybrid lexical/metadata search over indexed Odoo codebase.",
        ),
        (
            "odoo_module_context",
            "Return manifest, dependencies, models, and views for an Odoo module.",
        ),
        (
            "odoo_model_context",
            "Return model contributors, fields, methods, and views.",
        ),
        (
            "odoo_method_chain",
            "Return static override chain for an Odoo model method.",
        ),
        (
            "odoo_field_context",
            "Return field definitions, origins, and related view usage.",
        ),
        (
            "odoo_view_chain",
            "Return view records by XMLID or model and inheritance links.",
        ),
        (
            "odoo_xmlid_lookup",
            "Lookup XMLID records, views, actions, and menus.",
        ),
    ]
    .into_iter()
    .map(|(name, description)| {
        serde_json::json!({
            "name": name,
            "description": description,
            "inputSchema": {
                "type": "object",
                "additionalProperties": true,
                "properties": {}
            }
        })
    })
    .collect()
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
