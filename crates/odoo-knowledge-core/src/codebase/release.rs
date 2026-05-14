use std::path::Path;

use crate::error::Result;

pub fn detect_release(root: &Path) -> Result<(Option<String>, Option<String>)> {
    let release = root.join("odoo").join("release.py");
    if !release.exists() {
        return Ok((None, None));
    }
    let text = std::fs::read_to_string(release)?;
    let series = detect_version_info_series(&text);
    let version = detect_explicit_version(&text).or_else(|| series.clone());
    Ok((series, version))
}

fn detect_version_info_series(text: &str) -> Option<String> {
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("version_info"))?;
    let open = line.find('(')?;
    let close = line[open + 1..].find(')')? + open + 1;
    let parts: Vec<String> = line[open + 1..close]
        .split(',')
        .map(|part| part.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() >= 2 {
        Some(format!("{}.{}", parts[0], parts[1]))
    } else {
        None
    }
}

fn detect_explicit_version(text: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("version") || trimmed.starts_with("version_info") {
            continue;
        }
        let (_, value) = trimmed.split_once('=')?;
        let value = value.trim();
        if !(value.starts_with('"') || value.starts_with('\'')) {
            continue;
        }
        let version = value.trim_matches('"').trim_matches('\'');
        if !version.is_empty() {
            return Some(version.to_string());
        }
    }
    None
}
