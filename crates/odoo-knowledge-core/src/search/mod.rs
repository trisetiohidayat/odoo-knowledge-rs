use rusqlite::{params, Connection};
use serde::Serialize;

use crate::codebase::get_codebase;
use crate::error::Result;

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub codebase: SearchCodebase,
    pub query: String,
    pub results: SearchResults,
    pub basis: &'static str,
    pub confidence: &'static str,
}

#[derive(Debug, Serialize)]
pub struct SearchCodebase {
    pub name: String,
    pub series: Option<String>,
    pub version: Option<String>,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub root_path: String,
}

#[derive(Debug, Serialize)]
pub struct SearchResults {
    pub symbols: Vec<SearchSymbol>,
    pub chunks: Vec<SearchChunk>,
}

#[derive(Debug, Serialize)]
pub struct SearchSymbol {
    pub kind: String,
    pub name: String,
    pub qualname: Option<String>,
    pub module: Option<String>,
    pub file_path: Option<String>,
    pub rank: f64,
}

#[derive(Debug, Serialize)]
pub struct SearchChunk {
    pub kind: Option<String>,
    pub name: Option<String>,
    pub module: Option<String>,
    pub file_path: Option<String>,
    pub rank: f64,
}

pub fn search(
    con: &Connection,
    query: &str,
    codebase_name: Option<&str>,
    module: Option<&str>,
    limit: usize,
) -> Result<SearchResponse> {
    let codebase = get_codebase(con, codebase_name)?;
    let exact_symbols = exact_search_symbols(con, codebase.id, query, module, limit)?;
    let mut symbols = search_symbols(con, codebase.id, query, module, limit, "and")?;
    apply_odoo_ranking(query, &mut symbols);
    prepend_unique_symbols(&mut symbols, exact_symbols, limit);
    let mut chunks = search_chunks(con, codebase.id, query, module, limit, "and")?;
    if symbols.len() + chunks.len() < limit.min(5) {
        symbols = search_symbols(con, codebase.id, query, module, limit, "or")?;
        apply_odoo_ranking(query, &mut symbols);
        let exact_symbols = exact_search_symbols(con, codebase.id, query, module, limit)?;
        prepend_unique_symbols(&mut symbols, exact_symbols, limit);
        chunks = search_chunks(con, codebase.id, query, module, limit, "or")?;
    }

    Ok(SearchResponse {
        codebase: SearchCodebase {
            name: codebase.name,
            series: codebase.odoo_series,
            version: codebase.version,
            branch: codebase.git_branch,
            commit: codebase.git_commit,
            root_path: codebase.root_path.to_string_lossy().to_string(),
        },
        query: query.to_string(),
        results: SearchResults { symbols, chunks },
        basis: "sqlite_fts5",
        confidence: "medium",
    })
}

fn apply_odoo_ranking(query: &str, symbols: &mut [SearchSymbol]) {
    let query_lower = query.to_ascii_lowercase();
    for symbol in symbols.iter_mut() {
        symbol.rank += odoo_rank_adjustment(&query_lower, symbol);
    }
    symbols.sort_by(|left, right| {
        left.rank
            .partial_cmp(&right.rank)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.module.cmp(&right.module))
            .then_with(|| left.file_path.cmp(&right.file_path))
    });
}

fn odoo_rank_adjustment(query_lower: &str, symbol: &SearchSymbol) -> f64 {
    let mut adjustment = 0.0;
    let file_path = symbol.file_path.as_deref().unwrap_or("");

    if symbol.qualname.as_deref() == Some(query_lower)
        || symbol.name.eq_ignore_ascii_case(query_lower)
    {
        adjustment -= 10.0;
    }
    if query_lower.contains("model") && symbol.kind == "model" {
        adjustment -= 2.0;
    }
    if query_lower.contains("method") && symbol.kind == "method" {
        adjustment -= 2.0;
    }
    if query_lower.contains("field") && symbol.kind == "field" {
        adjustment -= 2.0;
    }
    if (query_lower.contains("xmlid") || query_lower.contains("view"))
        && (symbol.kind == "xmlid" || symbol.kind == "view")
    {
        adjustment -= 2.0;
    }
    if query_lower.contains("module") && symbol.kind == "module" {
        adjustment -= 2.0;
    }

    if file_path.contains("/models/") && mentions_backend_model(query_lower) {
        adjustment -= 0.4;
    }
    if file_path.contains("/views/")
        && (query_lower.contains("view") || query_lower.contains("xml"))
    {
        adjustment -= 0.4;
    }
    if file_path.contains("/security/") && query_lower.contains("security") {
        adjustment -= 0.4;
    }
    if file_path.contains("/static/src/")
        && (query_lower.contains("js")
            || query_lower.contains("javascript")
            || query_lower.contains("owl")
            || query_lower.contains("frontend"))
    {
        adjustment -= 0.4;
    }
    if symbol.kind == "model"
        && symbol
            .qualname
            .as_deref()
            .is_some_and(|qualname| query_lower.contains(qualname.trim_start_matches("model:")))
    {
        adjustment -= 0.6;
    }

    adjustment
}

fn mentions_backend_model(query_lower: &str) -> bool {
    query_lower.contains("model")
        || query_lower.contains("method")
        || query_lower.contains("field")
        || query_lower
            .split_whitespace()
            .any(|token| token.contains('.'))
}

fn exact_search_symbols(
    con: &Connection,
    codebase_id: i64,
    query: &str,
    module: Option<&str>,
    limit: usize,
) -> Result<Vec<SearchSymbol>> {
    let normalized = query.trim();
    if normalized.is_empty() {
        return Ok(Vec::new());
    }
    let exact_qualnames = [
        format!("model:{normalized}"),
        format!("method:{normalized}"),
        format!("field:{normalized}"),
        format!("xmlid:{normalized}"),
        format!("view:{normalized}"),
        format!("module:{normalized}"),
        normalized.to_string(),
    ];
    let sql = if module.is_some() {
        r#"
        SELECT kind, name, qualname, module, file_path,
               CASE kind
                   WHEN 'module' THEN -1000001.0
                   WHEN 'model' THEN -1000000.0
                   WHEN 'method' THEN -999999.0
                   WHEN 'field' THEN -999998.0
                   WHEN 'xmlid' THEN -999997.0
                   WHEN 'view' THEN -999996.0
                   ELSE -999995.0
               END AS exact_rank
        FROM symbols
        WHERE codebase_id = ?1
          AND module = ?2
          AND (name = ?3 OR qualname IN (?4, ?5, ?6, ?7, ?8, ?9, ?10))
        ORDER BY exact_rank, module, file_path
        LIMIT ?11
        "#
    } else {
        r#"
        SELECT kind, name, qualname, module, file_path,
               CASE kind
                   WHEN 'module' THEN -1000001.0
                   WHEN 'model' THEN -1000000.0
                   WHEN 'method' THEN -999999.0
                   WHEN 'field' THEN -999998.0
                   WHEN 'xmlid' THEN -999997.0
                   WHEN 'view' THEN -999996.0
                   ELSE -999995.0
               END AS exact_rank
        FROM symbols
        WHERE codebase_id = ?1
          AND (name = ?2 OR qualname IN (?3, ?4, ?5, ?6, ?7, ?8, ?9))
        ORDER BY exact_rank, module, file_path
        LIMIT ?10
        "#
    };
    let mut stmt = con.prepare(sql)?;
    if let Some(module) = module {
        let rows = stmt.query_map(
            params![
                codebase_id,
                module,
                normalized,
                exact_qualnames[0],
                exact_qualnames[1],
                exact_qualnames[2],
                exact_qualnames[3],
                exact_qualnames[4],
                exact_qualnames[5],
                exact_qualnames[6],
                limit as i64
            ],
            search_symbol_from_row,
        )?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    } else {
        let rows = stmt.query_map(
            params![
                codebase_id,
                normalized,
                exact_qualnames[0],
                exact_qualnames[1],
                exact_qualnames[2],
                exact_qualnames[3],
                exact_qualnames[4],
                exact_qualnames[5],
                exact_qualnames[6],
                limit as i64
            ],
            search_symbol_from_row,
        )?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

fn prepend_unique_symbols(
    symbols: &mut Vec<SearchSymbol>,
    exact_symbols: Vec<SearchSymbol>,
    limit: usize,
) {
    for exact in exact_symbols.into_iter().rev() {
        symbols.retain(|symbol| {
            !(symbol.kind == exact.kind
                && symbol.name == exact.name
                && symbol.qualname == exact.qualname
                && symbol.module == exact.module
                && symbol.file_path == exact.file_path)
        });
        symbols.insert(0, exact);
    }
    symbols.truncate(limit);
}

fn search_symbol_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SearchSymbol> {
    Ok(SearchSymbol {
        kind: row.get(0)?,
        name: row.get(1)?,
        qualname: row.get(2)?,
        module: row.get(3)?,
        file_path: row.get(4)?,
        rank: row.get(5)?,
    })
}

fn search_symbols(
    con: &Connection,
    codebase_id: i64,
    query: &str,
    module: Option<&str>,
    limit: usize,
    mode: &str,
) -> Result<Vec<SearchSymbol>> {
    let fts_query = fts_query(query, mode);
    let sql = if module.is_some() {
        r#"
        SELECT kind, name, qualname, module, file_path, rank
        FROM fts_symbols
        WHERE codebase_id = ?1 AND fts_symbols MATCH ?2 AND module = ?3
        ORDER BY rank
        LIMIT ?4
        "#
    } else {
        r#"
        SELECT kind, name, qualname, module, file_path, rank
        FROM fts_symbols
        WHERE codebase_id = ?1 AND fts_symbols MATCH ?2
        ORDER BY rank
        LIMIT ?3
        "#
    };
    let mut stmt = con.prepare(sql)?;
    if let Some(module) = module {
        let rows = stmt.query_map(
            params![codebase_id, fts_query, module, limit as i64],
            |row| {
                Ok(SearchSymbol {
                    kind: row.get(0)?,
                    name: row.get(1)?,
                    qualname: row.get(2)?,
                    module: row.get(3)?,
                    file_path: row.get(4)?,
                    rank: row.get(5)?,
                })
            },
        )?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    } else {
        let rows = stmt.query_map(params![codebase_id, fts_query, limit as i64], |row| {
            Ok(SearchSymbol {
                kind: row.get(0)?,
                name: row.get(1)?,
                qualname: row.get(2)?,
                module: row.get(3)?,
                file_path: row.get(4)?,
                rank: row.get(5)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

fn search_chunks(
    con: &Connection,
    codebase_id: i64,
    query: &str,
    module: Option<&str>,
    limit: usize,
    mode: &str,
) -> Result<Vec<SearchChunk>> {
    let fts_query = fts_query(query, mode);
    let sql = if module.is_some() {
        r#"
        SELECT symbol_kind, symbol_name, module, file_path, rank
        FROM fts_chunks
        WHERE codebase_id = ?1 AND fts_chunks MATCH ?2 AND module = ?3
        ORDER BY rank
        LIMIT ?4
        "#
    } else {
        r#"
        SELECT symbol_kind, symbol_name, module, file_path, rank
        FROM fts_chunks
        WHERE codebase_id = ?1 AND fts_chunks MATCH ?2
        ORDER BY rank
        LIMIT ?3
        "#
    };
    let mut stmt = con.prepare(sql)?;
    if let Some(module) = module {
        let rows = stmt.query_map(
            params![codebase_id, fts_query, module, limit as i64],
            |row| {
                Ok(SearchChunk {
                    kind: row.get(0)?,
                    name: row.get(1)?,
                    module: row.get(2)?,
                    file_path: row.get(3)?,
                    rank: row.get(4)?,
                })
            },
        )?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    } else {
        let rows = stmt.query_map(params![codebase_id, fts_query, limit as i64], |row| {
            Ok(SearchChunk {
                kind: row.get(0)?,
                name: row.get(1)?,
                module: row.get(2)?,
                file_path: row.get(3)?,
                rank: row.get(4)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

fn fts_query(value: &str, mode: &str) -> String {
    let tokens: Vec<String> = value
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '.'))
        .filter(|token| token.len() > 1)
        .take(12)
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect();
    if tokens.is_empty() {
        "\"\"".to_string()
    } else if mode == "or" {
        tokens.join(" OR ")
    } else {
        tokens.join(" ")
    }
}
