use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

#[test]
fn cli_registry_index_search_and_tool_smoke() {
    let db_path = temp_db_path();
    let fixture_root = workspace_root().join("tests/fixtures");

    let added = run_json(&[
        "--db",
        db_path.to_str().unwrap(),
        "add-codebase",
        "--name",
        "fixtures",
        "--path",
        fixture_root.to_str().unwrap(),
    ]);
    assert_eq!(added["name"], "fixtures");
    assert!(added["codebase_id"].as_i64().unwrap() > 0);

    let listed = run_json(&["--db", db_path.to_str().unwrap(), "list-codebases"]);
    assert!(listed
        .as_array()
        .unwrap()
        .iter()
        .any(|codebase| codebase["name"] == "fixtures"));

    let added_copy = run_json(&[
        "--db",
        db_path.to_str().unwrap(),
        "add-codebase",
        "--name",
        "fixtures_copy",
        "--path",
        fixture_root.to_str().unwrap(),
    ]);
    assert_eq!(added_copy["name"], "fixtures_copy");

    let indexed = run_json(&[
        "--db",
        db_path.to_str().unwrap(),
        "index",
        "--codebase",
        "fixtures",
    ]);
    assert_eq!(indexed["stats"]["modules"], 1);
    assert_eq!(indexed["stats"]["frontend"], 6);

    let indexed_copy = run_json(&[
        "--db",
        db_path.to_str().unwrap(),
        "index",
        "--codebase",
        "fixtures_copy",
    ]);
    assert_eq!(indexed_copy["stats"]["modules"], 1);

    let added_odoo17 = run_json(&[
        "--db",
        db_path.to_str().unwrap(),
        "add-codebase",
        "--name",
        "odoo-17",
        "--path",
        fixture_root.to_str().unwrap(),
    ]);
    assert_eq!(added_odoo17["name"], "odoo-17");

    let indexed_odoo17 = run_json(&[
        "--db",
        db_path.to_str().unwrap(),
        "index",
        "--codebase",
        "odoo-17",
    ]);
    assert_eq!(indexed_odoo17["stats"]["modules"], 1);

    let search = run_json(&[
        "--db",
        db_path.to_str().unwrap(),
        "search",
        "sale.order",
        "--codebase",
        "fixtures",
        "--limit",
        "10",
    ]);
    assert!(search["results"]["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .any(|symbol| symbol["name"] == "sale.order"));

    let tool = run_json(&[
        "--db",
        db_path.to_str().unwrap(),
        "tool",
        "odoo_model_context",
        r#"{"model_name":"sale.order","codebase":"fixtures"}"#,
    ]);
    assert_eq!(tool["model"], "sale.order");
    assert!(tool["fields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["field_name"] == "x_reference"));

    let version_alias_tool = run_json(&[
        "--db",
        db_path.to_str().unwrap(),
        "tool",
        "odoo_model_context",
        r#"{"model_name":"sale.order","codebase":"Odoo 17 CE"}"#,
    ]);
    assert_eq!(version_alias_tool["codebase"]["name"], "odoo-17");

    let search_top_level_alias = run_json(&[
        "--db",
        db_path.to_str().unwrap(),
        "tool",
        "odoo_search",
        r#"{"query":"sale.order","codebase":"Odoo 17 CE","limit":2}"#,
    ]);
    assert_eq!(search_top_level_alias["codebase"]["name"], "odoo-17");
    assert!(
        search_top_level_alias["results"]["symbols"]
            .as_array()
            .unwrap()
            .len()
            <= 2
    );

    let unknown_codebase_error = run_failure(&[
        "--db",
        db_path.to_str().unwrap(),
        "tool",
        "odoo_model_context",
        r#"{"model_name":"sale.order","codebase":"suqma-local"}"#,
    ]);
    assert!(unknown_codebase_error.contains("suqma-local"));
    assert!(unknown_codebase_error.contains("Available codebases"));
    assert!(unknown_codebase_error.contains("not a local project/addons directory name"));

    let impact = run_json(&[
        "--db",
        db_path.to_str().unwrap(),
        "tool",
        "odoo_impact_analysis",
        r#"{"target":"sale.order","codebase":"fixtures"}"#,
    ]);
    assert_eq!(impact["normalized_target"], "sale.order");
    assert!(impact["matches"]
        .as_array()
        .unwrap()
        .iter()
        .any(|symbol| symbol["kind"] == "model" && symbol["name"] == "sale.order"));
    assert!(impact["incoming_edges"]
        .as_array()
        .unwrap()
        .iter()
        .any(|edge| edge["edge_type"] == "targets_model"));

    let bundle = run_json(&[
        "--db",
        db_path.to_str().unwrap(),
        "tool",
        "odoo_context_bundle",
        r#"{"query":"sale.order","codebase":"fixtures","limit":5}"#,
    ]);
    assert_eq!(bundle["query"], "sale.order");
    assert!(bundle["search"]["results"]["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .any(|symbol| symbol["name"] == "sale.order"));
    assert!(!bundle["impact_samples"].as_array().unwrap().is_empty());

    let trace = run_json(&[
        "--db",
        db_path.to_str().unwrap(),
        "tool",
        "odoo_trace_business_flow",
        r#"{"model_name":"sale.order","method_name":"action_confirm","codebase":"fixtures"}"#,
    ]);
    assert_eq!(trace["model"], "sale.order");
    assert_eq!(trace["method"], "action_confirm");
    assert!(trace["method_chain"]["chain"]
        .as_array()
        .unwrap()
        .iter()
        .any(|method| method["method_name"] == "action_confirm"));

    let extension = run_json(&[
        "--db",
        db_path.to_str().unwrap(),
        "tool",
        "odoo_find_extension_point",
        r#"{"goal":"sale.order","codebase":"fixtures"}"#,
    ]);
    assert!(extension["candidate_models"]
        .as_array()
        .unwrap()
        .iter()
        .any(|model| model["model_name"] == "sale.order"));

    let hypotheses = run_json(&[
        "--db",
        db_path.to_str().unwrap(),
        "tool",
        "odoo_debug_hypotheses",
        r#"{"symptom":"sale.order action_confirm","codebase":"fixtures"}"#,
    ]);
    assert!(hypotheses["hypotheses"].as_array().unwrap().len() >= 4);
    assert_eq!(
        hypotheses["context_bundle"]["query"],
        "sale.order action_confirm"
    );

    let comparison = run_json(&[
        "--db",
        db_path.to_str().unwrap(),
        "tool",
        "odoo_compare_symbol",
        r#"{"symbol":"sale.order","left_codebase":"fixtures","right_codebase":"fixtures_copy"}"#,
    ]);
    assert_eq!(comparison["summary"]["status"], "present_both");

    let diagnostics = run_json(&[
        "--db",
        db_path.to_str().unwrap(),
        "validate",
        "--codebase",
        "fixtures",
    ]);
    assert_eq!(diagnostics["codebase"], "fixtures");
    assert!(diagnostics["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["kind"] == "missing_dependency_source"));
}

fn run_json(args: &[&str]) -> Value {
    let output = Command::new(odoo_binary()).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "command failed: {:?}\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn run_failure(args: &[&str]) -> String {
    let output = Command::new(odoo_binary()).args(args).output().unwrap();
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded: {:?}\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn odoo_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_odoo-knowledge"))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

fn temp_db_path() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "odoo-knowledge-rs-cli-smoke-{}.db",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}
