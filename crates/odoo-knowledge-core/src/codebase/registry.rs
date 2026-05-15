use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::codebase::git::git_value;
use crate::codebase::release::detect_release;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize)]
pub struct Codebase {
    pub id: i64,
    pub name: String,
    pub root_path: PathBuf,
    pub odoo_series: Option<String>,
    pub version: Option<String>,
    pub git_remote: Option<String>,
    pub git_branch: Option<String>,
    pub git_commit: Option<String>,
    pub indexed_at: Option<String>,
}

pub fn add_codebase(con: &Connection, name: &str, root_path: &Path) -> Result<i64> {
    let root = root_path.canonicalize()?;
    let (series, version) = detect_release(&root)?;
    let remote = git_value(&root, &["config", "--get", "remote.origin.url"]);
    let branch = git_value(&root, &["branch", "--show-current"]);
    let commit = git_value(&root, &["rev-parse", "--short", "HEAD"]);

    con.execute(
        r#"
        INSERT INTO codebases(name, root_path, odoo_series, version, git_remote, git_branch, git_commit)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(name) DO UPDATE SET
            root_path=excluded.root_path,
            odoo_series=excluded.odoo_series,
            version=excluded.version,
            git_remote=excluded.git_remote,
            git_branch=excluded.git_branch,
            git_commit=excluded.git_commit
        "#,
        params![
            name,
            root.to_string_lossy(),
            series,
            version,
            remote,
            branch,
            commit
        ],
    )?;

    let id = con.query_row("SELECT id FROM codebases WHERE name=?1", [name], |row| {
        row.get::<_, i64>(0)
    })?;
    Ok(id)
}

pub fn list_codebases(con: &Connection) -> Result<Vec<Codebase>> {
    let mut stmt = con.prepare(
        r#"
        SELECT id, name, root_path, odoo_series, version, git_remote, git_branch, git_commit, indexed_at
        FROM codebases
        ORDER BY name
        "#,
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Codebase {
            id: row.get(0)?,
            name: row.get(1)?,
            root_path: PathBuf::from(row.get::<_, String>(2)?),
            odoo_series: row.get(3)?,
            version: row.get(4)?,
            git_remote: row.get(5)?,
            git_branch: row.get(6)?,
            git_commit: row.get(7)?,
            indexed_at: row.get(8)?,
        })
    })?;

    let mut codebases = Vec::new();
    for row in rows {
        codebases.push(row?);
    }
    Ok(codebases)
}

pub fn get_codebase(con: &Connection, name: Option<&str>) -> Result<Codebase> {
    let resolved_name = if let Some(name) = name {
        Some(resolve_codebase_name(con, name)?)
    } else {
        None
    };
    let sql = if resolved_name.is_some() {
        r#"
        SELECT id, name, root_path, odoo_series, version, git_remote, git_branch, git_commit, indexed_at
        FROM codebases
        WHERE name=?1
        "#
    } else {
        r#"
        SELECT id, name, root_path, odoo_series, version, git_remote, git_branch, git_commit, indexed_at
        FROM codebases
        ORDER BY indexed_at DESC NULLS LAST, id DESC
        LIMIT 1
        "#
    };

    let mut stmt = con.prepare(sql)?;
    let mut rows = if let Some(name) = resolved_name.as_deref() {
        stmt.query([name])?
    } else {
        stmt.query([])?
    };
    let Some(row) = rows.next()? else {
        return Err(Error::CodebaseNotFound(codebase_not_found_message(con, name)?));
    };
    Ok(Codebase {
        id: row.get(0)?,
        name: row.get(1)?,
        root_path: PathBuf::from(row.get::<_, String>(2)?),
        odoo_series: row.get(3)?,
        version: row.get(4)?,
        git_remote: row.get(5)?,
        git_branch: row.get(6)?,
        git_commit: row.get(7)?,
        indexed_at: row.get(8)?,
    })
}

fn resolve_codebase_name(con: &Connection, requested: &str) -> Result<String> {
    let requested = requested.trim();
    if requested.is_empty() {
        return Err(Error::CodebaseNotFound(codebase_not_found_message(
            con,
            Some(requested),
        )?));
    }
    if codebase_exists(con, requested)? {
        return Ok(requested.to_string());
    }

    let Some(requested_series) = extract_odoo_series(requested) else {
        return Err(Error::CodebaseNotFound(codebase_not_found_message(
            con,
            Some(requested),
        )?));
    };
    let candidates = list_codebases(con)?
        .into_iter()
        .filter(|codebase| codebase_matches_series(codebase, &requested_series))
        .collect::<Vec<_>>();
    if candidates.len() == 1 {
        return Ok(candidates[0].name.clone());
    }

    Err(Error::CodebaseNotFound(codebase_not_found_message(
        con,
        Some(requested),
    )?))
}

fn codebase_exists(con: &Connection, name: &str) -> Result<bool> {
    let count = con.query_row(
        "SELECT COUNT(*) FROM codebases WHERE name=?1",
        [name],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(count > 0)
}

fn codebase_matches_series(codebase: &Codebase, requested_series: &str) -> bool {
    codebase
        .odoo_series
        .as_deref()
        .is_some_and(|series| series.starts_with(requested_series))
        || codebase
            .version
            .as_deref()
            .is_some_and(|version| version.starts_with(requested_series))
        || extract_odoo_series(&codebase.name).as_deref() == Some(requested_series)
}

fn extract_odoo_series(value: &str) -> Option<String> {
    let mut digits = String::new();
    for ch in value.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            if digits.len() == 2 {
                let number = digits.parse::<u8>().ok()?;
                if (8..=99).contains(&number) {
                    return Some(digits);
                }
                digits.clear();
            }
        } else {
            digits.clear();
        }
    }
    None
}

fn codebase_not_found_message(con: &Connection, requested: Option<&str>) -> Result<String> {
    let available = list_codebases(con)?;
    let available_names = available
        .iter()
        .map(|codebase| codebase.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let mut message = format!(
        "codebase `{}` is not indexed. `codebase` must be one of the indexed Odoo source names, not a local project/addons directory name.",
        requested.unwrap_or("(default)")
    );
    if available.is_empty() {
        message.push_str(" No codebases are registered in this index yet.");
    } else {
        message.push_str(&format!(" Available codebases: {available_names}."));
    }
    message.push_str(
        " If your local project uses Odoo CE 17/18/19, pass the matching indexed core codebase such as `odoo-17`, `odoo-18`, or `odoo-19`. You may also pass version-like text such as `17`, `17.0`, or `Odoo 17 CE` when exactly one indexed codebase matches that series.",
    );
    Ok(message)
}
