use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

#[test]
fn mcp_stdio_lists_tools() {
    let db_path = temp_path("stdio", "db");
    let mut child = Command::new(odoo_binary())
        .arg("--db")
        .arg(&db_path)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{}}}}"#
        )
        .unwrap();
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{{}}}}"#
        )
        .unwrap();
    }

    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let responses = String::from_utf8(output.stdout).unwrap();
    let lines = responses.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);

    let initialize: Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(initialize["id"], 1);
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        "odoo-knowledge-rs"
    );

    let tools: Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(tools["id"], 2);
    assert_tool_schema(
        &tools["result"]["tools"],
        "odoo_trace_business_flow",
        &["model_name", "method_name"],
    );
}

#[test]
fn http_jsonrpc_lists_tools() {
    let db_path = temp_path("http", "db");
    let config_path = temp_path("http", "toml");
    let port = free_port();
    std::env::remove_var("ODOO_KNOWLEDGE_TEST_EMPTY_TOKEN");
    std::fs::write(
        &config_path,
        format!(
            r#"environment = "test"
database_path = "{}"
log_level = "debug"

[server]
host = "127.0.0.1"
port = {port}
bearer_token_env = "ODOO_KNOWLEDGE_TEST_EMPTY_TOKEN"

[indexer]
parallelism = 1
"#,
            db_path.display()
        ),
    )
    .unwrap();

    let mut child = Command::new(odoo_binary())
        .arg("--config")
        .arg(&config_path)
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    wait_for_server_banner(&mut child);
    wait_for_tcp(port);

    let response = http_post_json(
        port,
        r#"{"jsonrpc":"2.0","id":7,"method":"tools/list","params":{}}"#,
    );
    let body = response.split("\r\n\r\n").nth(1).unwrap_or("");
    let payload: Value = serde_json::from_str(body).unwrap();
    assert_eq!(payload["id"], 7);
    assert_tool_schema(
        &payload["result"]["tools"],
        "odoo_compare_symbol",
        &["symbol", "left_codebase", "right_codebase"],
    );

    stop_child(&mut child);
}

#[test]
fn http_hardening_auth_health_and_cors() {
    let db_path = temp_path("http-hardening", "db");
    let config_path = temp_path("http-hardening", "toml");
    let port = free_port();
    std::env::set_var("ODOO_KNOWLEDGE_TEST_TOKEN", "secret-token");
    std::fs::write(
        &config_path,
        format!(
            r#"environment = "test"
database_path = "{}"
log_level = "debug"

[server]
host = "127.0.0.1"
port = {port}
bearer_token_env = "ODOO_KNOWLEDGE_TEST_TOKEN"
request_body_limit_bytes = 1048576
request_timeout_secs = 30
cors_allow_origin = "http://127.0.0.1"

[indexer]
parallelism = 1
"#,
            db_path.display()
        ),
    )
    .unwrap();

    let mut child = Command::new(odoo_binary())
        .arg("--config")
        .arg(&config_path)
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    wait_for_server_banner(&mut child);
    wait_for_tcp(port);

    let health = http_get(port, "/health", &[]);
    assert!(health.starts_with("HTTP/1.1 200 OK"), "{health}");
    let health_body = health.split("\r\n\r\n").nth(1).unwrap_or("");
    let payload: Value = serde_json::from_str(health_body).unwrap();
    assert_eq!(payload["status"], "ok");
    assert!(payload["database_path"]
        .as_str()
        .unwrap()
        .contains("http-hardening"));

    let unauthorized = http_post_json_raw(
        port,
        r#"{"jsonrpc":"2.0","id":9,"method":"tools/list","params":{}}"#,
        &[],
    );
    assert!(
        unauthorized.starts_with("HTTP/1.1 401 Unauthorized"),
        "{unauthorized}"
    );

    let authorized = http_post_json_raw(
        port,
        r#"{"jsonrpc":"2.0","id":10,"method":"tools/list","params":{}}"#,
        &[
            ("Authorization", "Bearer secret-token"),
            ("Origin", "http://127.0.0.1"),
        ],
    );
    assert!(authorized.starts_with("HTTP/1.1 200 OK"), "{authorized}");
    assert!(authorized
        .to_ascii_lowercase()
        .contains("access-control-allow-origin"));

    stop_child(&mut child);
    std::env::remove_var("ODOO_KNOWLEDGE_TEST_TOKEN");
}

fn odoo_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_odoo-knowledge"))
}

fn assert_tool_schema(tools: &Value, name: &str, required: &[&str]) {
    let tool = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == name)
        .unwrap_or_else(|| panic!("missing tool schema for {name}"));
    let schema = &tool["inputSchema"];
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["additionalProperties"], false);
    for required_field in required {
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field == required_field));
        assert_eq!(schema["properties"][*required_field]["type"], "string");
    }
}

fn temp_path(label: &str, extension: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "odoo-knowledge-rs-{label}-{}-{label}.{extension}",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn wait_for_server_banner(child: &mut Child) {
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let payload: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(payload["server"], "odoo-knowledge-rs");
}

fn wait_for_tcp(port: u16) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("server did not accept TCP connections on port {port}");
}

fn http_post_json(port: u16, body: &str) -> String {
    let response = http_post_json_raw(port, body, &[]);
    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    response
}

fn http_get(port: u16, path: &str, headers: &[(&str, &str)]) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let header_text = headers
        .iter()
        .map(|(name, value)| format!("{name}: {value}\r\n"))
        .collect::<String>();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{header_text}Connection: close\r\n\r\n",
    )
    .unwrap();
    let mut response = String::new();
    std::io::Read::read_to_string(&mut stream, &mut response).unwrap();
    response
}

fn http_post_json_raw(port: u16, body: &str, headers: &[(&str, &str)]) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let header_text = headers
        .iter()
        .map(|(name, value)| format!("{name}: {value}\r\n"))
        .collect::<String>();
    write!(
        stream,
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\n{header_text}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
    .unwrap();
    let mut response = String::new();
    std::io::Read::read_to_string(&mut stream, &mut response).unwrap();
    response
}

fn stop_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}
