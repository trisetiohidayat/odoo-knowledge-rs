use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use crate::codebase::{get_codebase, Codebase};
use crate::error::{Error, Result};
use crate::search::search;

const MATERIALIZED_PAYLOAD_VERSION: &str = "mcp-context-v1";

pub fn module_context(
    con: &Connection,
    module_name: &str,
    codebase: Option<&str>,
) -> Result<Value> {
    let cb = get_codebase(con, codebase)?;
    if let Some(payload) = materialized_payload(con, &cb, "odoo_module_context", module_name)? {
        return Ok(payload);
    }
    module_context_live_for_cb(con, &cb, module_name)
}

fn module_context_live_for_cb(con: &Connection, cb: &Codebase, module_name: &str) -> Result<Value> {
    let Some(module) = query_one_json(
        con,
        "SELECT * FROM modules WHERE codebase_id=?1 AND name=?2",
        params![cb.id, module_name],
    )?
    else {
        return Ok(
            json!({"error": format!("module not found: {module_name}"), "codebase": cb_meta(&cb)}),
        );
    };
    Ok(json!({
        "codebase": cb_meta(&cb),
        "module": module,
        "depends": query_strings(con, "SELECT depends_on FROM module_dependencies WHERE codebase_id=?1 AND module=?2 ORDER BY depends_on", params![cb.id, module_name])?,
        "dependents": query_strings(con, "SELECT module FROM module_dependencies WHERE codebase_id=?1 AND depends_on=?2 ORDER BY module", params![cb.id, module_name])?,
        "models": query_json(con, "SELECT model_name, class_name, file_path, line_start FROM models WHERE codebase_id=?1 AND module=?2 ORDER BY model_name LIMIT 200", params![cb.id, module_name])?,
        "views": query_json(con, "SELECT xmlid, view_model, inherit_id, file_path, line_start FROM views WHERE codebase_id=?1 AND module=?2 ORDER BY xmlid LIMIT 200", params![cb.id, module_name])?,
        "profile": "all_source",
        "basis": "sqlite_graph",
        "confidence": "high"
    }))
}

pub fn model_context(con: &Connection, model_name: &str, codebase: Option<&str>) -> Result<Value> {
    let cb = get_codebase(con, codebase)?;
    let contributors = query_json(con, "SELECT module, model_name, class_name, inherit, inherits, file_path, line_start, line_end FROM models WHERE codebase_id=?1 AND model_name=?2 ORDER BY module, file_path, line_start LIMIT 120", params![cb.id, model_name])?;
    Ok(json!({
        "codebase": cb_meta(&cb),
        "model": model_name,
        "contributors": contributors,
        "fields": query_json(con, "SELECT field_name, field_type, module, comodel, compute, related, file_path, line_start FROM fields WHERE codebase_id=?1 AND model_name=?2 ORDER BY field_name, module LIMIT 300", params![cb.id, model_name])?,
        "methods": query_json(con, "SELECT method_name, module, class_name, calls_super, file_path, line_start FROM methods WHERE codebase_id=?1 AND model_name=?2 ORDER BY method_name, module LIMIT 300", params![cb.id, model_name])?,
        "views": query_json(con, "SELECT xmlid, module, inherit_id, xpath_count, file_path, line_start FROM views WHERE codebase_id=?1 AND view_model=?2 ORDER BY module, xmlid LIMIT 200", params![cb.id, model_name])?,
        "profile": "all_source",
        "basis": "sqlite_graph",
        "confidence": if contributors.is_empty() { "low" } else { "high" }
    }))
}

pub fn field_context(
    con: &Connection,
    model_name: &str,
    field_name: &str,
    codebase: Option<&str>,
) -> Result<Value> {
    let cb = get_codebase(con, codebase)?;
    let definitions = query_json(con, "SELECT * FROM fields WHERE codebase_id=?1 AND model_name=?2 AND field_name=?3 ORDER BY module, file_path, line_start", params![cb.id, model_name, field_name])?;
    Ok(json!({
        "codebase": cb_meta(&cb),
        "model": model_name,
        "field": field_name,
        "definitions": definitions,
        "related_views_sample": query_json(con, "SELECT xmlid, module, file_path, line_start FROM views WHERE codebase_id=?1 AND view_model=?2 ORDER BY module, xmlid LIMIT 100", params![cb.id, model_name])?,
        "profile": "all_source",
        "basis": "tree_sitter_python_parse",
        "confidence": if definitions.is_empty() { "low" } else { "medium" }
    }))
}

pub fn method_chain(
    con: &Connection,
    model_name: &str,
    method_name: &str,
    codebase: Option<&str>,
) -> Result<Value> {
    let cb = get_codebase(con, codebase)?;
    let order = module_load_order(con, cb.id)?;
    let mut chain = query_json(con, "SELECT module, model_name, class_name, method_name, decorators, calls_super, file_path, line_start, line_end FROM methods WHERE codebase_id=?1 AND model_name=?2 AND method_name=?3", params![cb.id, model_name, method_name])?;
    chain.sort_by(|left, right| {
        let left_module = left.get("module").and_then(Value::as_str).unwrap_or("");
        let right_module = right.get("module").and_then(Value::as_str).unwrap_or("");
        let left_order = order.get(left_module).copied().unwrap_or(0);
        let right_order = order.get(right_module).copied().unwrap_or(0);
        right_order.cmp(&left_order)
    });
    for row in &mut chain {
        if let Some(raw) = row.get("decorators").and_then(Value::as_str) {
            if let Ok(parsed) = serde_json::from_str::<Value>(raw) {
                row["decorators"] = parsed;
            }
        }
    }
    Ok(json!({
        "codebase": cb_meta(&cb),
        "model": model_name,
        "method": method_name,
        "chain": chain,
        "profile": "all_source",
        "note": "Order approximates Odoo override order by reversing dependency-based module load order; exact registry MRO may differ.",
        "basis": "tree_sitter_python_parse",
        "confidence": if chain.is_empty() { "low" } else { "medium" }
    }))
}

pub fn view_chain(con: &Connection, xmlid_or_model: &str, codebase: Option<&str>) -> Result<Value> {
    let cb = get_codebase(con, codebase)?;
    let views = if xmlid_or_model.contains('.') && !xmlid_or_model.starts_with("model:") {
        query_json(con, "SELECT * FROM views WHERE codebase_id=?1 AND (xmlid=?2 OR inherit_id=?2) ORDER BY module, xmlid", params![cb.id, xmlid_or_model])?
    } else {
        query_json(
            con,
            "SELECT * FROM views WHERE codebase_id=?1 AND view_model=?2 ORDER BY module, xmlid",
            params![cb.id, xmlid_or_model],
        )?
    };
    Ok(json!({
        "codebase": cb_meta(&cb),
        "query": xmlid_or_model,
        "views": views,
        "profile": "all_source",
        "basis": "exact_xml_parse",
        "confidence": if views.is_empty() { "low" } else { "high" }
    }))
}

pub fn xmlid_lookup(con: &Connection, xmlid: &str, codebase: Option<&str>) -> Result<Value> {
    let cb = get_codebase(con, codebase)?;
    if let Some(payload) = materialized_payload(con, &cb, "odoo_xmlid_lookup", xmlid)? {
        return Ok(payload);
    }
    xmlid_lookup_live_for_cb(con, &cb, xmlid)
}

fn xmlid_lookup_live_for_cb(con: &Connection, cb: &Codebase, xmlid: &str) -> Result<Value> {
    let records = query_json(
        con,
        "SELECT * FROM xml_records WHERE codebase_id=?1 AND xmlid=?2",
        params![cb.id, xmlid],
    )?;
    let views = query_json(
        con,
        "SELECT * FROM views WHERE codebase_id=?1 AND xmlid=?2",
        params![cb.id, xmlid],
    )?;
    let actions = query_json(
        con,
        "SELECT * FROM actions WHERE codebase_id=?1 AND xmlid=?2",
        params![cb.id, xmlid],
    )?;
    let menus = query_json(
        con,
        "SELECT * FROM menus WHERE codebase_id=?1 AND xmlid=?2",
        params![cb.id, xmlid],
    )?;
    let found = !(records.is_empty() && views.is_empty() && actions.is_empty() && menus.is_empty());
    Ok(json!({
        "codebase": cb_meta(&cb),
        "xmlid": xmlid,
        "records": records,
        "views": views,
        "actions": actions,
        "menus": menus,
        "basis": "exact_xml_parse",
        "confidence": if found { "high" } else { "low" }
    }))
}

pub fn impact_analysis(con: &Connection, target: &str, codebase: Option<&str>) -> Result<Value> {
    let cb = get_codebase(con, codebase)?;
    let normalized = target.strip_prefix("symbol:").unwrap_or(target);
    let symbols = query_json(
        con,
        r#"
        SELECT kind, name, qualname, module, file_path, line_start, line_end, basis, confidence
        FROM symbols
        WHERE codebase_id=?1 AND (qualname=?2 OR name=?2 OR file_path=?2)
        ORDER BY kind, module, file_path, line_start
        LIMIT 100
        "#,
        params![cb.id, normalized],
    )?;
    let outgoing = query_json(
        con,
        r#"
        SELECT source_kind, source, edge_type, target_kind, target, file_path, line_start, confidence, basis
        FROM graph_edges
        WHERE codebase_id=?1 AND source=?2
        ORDER BY edge_type, target_kind, target
        LIMIT 200
        "#,
        params![cb.id, normalized],
    )?;
    let incoming = query_json(
        con,
        r#"
        SELECT source_kind, source, edge_type, target_kind, target, file_path, line_start, confidence, basis
        FROM graph_edges
        WHERE codebase_id=?1 AND target=?2
        ORDER BY edge_type, source_kind, source
        LIMIT 200
        "#,
        params![cb.id, normalized],
    )?;
    let file_paths = symbols
        .iter()
        .filter_map(|symbol| symbol.get("file_path").and_then(Value::as_str))
        .collect::<HashSet<_>>();
    let mut related_symbols = Vec::new();
    for file_path in file_paths {
        related_symbols.extend(query_json(
            con,
            r#"
            SELECT kind, name, qualname, module, file_path, line_start, line_end
            FROM symbols
            WHERE codebase_id=?1 AND file_path=?2 AND NOT (qualname=?3 OR name=?3)
            ORDER BY line_start, kind, name
            LIMIT 100
            "#,
            params![cb.id, file_path, normalized],
        )?);
    }
    let found = !(symbols.is_empty() && outgoing.is_empty() && incoming.is_empty());
    Ok(json!({
        "codebase": cb_meta(&cb),
        "target": target,
        "normalized_target": normalized,
        "matches": symbols,
        "outgoing_edges": outgoing,
        "incoming_edges": incoming,
        "related_symbols": related_symbols,
        "profile": "all_source",
        "basis": "sqlite_graph",
        "confidence": if found { "medium" } else { "low" },
        "note": "Static impact analysis is an approximation from indexed symbols and graph edges, not exact runtime Odoo behavior."
    }))
}

pub fn context_bundle(
    con: &Connection,
    query: &str,
    codebase: Option<&str>,
    module: Option<&str>,
    limit: usize,
) -> Result<Value> {
    let cb = get_codebase(con, codebase)?;
    let effective_limit = limit.clamp(1, 50);
    let search_response = search(con, query, Some(&cb.name), module, effective_limit)?;
    let symbol_names = search_response
        .results
        .symbols
        .iter()
        .filter_map(|symbol| symbol.qualname.as_deref().or(Some(symbol.name.as_str())))
        .take(8)
        .map(str::to_string)
        .collect::<Vec<_>>();

    let mut impacts = Vec::new();
    for symbol_name in &symbol_names {
        impacts.push(impact_analysis(con, symbol_name, Some(&cb.name))?);
    }

    Ok(json!({
        "codebase": cb_meta(&cb),
        "query": query,
        "module_filter": module,
        "search": serde_json::to_value(search_response)?,
        "symbol_details": query_json(
            con,
            r#"
            SELECT kind, name, qualname, module, file_path, line_start, line_end, basis, confidence
            FROM symbols
            WHERE codebase_id=?1 AND (name=?2 OR qualname=?2 OR module=?2)
            ORDER BY kind, module, file_path, line_start
            LIMIT 100
            "#,
            params![cb.id, query],
        )?,
        "chunk_samples": query_json(
            con,
            r#"
            SELECT module, symbol_kind, symbol_name, text, file_path, line_start, line_end
            FROM chunks
            WHERE codebase_id=?1 AND (symbol_name=?2 OR text LIKE '%' || ?2 || '%')
            ORDER BY module, file_path, line_start
            LIMIT 20
            "#,
            params![cb.id, query],
        )?,
        "impact_samples": impacts,
        "diagnostics_sample": query_json(
            con,
            r#"
            SELECT severity, kind, message, file_path, line_start
            FROM index_diagnostics
            WHERE codebase_id=?1
            ORDER BY severity, kind, file_path
            LIMIT 20
            "#,
            params![cb.id],
        )?,
        "profile": "all_source",
        "basis": "sqlite_fts5+sqlite_graph",
        "confidence": "medium",
        "note": "Context bundle combines indexed static facts for agent debugging or implementation. It is not exact runtime Odoo behavior."
    }))
}

pub fn trace_business_flow(
    con: &Connection,
    model_name: &str,
    method_name: &str,
    codebase: Option<&str>,
) -> Result<Value> {
    let cb = get_codebase(con, codebase)?;
    let chain = method_chain(con, model_name, method_name, Some(&cb.name))?;
    let method_key = format!("{model_name}.{method_name}");
    Ok(json!({
        "codebase": cb_meta(&cb),
        "model": model_name,
        "method": method_name,
        "method_chain": chain,
        "related_edges": query_json(
            con,
            r#"
            SELECT source_kind, source, edge_type, target_kind, target, file_path, line_start, confidence, basis
            FROM graph_edges
            WHERE codebase_id=?1 AND (source=?2 OR target=?2 OR source=?3 OR target=?3)
            ORDER BY source_kind, source, edge_type, target_kind, target
            LIMIT 200
            "#,
            params![cb.id, model_name, method_key],
        )?,
        "callers_or_mentions": query_json(
            con,
            r#"
            SELECT module, model_name, class_name, method_name, decorators, calls_super, calls, file_path, line_start, line_end
            FROM methods
            WHERE codebase_id=?1 AND calls LIKE '%' || ?2 || '%'
            ORDER BY module, model_name, method_name
            LIMIT 100
            "#,
            params![cb.id, method_name],
        )?,
        "views_for_model": query_json(
            con,
            "SELECT xmlid, module, inherit_id, xpath_count, file_path, line_start FROM views WHERE codebase_id=?1 AND view_model=?2 ORDER BY module, xmlid LIMIT 100",
            params![cb.id, model_name],
        )?,
        "basis": "tree_sitter_python_parse+sqlite_graph",
        "confidence": "medium",
        "note": "Business flow trace is a static approximation from method chains, calls, views, and graph edges; it is not exact runtime Odoo behavior."
    }))
}

pub fn find_extension_point(
    con: &Connection,
    goal: &str,
    codebase: Option<&str>,
    module: Option<&str>,
) -> Result<Value> {
    let cb = get_codebase(con, codebase)?;
    let search_response = search(con, goal, Some(&cb.name), module, 20)?;
    Ok(json!({
        "codebase": cb_meta(&cb),
        "goal": goal,
        "module_filter": module,
        "candidate_models": query_json(
            con,
            r#"
            SELECT module, model_name, class_name, inherit, inherits, file_path, line_start, line_end
            FROM models
            WHERE codebase_id=?1 AND (model_name LIKE '%' || ?2 || '%' OR class_name LIKE '%' || ?2 || '%' OR module LIKE '%' || ?2 || '%')
            ORDER BY module, model_name
            LIMIT 50
            "#,
            params![cb.id, goal],
        )?,
        "candidate_methods": query_json(
            con,
            r#"
            SELECT module, model_name, class_name, method_name, decorators, calls_super, file_path, line_start, line_end
            FROM methods
            WHERE codebase_id=?1 AND (method_name LIKE '%' || ?2 || '%' OR model_name LIKE '%' || ?2 || '%' OR calls LIKE '%' || ?2 || '%')
            ORDER BY module, model_name, method_name
            LIMIT 100
            "#,
            params![cb.id, goal],
        )?,
        "candidate_views": query_json(
            con,
            r#"
            SELECT module, xmlid, view_model, inherit_id, xpath_count, file_path, line_start
            FROM views
            WHERE codebase_id=?1 AND (xmlid LIKE '%' || ?2 || '%' OR view_model LIKE '%' || ?2 || '%' OR inherit_id LIKE '%' || ?2 || '%')
            ORDER BY module, xmlid
            LIMIT 100
            "#,
            params![cb.id, goal],
        )?,
        "search": serde_json::to_value(search_response)?,
        "basis": "sqlite_fts5+sqlite_graph",
        "confidence": "medium",
        "note": "Extension points are static candidates. Verify against Odoo runtime semantics before implementation."
    }))
}

pub fn debug_hypotheses(
    con: &Connection,
    symptom: &str,
    codebase: Option<&str>,
    module: Option<&str>,
) -> Result<Value> {
    let cb = get_codebase(con, codebase)?;
    let bundle = context_bundle(con, symptom, Some(&cb.name), module, 10)?;
    Ok(json!({
        "codebase": cb_meta(&cb),
        "symptom": symptom,
        "module_filter": module,
        "hypotheses": [
            {"kind": "override_chain", "description": "A method override or missing super() call may alter the expected flow.", "check": "Inspect method_chain and callers_or_mentions for the affected model/method."},
            {"kind": "view_inheritance", "description": "A view inheritance or xpath may hide, move, or alter fields/actions.", "check": "Inspect candidate views and inherited XMLIDs."},
            {"kind": "module_dependency", "description": "Module load order or missing dependency source may affect availability.", "check": "Inspect module dependencies and index diagnostics."},
            {"kind": "frontend_patch", "description": "A POS/Owl registry entry or patch may alter client behavior.", "check": "Inspect frontend symbols and related JS chunks."}
        ],
        "context_bundle": bundle,
        "basis": "static_indexed_context",
        "confidence": "medium",
        "note": "Debug hypotheses are generated from static indexed facts and require runtime verification."
    }))
}

pub fn compare_symbol(
    con: &Connection,
    symbol: &str,
    left_codebase: &str,
    right_codebase: &str,
) -> Result<Value> {
    let left = get_codebase(con, Some(left_codebase))?;
    let right = get_codebase(con, Some(right_codebase))?;
    let left_matches = symbol_matches(con, left.id, symbol)?;
    let right_matches = symbol_matches(con, right.id, symbol)?;
    Ok(json!({
        "symbol": symbol,
        "left": {"codebase": cb_meta(&left), "matches": left_matches},
        "right": {"codebase": cb_meta(&right), "matches": right_matches},
        "summary": {
            "left_count": left_matches.len(),
            "right_count": right_matches.len(),
            "status": if left_matches.is_empty() && right_matches.is_empty() { "missing_both" } else if left_matches.is_empty() { "missing_left" } else if right_matches.is_empty() { "missing_right" } else { "present_both" }
        },
        "basis": "sqlite_symbol_compare",
        "confidence": "medium",
        "note": "Symbol comparison uses static indexed facts across two indexed codebases."
    }))
}

#[derive(Debug, serde::Serialize)]
pub struct MaterializeStats {
    pub codebase: String,
    pub module_contexts: usize,
    pub xmlid_lookups: usize,
    pub payload_version: &'static str,
}

#[derive(Debug, serde::Serialize)]
pub struct MaterializedValidation {
    pub codebase: String,
    pub checked: usize,
    pub mismatches: usize,
    pub samples: Vec<String>,
}

pub fn materialize_contexts(con: &Connection, codebase: Option<&str>) -> Result<MaterializeStats> {
    let cb = get_codebase(con, codebase)?;
    con.execute_batch("BEGIN IMMEDIATE")?;
    let result = materialize_contexts_inner(con, &cb);
    if result.is_ok() {
        con.execute_batch("COMMIT")?;
    } else {
        let _ = con.execute_batch("ROLLBACK");
    }
    result
}

fn materialize_contexts_inner(con: &Connection, cb: &Codebase) -> Result<MaterializeStats> {
    let mut module_contexts = 0;
    for module_name in query_strings(
        con,
        "SELECT name FROM modules WHERE codebase_id=?1 ORDER BY name",
        params![cb.id],
    )? {
        let payload = module_context_live_for_cb(con, &cb, &module_name)?;
        store_materialized_payload(con, &cb, "odoo_module_context", &module_name, &payload)?;
        module_contexts += 1;
    }

    let mut xmlid_lookups = 0;
    for xmlid in query_strings(
        con,
        "SELECT DISTINCT xmlid FROM xml_records WHERE codebase_id=?1 ORDER BY xmlid",
        params![cb.id],
    )? {
        let payload = xmlid_lookup_live_for_cb(con, &cb, &xmlid)?;
        store_materialized_payload(con, &cb, "odoo_xmlid_lookup", &xmlid, &payload)?;
        xmlid_lookups += 1;
    }

    Ok(MaterializeStats {
        codebase: cb.name.clone(),
        module_contexts,
        xmlid_lookups,
        payload_version: MATERIALIZED_PAYLOAD_VERSION,
    })
}

pub fn validate_materialized_contexts(
    con: &Connection,
    codebase: Option<&str>,
    limit: usize,
) -> Result<MaterializedValidation> {
    let cb = get_codebase(con, codebase)?;
    let mut checked = 0;
    let mut samples = Vec::new();

    for module_name in query_strings(
        con,
        "SELECT name FROM modules WHERE codebase_id=?1 ORDER BY name LIMIT ?2",
        params![cb.id, limit as i64],
    )? {
        let live = module_context_live_for_cb(con, &cb, &module_name)?;
        let cached = materialized_payload(con, &cb, "odoo_module_context", &module_name)?;
        checked += 1;
        if cached.as_ref() != Some(&live) {
            samples.push(format!("odoo_module_context:{module_name}"));
        }
    }

    for xmlid in query_strings(
        con,
        "SELECT DISTINCT xmlid FROM xml_records WHERE codebase_id=?1 ORDER BY xmlid LIMIT ?2",
        params![cb.id, limit as i64],
    )? {
        let live = xmlid_lookup_live_for_cb(con, &cb, &xmlid)?;
        let cached = materialized_payload(con, &cb, "odoo_xmlid_lookup", &xmlid)?;
        checked += 1;
        if cached.as_ref() != Some(&live) {
            samples.push(format!("odoo_xmlid_lookup:{xmlid}"));
        }
    }

    Ok(MaterializedValidation {
        codebase: cb.name,
        checked,
        mismatches: samples.len(),
        samples,
    })
}

fn materialized_payload(
    con: &Connection,
    cb: &Codebase,
    tool_name: &str,
    cache_key: &str,
) -> Result<Option<Value>> {
    let source_indexed_at = cb.indexed_at.as_deref();
    let payload_json = con
        .query_row(
            r#"
            SELECT payload_json
            FROM materialized_tool_contexts
            WHERE codebase_id=?1
              AND tool_name=?2
              AND cache_key=?3
              AND payload_version=?4
              AND (source_indexed_at IS ?5 OR source_indexed_at = ?5)
            "#,
            params![
                cb.id,
                tool_name,
                cache_key,
                MATERIALIZED_PAYLOAD_VERSION,
                source_indexed_at
            ],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    payload_json
        .map(|text| serde_json::from_str(&text).map_err(Error::from))
        .transpose()
}

fn store_materialized_payload(
    con: &Connection,
    cb: &Codebase,
    tool_name: &str,
    cache_key: &str,
    payload: &Value,
) -> Result<()> {
    con.execute(
        r#"
        INSERT INTO materialized_tool_contexts(
            codebase_id, tool_name, cache_key, payload_version, source_indexed_at, payload_json, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, CURRENT_TIMESTAMP)
        ON CONFLICT(codebase_id, tool_name, cache_key) DO UPDATE SET
            payload_version=excluded.payload_version,
            source_indexed_at=excluded.source_indexed_at,
            payload_json=excluded.payload_json,
            created_at=CURRENT_TIMESTAMP
        "#,
        params![
            cb.id,
            tool_name,
            cache_key,
            MATERIALIZED_PAYLOAD_VERSION,
            cb.indexed_at.as_deref(),
            serde_json::to_string(payload)?
        ],
    )?;
    Ok(())
}

fn symbol_matches(con: &Connection, codebase_id: i64, symbol: &str) -> Result<Vec<Value>> {
    query_json(
        con,
        r#"
        SELECT kind, name, qualname, module, file_path, line_start, line_end, basis, confidence
        FROM symbols
        WHERE codebase_id=?1 AND (name=?2 OR qualname=?2 OR file_path=?2)
        ORDER BY kind, module, file_path, line_start
        LIMIT 200
        "#,
        params![codebase_id, symbol],
    )
}

fn module_load_order(con: &Connection, codebase_id: i64) -> Result<HashMap<String, usize>> {
    let modules = query_strings(
        con,
        "SELECT name FROM modules WHERE codebase_id=?1",
        params![codebase_id],
    )?;
    let module_set: HashSet<String> = modules.iter().cloned().collect();
    let mut deps: HashMap<String, HashSet<String>> = modules
        .iter()
        .map(|module| (module.clone(), HashSet::new()))
        .collect();
    let mut stmt =
        con.prepare("SELECT module, depends_on FROM module_dependencies WHERE codebase_id=?1")?;
    let rows = stmt.query_map([codebase_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (module, depends_on) = row?;
        if module_set.contains(&module) && module_set.contains(&depends_on) {
            deps.entry(module).or_default().insert(depends_on);
        }
    }
    let mut pending: HashSet<String> = module_set;
    let mut order = HashMap::new();
    let mut idx = 0;
    while !pending.is_empty() {
        let mut ready: Vec<String> = pending
            .iter()
            .filter(|module| {
                deps.get(*module)
                    .is_none_or(|deps| deps.is_disjoint(&pending))
            })
            .cloned()
            .collect();
        ready.sort();
        if ready.is_empty() {
            ready.push(pending.iter().min().cloned().unwrap_or_default());
        }
        for module in ready {
            pending.remove(&module);
            order.insert(module, idx);
            idx += 1;
        }
    }
    Ok(order)
}

fn query_strings<P: rusqlite::Params>(
    con: &Connection,
    sql: &str,
    params: P,
) -> Result<Vec<String>> {
    let mut stmt = con.prepare(sql)?;
    let rows = stmt.query_map(params, |row| row.get::<_, String>(0))?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn query_one_json<P: rusqlite::Params>(
    con: &Connection,
    sql: &str,
    params: P,
) -> Result<Option<Value>> {
    let rows = query_json(con, sql, params)?;
    Ok(rows.into_iter().next())
}

fn query_json<P: rusqlite::Params>(con: &Connection, sql: &str, params: P) -> Result<Vec<Value>> {
    let mut stmt = con.prepare(sql)?;
    let names: Vec<String> = stmt
        .column_names()
        .iter()
        .map(|name| name.to_string())
        .collect();
    let rows = stmt.query_map(params, |row| {
        let mut object = serde_json::Map::new();
        for (idx, name) in names.iter().enumerate() {
            let value = row.get_ref(idx)?;
            object.insert(name.clone(), sqlite_value(value));
        }
        Ok(Value::Object(object))
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn sqlite_value(value: rusqlite::types::ValueRef<'_>) -> Value {
    use rusqlite::types::ValueRef;
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => json!(value),
        ValueRef::Real(value) => json!(value),
        ValueRef::Text(value) => json!(String::from_utf8_lossy(value)),
        ValueRef::Blob(_) => Value::Null,
    }
}

fn cb_meta(cb: &Codebase) -> Value {
    json!({
        "name": cb.name,
        "series": cb.odoo_series,
        "version": cb.version,
        "branch": cb.git_branch,
        "commit": cb.git_commit,
        "root_path": cb.root_path.to_string_lossy()
    })
}
