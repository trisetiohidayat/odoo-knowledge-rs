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
    let mut symbols = search_symbols(con, codebase.id, query, module, limit, "and")?;
    let mut chunks = search_chunks(con, codebase.id, query, module, limit, "and")?;
    if symbols.len() + chunks.len() < limit.min(5) {
        symbols = search_symbols(con, codebase.id, query, module, limit, "or")?;
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
