use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::Result;

#[derive(Debug, Clone, Serialize)]
pub struct Manifest {
    pub module: String,
    pub path: PathBuf,
    pub manifest_path: PathBuf,
    pub depends: Vec<String>,
    pub installable: bool,
    pub auto_install: bool,
    pub application: bool,
    pub summary: String,
}

pub fn parse_manifest(path: &Path) -> Result<Manifest> {
    let text = std::fs::read_to_string(path)?;
    let module = path
        .parent()
        .and_then(|parent| parent.file_name())
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();

    Ok(Manifest {
        module,
        path: path.parent().unwrap_or_else(|| Path::new("")).to_path_buf(),
        manifest_path: path.to_path_buf(),
        depends: extract_string_list(&text, "depends"),
        installable: extract_bool(&text, "installable").unwrap_or(true),
        auto_install: extract_bool(&text, "auto_install").unwrap_or(false),
        application: extract_bool(&text, "application").unwrap_or(false),
        summary: extract_string(&text, "summary")
            .or_else(|| extract_string(&text, "description"))
            .unwrap_or_default(),
    })
}

fn extract_string(text: &str, key: &str) -> Option<String> {
    let needle1 = format!("'{}'", key);
    let needle2 = format!("\"{}\"", key);
    for line in text.lines() {
        if !(line.contains(&needle1) || line.contains(&needle2)) {
            continue;
        }
        let (_, value) = line.split_once(':')?;
        let value = value
            .trim()
            .trim_end_matches(',')
            .trim()
            .trim_matches('"')
            .trim_matches('\'');
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

fn extract_bool(text: &str, key: &str) -> Option<bool> {
    let raw = extract_raw_value(text, key)?;
    if raw.starts_with("True") {
        Some(true)
    } else if raw.starts_with("False") {
        Some(false)
    } else {
        None
    }
}

fn extract_string_list(text: &str, key: &str) -> Vec<String> {
    let Some(raw) = extract_raw_value(text, key) else {
        return Vec::new();
    };
    if raw.starts_with('"') || raw.starts_with('\'') {
        return vec![raw
            .trim_end_matches(',')
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string()];
    }
    let Some(open) = raw.find('[') else {
        return Vec::new();
    };
    let close = raw.find(']').unwrap_or(raw.len());
    raw[open + 1..close]
        .split(',')
        .map(|part| part.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

fn extract_raw_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let needle1 = format!("'{}'", key);
    let needle2 = format!("\"{}\"", key);
    for line in text.lines() {
        if !(line.contains(&needle1) || line.contains(&needle2)) {
            continue;
        }
        let (_, value) = line.split_once(':')?;
        return Some(value.trim());
    }
    None
}
