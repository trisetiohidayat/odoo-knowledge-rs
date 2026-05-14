use std::collections::HashMap;
use std::path::Path;

use chrono::Utc;
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::codebase::get_codebase;
use crate::error::Result;
use crate::parsers::csv_access::parse_security_csv;
use crate::parsers::javascript::parse_js_file;
use crate::parsers::manifest::{parse_manifest, Manifest};
use crate::parsers::python::parse_python_file;
use crate::parsers::xml::parse_xml_file;
use crate::scanner::classify::classify_file;
use crate::scanner::file_scan::relevant_files;
use crate::scanner::manifest_scan::find_manifests;
use sha1::{Digest, Sha1};

#[derive(Debug, Default, Serialize)]
pub struct IndexStats {
    pub modules: usize,
    pub files: usize,
    pub models: usize,
    pub fields: usize,
    pub methods: usize,
    pub xml_records: usize,
    pub views: usize,
    pub frontend: usize,
}

pub fn index_codebase(con: &Connection, codebase_name: &str) -> Result<IndexStats> {
    con.execute_batch("BEGIN IMMEDIATE")?;
    let result = index_codebase_inner(con, codebase_name);
    match result {
        Ok(stats) => {
            con.execute_batch("COMMIT")?;
            Ok(stats)
        }
        Err(err) => {
            let _ = con.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}

fn index_codebase_inner(con: &Connection, codebase_name: &str) -> Result<IndexStats> {
    let codebase = get_codebase(con, Some(codebase_name))?;
    clear_codebase(con, codebase.id)?;

    let parsed = find_manifests(&codebase.root_path)
        .into_iter()
        .map(|path| parse_manifest(&path))
        .collect::<Result<Vec<_>>>()?;
    let (manifests, duplicates) = dedupe_manifests(parsed);
    let module_names: std::collections::HashSet<String> = manifests
        .iter()
        .map(|manifest| manifest.module.clone())
        .collect();

    for (chosen, skipped) in duplicates {
        con.execute(
            r#"
            INSERT INTO index_diagnostics(codebase_id, severity, kind, message, file_path)
            VALUES (?1, 'warning', 'duplicate_module_manifest', ?2, ?3)
            "#,
            params![
                codebase.id,
                format!(
                    "Skipped duplicate manifest for module {}; using {}",
                    skipped.module,
                    rel_path(&chosen.manifest_path, &codebase.root_path)
                ),
                rel_path(&skipped.manifest_path, &codebase.root_path)
            ],
        )?;
    }

    let mut stats = IndexStats::default();
    for manifest in manifests {
        insert_module(con, codebase.id, &codebase.root_path, &manifest)?;
        insert_symbol(
            con,
            codebase.id,
            Some(&manifest.module),
            "module",
            &manifest.module,
            &format!("module:{}", manifest.module),
            &rel_path(&manifest.manifest_path, &codebase.root_path),
            1,
            1,
            "exact_manifest_parse",
            "high",
        )?;
        insert_chunk(
            con,
            codebase.id,
            Some(&manifest.module),
            "module",
            &manifest.module,
            &serde_json::to_string(&manifest)?,
            &rel_path(&manifest.manifest_path, &codebase.root_path),
        )?;
        stats.modules += 1;

        for dep in &manifest.depends {
            con.execute(
                "INSERT OR IGNORE INTO module_dependencies(codebase_id, module, depends_on) VALUES (?1, ?2, ?3)",
                params![codebase.id, manifest.module, dep],
            )?;
            con.execute(
                r#"
                INSERT INTO graph_edges(codebase_id, source_kind, source, edge_type, target_kind, target, file_path, confidence, basis)
                VALUES (?1, 'module', ?2, 'depends_on', 'module', ?3, ?4, 'high', 'exact_manifest_parse')
                "#,
                params![
                    codebase.id,
                    manifest.module,
                    dep,
                    rel_path(&manifest.manifest_path, &codebase.root_path)
                ],
            )?;
            if !module_names.contains(dep) {
                con.execute(
                    r#"
                    INSERT INTO index_diagnostics(codebase_id, severity, kind, message, file_path)
                    VALUES (?1, 'warning', 'missing_dependency_source', ?2, ?3)
                    "#,
                    params![
                        codebase.id,
                        format!(
                            "{} depends on {}, but source module was not found",
                            manifest.module, dep
                        ),
                        rel_path(&manifest.manifest_path, &codebase.root_path)
                    ],
                )?;
            }
        }

        for path in relevant_files(&manifest.path) {
            let file_rel = rel_path(&path, &codebase.root_path);
            let (language, role) = classify_file(&path);
            con.execute(
                r#"
                INSERT OR REPLACE INTO files(codebase_id, module, path, rel_path, language, role, sha1)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
                params![
                    codebase.id,
                    manifest.module,
                    path.to_string_lossy(),
                    file_rel,
                    language,
                    role,
                    sha1_file(&path)?
                ],
            )?;
            con.execute(
                r#"
                INSERT INTO graph_edges(codebase_id, source_kind, source, edge_type, target_kind, target, file_path, confidence, basis)
                VALUES (?1, 'module', ?2, 'owns_file', 'file', ?3, ?4, 'high', 'scanner')
                "#,
                params![codebase.id, manifest.module, file_rel, file_rel],
            )?;
            stats.files += 1;

            match path.extension().and_then(|value| value.to_str()) {
                Some("csv") => {
                    for rule in parse_security_csv(&path, &manifest.module, &codebase.root_path)? {
                        con.execute(
                            r#"
                            INSERT INTO security_rules(codebase_id, module, kind, name, model_ref, group_ref, permissions, file_path, line_start)
                            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                            "#,
                            params![
                                codebase.id,
                                rule.module,
                                rule.kind,
                                rule.name,
                                rule.model_ref,
                                rule.group_ref,
                                rule.permissions,
                                rule.file_path,
                                rule.line_start as i64
                            ],
                        )?;
                    }
                }
                Some("js") => {
                    for symbol in parse_js_file(&path, &manifest.module, &codebase.root_path)? {
                        con.execute(
                            r#"
                            INSERT INTO frontend_symbols(codebase_id, module, kind, name, target, category, file_path, line_start, line_end)
                            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                            "#,
                            params![
                                codebase.id,
                                &symbol.module,
                                &symbol.kind,
                                &symbol.name,
                                &symbol.target,
                                &symbol.category,
                                &symbol.file_path,
                                symbol.line_start as i64,
                                symbol.line_end as i64
                            ],
                        )?;
                        insert_symbol(
                            con,
                            codebase.id,
                            Some(&manifest.module),
                            &symbol.kind,
                            &symbol.name,
                            &format!("{}:{}", symbol.kind, symbol.name),
                            &symbol.file_path,
                            symbol.line_start as i64,
                            symbol.line_end as i64,
                            "string_js_parse",
                            "medium",
                        )?;
                        stats.frontend += 1;
                    }
                }
                Some("xml") => {
                    let parsed = parse_xml_file(&path, &manifest.module, &codebase.root_path);
                    for err in parsed.errors {
                        con.execute(
                            r#"
                            INSERT INTO index_diagnostics(codebase_id, severity, kind, message, file_path)
                            VALUES (?1, 'error', 'xml_parse', ?2, ?3)
                            "#,
                            params![codebase.id, err, &file_rel],
                        )?;
                    }
                    for rec in parsed.records {
                        con.execute(
                            r#"
                            INSERT OR IGNORE INTO xml_records(codebase_id, module, xmlid, record_model, file_path, line_start, line_end)
                            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                            "#,
                            params![
                                codebase.id,
                                &rec.module,
                                &rec.xmlid,
                                &rec.record_model,
                                &rec.file_path,
                                rec.line_start as i64,
                                rec.line_end as i64
                            ],
                        )?;
                        insert_symbol(
                            con,
                            codebase.id,
                            Some(&manifest.module),
                            "xmlid",
                            &rec.xmlid,
                            &format!("xmlid:{}", rec.xmlid),
                            &rec.file_path,
                            rec.line_start as i64,
                            rec.line_end as i64,
                            "exact_xml_parse",
                            "high",
                        )?;
                        stats.xml_records += 1;
                    }
                    for view in parsed.views {
                        con.execute(
                            r#"
                            INSERT INTO views(codebase_id, module, xmlid, view_model, inherit_id, priority, xpath_count, file_path, line_start, line_end)
                            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                            "#,
                            params![
                                codebase.id,
                                &view.module,
                                &view.xmlid,
                                &view.view_model,
                                &view.inherit_id,
                                &view.priority,
                                view.xpath_count as i64,
                                &view.file_path,
                                view.line_start as i64,
                                view.line_end as i64
                            ],
                        )?;
                        if let Some(xmlid) = &view.xmlid {
                            insert_symbol(
                                con,
                                codebase.id,
                                Some(&manifest.module),
                                "view",
                                xmlid,
                                &format!("view:{xmlid}"),
                                &view.file_path,
                                view.line_start as i64,
                                view.line_end as i64,
                                "exact_xml_parse",
                                "high",
                            )?;
                        }
                        if let Some(view_model) = &view.view_model {
                            con.execute(
                                r#"
                                INSERT INTO graph_edges(codebase_id, source_kind, source, edge_type, target_kind, target, file_path, line_start, confidence, basis)
                                VALUES (?1, 'view', ?2, 'targets_model', 'model', ?3, ?4, ?5, 'high', 'exact_xml_parse')
                                "#,
                                params![
                                    codebase.id,
                                    view.xmlid.as_deref().unwrap_or(""),
                                    view_model,
                                    view.file_path,
                                    view.line_start as i64
                                ],
                            )?;
                        }
                        if let Some(inherit_id) = &view.inherit_id {
                            con.execute(
                                r#"
                                INSERT INTO graph_edges(codebase_id, source_kind, source, edge_type, target_kind, target, file_path, line_start, confidence, basis)
                                VALUES (?1, 'view', ?2, 'inherits_view', 'xmlid', ?3, ?4, ?5, 'high', 'exact_xml_parse')
                                "#,
                                params![
                                    codebase.id,
                                    view.xmlid.as_deref().unwrap_or(""),
                                    inherit_id,
                                    view.file_path,
                                    view.line_start as i64
                                ],
                            )?;
                        }
                        stats.views += 1;
                    }
                    for action in parsed.actions {
                        con.execute(
                            r#"
                            INSERT INTO actions(codebase_id, module, xmlid, action_model, res_model, view_id, file_path, line_start)
                            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                            "#,
                            params![
                                codebase.id,
                                action.module,
                                action.xmlid,
                                action.action_model,
                                action.res_model,
                                action.view_id,
                                action.file_path,
                                action.line_start as i64
                            ],
                        )?;
                    }
                    for menu in parsed.menus {
                        con.execute(
                            r#"
                            INSERT INTO menus(codebase_id, module, xmlid, action_ref, parent_ref, file_path, line_start)
                            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                            "#,
                            params![
                                codebase.id,
                                menu.module,
                                menu.xmlid,
                                menu.action_ref,
                                menu.parent_ref,
                                menu.file_path,
                                menu.line_start as i64
                            ],
                        )?;
                    }
                }
                Some("py") => {
                    let parsed = parse_python_file(&path, &manifest.module, &codebase.root_path)?;
                    for err in parsed.errors {
                        con.execute(
                            r#"
                            INSERT INTO index_diagnostics(codebase_id, severity, kind, message, file_path)
                            VALUES (?1, 'error', 'python_parse', ?2, ?3)
                            "#,
                            params![codebase.id, err, &file_rel],
                        )?;
                    }
                    for model in parsed.models {
                        let Some(model_name) = &model.model_name else {
                            continue;
                        };
                        let inherit_json = serde_json::to_string(&model.inherit)?;
                        con.execute(
                            r#"
                            INSERT INTO models(codebase_id, module, model_name, class_name, inherit, inherits, file_path, line_start, line_end)
                            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                            "#,
                            params![
                                codebase.id,
                                &model.module,
                                model_name,
                                &model.class_name,
                                inherit_json,
                                &model.inherits,
                                &model.file_path,
                                model.line_start as i64,
                                model.line_end as i64
                            ],
                        )?;
                        insert_symbol(
                            con,
                            codebase.id,
                            Some(&manifest.module),
                            "model",
                            model_name,
                            &format!("model:{model_name}"),
                            &model.file_path,
                            model.line_start as i64,
                            model.line_end as i64,
                            "heuristic_python_parse",
                            "medium",
                        )?;
                        con.execute(
                            r#"
                            INSERT INTO graph_edges(codebase_id, source_kind, source, edge_type, target_kind, target, file_path, line_start, confidence, basis)
                            VALUES (?1, 'module', ?2, 'defines_or_extends', 'model', ?3, ?4, ?5, 'medium', 'heuristic_python_parse')
                            "#,
                            params![
                                codebase.id,
                                &model.module,
                                model_name,
                                &model.file_path,
                                model.line_start as i64
                            ],
                        )?;
                        for parent in &model.inherit {
                            con.execute(
                                r#"
                                INSERT INTO graph_edges(codebase_id, source_kind, source, edge_type, target_kind, target, file_path, line_start, confidence, basis)
                                VALUES (?1, 'model', ?2, 'inherits', 'model', ?3, ?4, ?5, 'medium', 'heuristic_python_parse')
                                "#,
                                params![
                                    codebase.id,
                                    model_name,
                                    parent,
                                    &model.file_path,
                                    model.line_start as i64
                                ],
                            )?;
                        }
                        stats.models += 1;
                    }
                    for field in parsed.fields {
                        con.execute(
                            r#"
                            INSERT INTO fields(codebase_id, module, model_name, field_name, field_type, comodel, compute, inverse, search, related, file_path, line_start, line_end)
                            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                            "#,
                            params![
                                codebase.id,
                                &field.module,
                                &field.model_name,
                                &field.field_name,
                                &field.field_type,
                                &field.comodel,
                                &field.compute,
                                &field.inverse,
                                &field.search,
                                &field.related,
                                &field.file_path,
                                field.line_start as i64,
                                field.line_end as i64
                            ],
                        )?;
                        let qualname = field
                            .model_name
                            .as_ref()
                            .map(|model| format!("field:{model}.{}", field.field_name))
                            .unwrap_or_else(|| format!("field:{}", field.field_name));
                        insert_symbol(
                            con,
                            codebase.id,
                            Some(&manifest.module),
                            "field",
                            &field.field_name,
                            &qualname,
                            &field.file_path,
                            field.line_start as i64,
                            field.line_end as i64,
                            "heuristic_python_parse",
                            "medium",
                        )?;
                        if let Some(model_name) = &field.model_name {
                            con.execute(
                                r#"
                                INSERT INTO graph_edges(codebase_id, source_kind, source, edge_type, target_kind, target, file_path, line_start, confidence, basis)
                                VALUES (?1, 'model', ?2, 'defines_field', 'field', ?3, ?4, ?5, 'medium', 'heuristic_python_parse')
                                "#,
                                params![
                                    codebase.id,
                                    model_name,
                                    format!("{model_name}.{}", field.field_name),
                                    &field.file_path,
                                    field.line_start as i64
                                ],
                            )?;
                        }
                        stats.fields += 1;
                    }
                    for method in parsed.methods {
                        let decorators = serde_json::to_string(&method.decorators)?;
                        let calls = serde_json::to_string(&method.calls)?;
                        con.execute(
                            r#"
                            INSERT INTO methods(codebase_id, module, model_name, class_name, method_name, decorators, calls_super, calls, file_path, line_start, line_end)
                            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                            "#,
                            params![
                                codebase.id,
                                &method.module,
                                &method.model_name,
                                &method.class_name,
                                &method.method_name,
                                decorators,
                                method.calls_super as i64,
                                calls,
                                &method.file_path,
                                method.line_start as i64,
                                method.line_end as i64
                            ],
                        )?;
                        let qualname = method
                            .model_name
                            .as_ref()
                            .map(|model| format!("method:{model}.{}", method.method_name))
                            .unwrap_or_else(|| {
                                format!("method:{}.{}", method.class_name, method.method_name)
                            });
                        insert_symbol(
                            con,
                            codebase.id,
                            Some(&manifest.module),
                            "method",
                            &method.method_name,
                            &qualname,
                            &method.file_path,
                            method.line_start as i64,
                            method.line_end as i64,
                            "heuristic_python_parse",
                            "medium",
                        )?;
                        insert_chunk(
                            con,
                            codebase.id,
                            Some(&manifest.module),
                            "method",
                            &qualname,
                            &format!(
                                "{}.{} decorators={:?} calls={:?}",
                                method.model_name.as_deref().unwrap_or(&method.class_name),
                                method.method_name,
                                method.decorators,
                                method.calls
                            ),
                            &method.file_path,
                        )?;
                        if let Some(model_name) = &method.model_name {
                            con.execute(
                                r#"
                                INSERT INTO graph_edges(codebase_id, source_kind, source, edge_type, target_kind, target, file_path, line_start, confidence, basis)
                                VALUES (?1, 'model', ?2, 'defines_method', 'method', ?3, ?4, ?5, 'medium', 'heuristic_python_parse')
                                "#,
                                params![
                                    codebase.id,
                                    model_name,
                                    format!("{model_name}.{}", method.method_name),
                                    &method.file_path,
                                    method.line_start as i64
                                ],
                            )?;
                        }
                        stats.methods += 1;
                    }
                }
                _ => {}
            }
        }
    }

    create_default_profiles(con, codebase.id)?;
    con.execute(
        "UPDATE codebases SET indexed_at=?1 WHERE id=?2",
        params![Utc::now().to_rfc3339(), codebase.id],
    )?;
    Ok(stats)
}

fn clear_codebase(con: &Connection, codebase_id: i64) -> Result<()> {
    let profile_ids = {
        let mut stmt = con.prepare("SELECT id FROM profiles WHERE codebase_id=?1")?;
        let rows = stmt.query_map([codebase_id], |row| row.get::<_, i64>(0))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    for profile_id in profile_ids {
        con.execute(
            "DELETE FROM profile_modules WHERE profile_id=?1",
            [profile_id],
        )?;
    }

    for table in [
        "modules",
        "module_dependencies",
        "profiles",
        "files",
        "symbols",
        "models",
        "fields",
        "methods",
        "xml_records",
        "views",
        "actions",
        "menus",
        "security_rules",
        "frontend_symbols",
        "graph_edges",
        "chunks",
        "index_diagnostics",
    ] {
        con.execute(
            &format!("DELETE FROM {table} WHERE codebase_id=?1"),
            [codebase_id],
        )?;
    }
    con.execute(
        "DELETE FROM fts_symbols WHERE codebase_id=?1",
        [codebase_id],
    )?;
    con.execute("DELETE FROM fts_chunks WHERE codebase_id=?1", [codebase_id])?;
    Ok(())
}

fn dedupe_manifests(manifests: Vec<Manifest>) -> (Vec<Manifest>, Vec<(Manifest, Manifest)>) {
    let mut by_module: HashMap<String, Manifest> = HashMap::new();
    let mut duplicates = Vec::new();
    for manifest in manifests {
        let Some(existing) = by_module.get(&manifest.module).cloned() else {
            by_module.insert(manifest.module.clone(), manifest);
            continue;
        };
        let manifest_depth = manifest.manifest_path.components().count();
        let existing_depth = existing.manifest_path.components().count();
        let (chosen, skipped) = if manifest_depth < existing_depth {
            (manifest, existing)
        } else {
            (existing, manifest)
        };
        by_module.insert(chosen.module.clone(), chosen.clone());
        duplicates.push((chosen, skipped));
    }
    let mut values: Vec<Manifest> = by_module.into_values().collect();
    values.sort_by(|left, right| left.manifest_path.cmp(&right.manifest_path));
    (values, duplicates)
}

fn insert_module(
    con: &Connection,
    codebase_id: i64,
    root: &Path,
    manifest: &Manifest,
) -> Result<()> {
    con.execute(
        r#"
        INSERT INTO modules(codebase_id, name, path, manifest_path, installable, auto_install, application, summary)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
        params![
            codebase_id,
            manifest.module,
            rel_path(&manifest.path, root),
            rel_path(&manifest.manifest_path, root),
            manifest.installable as i64,
            manifest.auto_install as i64,
            manifest.application as i64,
            manifest.summary
        ],
    )?;
    Ok(())
}

fn insert_symbol(
    con: &Connection,
    codebase_id: i64,
    module: Option<&str>,
    kind: &str,
    name: &str,
    qualname: &str,
    file_path: &str,
    line_start: i64,
    line_end: i64,
    basis: &str,
    confidence: &str,
) -> Result<()> {
    con.execute(
        r#"
        INSERT INTO symbols(codebase_id, module, kind, name, qualname, file_path, line_start, line_end, basis, confidence)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
        params![
            codebase_id,
            module,
            kind,
            name,
            qualname,
            file_path,
            line_start,
            line_end,
            basis,
            confidence
        ],
    )?;
    con.execute(
        r#"
        INSERT INTO fts_symbols(codebase_id, kind, name, qualname, module, file_path)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
        params![
            codebase_id,
            kind,
            name,
            qualname,
            module.unwrap_or(""),
            file_path
        ],
    )?;
    Ok(())
}

fn insert_chunk(
    con: &Connection,
    codebase_id: i64,
    module: Option<&str>,
    kind: &str,
    name: &str,
    text: &str,
    file_path: &str,
) -> Result<()> {
    con.execute(
        r#"
        INSERT INTO chunks(codebase_id, module, symbol_kind, symbol_name, text, file_path, line_start, line_end)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 1)
        "#,
        params![codebase_id, module, kind, name, text, file_path],
    )?;
    con.execute(
        r#"
        INSERT INTO fts_chunks(codebase_id, module, symbol_kind, symbol_name, text, file_path)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
        params![
            codebase_id,
            module.unwrap_or(""),
            kind,
            name,
            text,
            file_path
        ],
    )?;
    Ok(())
}

fn create_default_profiles(con: &Connection, codebase_id: i64) -> Result<()> {
    let modules = {
        let mut stmt = con.prepare("SELECT name FROM modules WHERE codebase_id=?1")?;
        let rows = stmt.query_map([codebase_id], |row| row.get::<_, String>(0))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    insert_profile(con, codebase_id, "all_source", &modules)?;
    let base_web: Vec<String> = ["base", "web"]
        .into_iter()
        .filter(|module| modules.iter().any(|existing| existing == module))
        .map(str::to_string)
        .collect();
    insert_profile(con, codebase_id, "base_web", &base_web)?;
    Ok(())
}

fn insert_profile(
    con: &Connection,
    codebase_id: i64,
    name: &str,
    modules: &[String],
) -> Result<()> {
    con.execute(
        "INSERT INTO profiles(codebase_id, name) VALUES (?1, ?2)",
        params![codebase_id, name],
    )?;
    let profile_id = con.last_insert_rowid();
    for module in modules {
        con.execute(
            "INSERT INTO profile_modules(profile_id, module) VALUES (?1, ?2)",
            params![profile_id, module],
        )?;
    }
    Ok(())
}

fn rel_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn sha1_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}
