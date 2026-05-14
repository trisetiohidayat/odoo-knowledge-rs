use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

#[derive(Debug, Default, Clone, Serialize)]
pub struct XmlParseResult {
    pub records: Vec<XmlRecord>,
    pub views: Vec<XmlView>,
    pub actions: Vec<XmlAction>,
    pub menus: Vec<XmlMenu>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct XmlRecord {
    pub module: String,
    pub file_path: String,
    pub xmlid: String,
    pub record_model: Option<String>,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct XmlView {
    pub module: String,
    pub file_path: String,
    pub xmlid: Option<String>,
    pub view_model: Option<String>,
    pub inherit_id: Option<String>,
    pub priority: Option<String>,
    pub xpath_count: usize,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct XmlAction {
    pub module: String,
    pub file_path: String,
    pub xmlid: Option<String>,
    pub action_model: Option<String>,
    pub res_model: Option<String>,
    pub view_id: Option<String>,
    pub line_start: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct XmlMenu {
    pub module: String,
    pub file_path: String,
    pub xmlid: Option<String>,
    pub action_ref: Option<String>,
    pub parent_ref: Option<String>,
    pub line_start: usize,
}

pub fn parse_xml_file(path: &Path, module: &str, root: &Path) -> XmlParseResult {
    let file_path = rel_path(path, root);
    let mut result = XmlParseResult::default();
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            result.errors.push(err.to_string());
            return result;
        }
    };
    let lines = line_map(&text);
    let doc = match roxmltree::Document::parse(&text) {
        Ok(doc) => doc,
        Err(err) => {
            result.errors.push(err.to_string());
            return result;
        }
    };

    for node in doc.descendants().filter(|node| node.has_tag_name("record")) {
        let Some(id) = node.attribute("id") else {
            continue;
        };
        let model = node.attribute("model").map(str::to_string);
        let xmlid = xmlid(module, id);
        let line = lines.get(id).copied().unwrap_or(0);
        result.records.push(XmlRecord {
            module: module.to_string(),
            file_path: file_path.clone(),
            xmlid: xmlid.clone(),
            record_model: model.clone(),
            line_start: line,
            line_end: line,
        });

        if model.as_deref() == Some("ir.ui.view") {
            result.views.push(XmlView {
                module: module.to_string(),
                file_path: file_path.clone(),
                xmlid: Some(xmlid),
                view_model: field_text(node, "model"),
                inherit_id: field_text(node, "inherit_id"),
                priority: field_text(node, "priority"),
                xpath_count: node
                    .descendants()
                    .filter(|child| child.has_tag_name("xpath"))
                    .count(),
                line_start: line,
                line_end: line,
            });
        } else if model
            .as_deref()
            .is_some_and(|value| value.starts_with("ir.actions."))
        {
            result.actions.push(XmlAction {
                module: module.to_string(),
                file_path: file_path.clone(),
                xmlid: Some(xmlid),
                action_model: model,
                res_model: field_text(node, "res_model"),
                view_id: field_text(node, "view_id"),
                line_start: line,
            });
        }
    }

    for node in doc
        .descendants()
        .filter(|node| node.has_tag_name("menuitem"))
    {
        let Some(id) = node.attribute("id") else {
            continue;
        };
        result.menus.push(XmlMenu {
            module: module.to_string(),
            file_path: file_path.clone(),
            xmlid: Some(xmlid(module, id)),
            action_ref: node.attribute("action").map(str::to_string),
            parent_ref: node.attribute("parent").map(str::to_string),
            line_start: lines.get(id).copied().unwrap_or(0),
        });
    }

    result
}

fn field_text(node: roxmltree::Node<'_, '_>, name: &str) -> Option<String> {
    for child in node.children().filter(|child| child.has_tag_name("field")) {
        if child.attribute("name") != Some(name) {
            continue;
        }
        if let Some(value) = child.attribute("ref") {
            return Some(value.to_string());
        }
        if let Some(value) = child.attribute("eval") {
            return Some(value.to_string());
        }
        let value = child.text().unwrap_or("").trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

fn line_map(text: &str) -> HashMap<String, usize> {
    let mut mapping = HashMap::new();
    for (offset, line) in text.lines().enumerate() {
        for tag in ["record", "menuitem"] {
            if let Some(id) = extract_id_from_tag(line, tag) {
                mapping.entry(id).or_insert(offset + 1);
            }
        }
    }
    mapping
}

fn extract_id_from_tag(line: &str, tag: &str) -> Option<String> {
    let tag_start = line.find(&format!("<{tag}"))?;
    let rest = &line[tag_start..];
    let id_start = rest.find("id=")? + 3;
    let rest = &rest[id_start..];
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &rest[1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn xmlid(module: &str, id: &str) -> String {
    if id.contains('.') {
        id.to_string()
    } else {
        format!("{module}.{id}")
    }
}

fn rel_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
