use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection};
use serde_json::{json, Value};

use crate::codebase::{get_codebase, Codebase};
use crate::error::Result;

pub fn module_context(
    con: &Connection,
    module_name: &str,
    codebase: Option<&str>,
) -> Result<Value> {
    let cb = get_codebase(con, codebase)?;
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
        "basis": "heuristic_python_parse",
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
        "note": "Order approximates Odoo override order by reversing dependency-based module load order; exact registry MRO may differ.",
        "basis": "heuristic_python_parse",
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
        "basis": "exact_xml_parse",
        "confidence": if views.is_empty() { "low" } else { "high" }
    }))
}

pub fn xmlid_lookup(con: &Connection, xmlid: &str, codebase: Option<&str>) -> Result<Value> {
    let cb = get_codebase(con, codebase)?;
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
