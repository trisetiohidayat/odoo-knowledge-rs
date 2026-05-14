use std::path::Path;

use serde::Serialize;

use crate::error::Result;

#[derive(Debug, Clone, Serialize)]
pub struct FrontendSymbol {
    pub module: String,
    pub kind: String,
    pub name: String,
    pub target: Option<String>,
    pub category: Option<String>,
    pub file_path: String,
    pub line_start: usize,
    pub line_end: usize,
}

pub fn parse_js_file(path: &Path, module: &str, root: &Path) -> Result<Vec<FrontendSymbol>> {
    let text = std::fs::read_to_string(path)?;
    let file_path = rel_path(path, root);
    let mut symbols = Vec::new();

    for (offset, line) in text.lines().enumerate() {
        let line_no = offset + 1;
        if let Some(target) = extract_call_first_arg(line, "patch(") {
            symbols.push(FrontendSymbol {
                module: module.to_string(),
                kind: "js_patch".to_string(),
                name: format!("patch:{target}"),
                target: Some(target),
                category: None,
                file_path: file_path.clone(),
                line_start: line_no,
                line_end: line_no,
            });
        }
        if let Some((category, name)) = extract_registry_add(line) {
            symbols.push(FrontendSymbol {
                module: module.to_string(),
                kind: "js_registry".to_string(),
                name,
                target: None,
                category: Some(category),
                file_path: file_path.clone(),
                line_start: line_no,
                line_end: line_no,
            });
        }
        if let Some(name) = extract_class_name(line) {
            symbols.push(FrontendSymbol {
                module: module.to_string(),
                kind: "js_class".to_string(),
                name,
                target: None,
                category: None,
                file_path: file_path.clone(),
                line_start: line_no,
                line_end: line_no,
            });
        }
    }

    Ok(symbols)
}

fn extract_call_first_arg(line: &str, marker: &str) -> Option<String> {
    let start = line.find(marker)? + marker.len();
    let rest = &line[start..];
    let end = rest
        .find(',')
        .or_else(|| rest.find(')'))
        .unwrap_or(rest.len());
    let value = rest[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn extract_registry_add(line: &str) -> Option<(String, String)> {
    let category_marker = "registry.category(";
    let category_start = line.find(category_marker)? + category_marker.len();
    let category_rest = &line[category_start..];
    let quote = category_rest.chars().find(|ch| *ch == '"' || *ch == '\'')?;
    let after_quote = &category_rest[category_rest.find(quote)? + 1..];
    let quote_end = after_quote.find(quote)?;
    let category = after_quote[..quote_end].to_string();
    let add_marker = ".add(";
    let add_start = line.find(add_marker)? + add_marker.len();
    let add_rest = &line[add_start..];
    let end = add_rest
        .find(',')
        .or_else(|| add_rest.find(')'))
        .unwrap_or(add_rest.len());
    let name = add_rest[..end]
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string();
    if category.is_empty() || name.is_empty() {
        None
    } else {
        Some((category, name))
    }
}

fn extract_class_name(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("class ")?;
    let name: String = rest
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn rel_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
