use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;
use tree_sitter::{Node, Parser};

use crate::error::Result;

#[derive(Debug, Default, Clone, Serialize)]
pub struct PythonParseResult {
    pub models: Vec<PythonModel>,
    pub fields: Vec<PythonField>,
    pub methods: Vec<PythonMethod>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PythonModel {
    pub module: String,
    pub file_path: String,
    pub class_name: String,
    pub model_name: Option<String>,
    pub inherit: Vec<String>,
    pub inherits: String,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PythonField {
    pub module: String,
    pub file_path: String,
    pub model_name: Option<String>,
    pub field_name: String,
    pub field_type: Option<String>,
    pub comodel: Option<String>,
    pub compute: Option<String>,
    pub inverse: Option<String>,
    pub search: Option<String>,
    pub related: Option<String>,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PythonMethod {
    pub module: String,
    pub file_path: String,
    pub model_name: Option<String>,
    pub class_name: String,
    pub method_name: String,
    pub decorators: Vec<String>,
    pub calls_super: bool,
    pub calls: Vec<String>,
    pub line_start: usize,
    pub line_end: usize,
}

pub fn parse_python_file(path: &Path, module: &str, root: &Path) -> Result<PythonParseResult> {
    let file_path = rel_path(path, root);
    let source = std::fs::read_to_string(path)?;
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .map_err(|err| {
            crate::Error::InvalidConfig(format!("failed to load Python grammar: {err}"))
        })?;
    let Some(tree) = parser.parse(&source, None) else {
        return Ok(PythonParseResult {
            errors: vec!["tree-sitter failed to parse Python source".to_string()],
            ..PythonParseResult::default()
        });
    };

    let mut result = PythonParseResult::default();
    if tree.root_node().has_error() {
        result
            .errors
            .push("tree-sitter reported syntax errors".to_string());
    }
    collect_classes(tree.root_node(), &source, module, &file_path, &mut result);
    Ok(result)
}

fn collect_classes(
    node: Node<'_>,
    source: &str,
    module: &str,
    file_path: &str,
    result: &mut PythonParseResult,
) {
    if node.kind() == "class_definition" {
        parse_class(node, source, module, file_path, result);
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_classes(child, source, module, file_path, result);
    }
}

fn parse_class(
    class_node: Node<'_>,
    source: &str,
    module: &str,
    file_path: &str,
    result: &mut PythonParseResult,
) {
    let class_name = class_node
        .child_by_field_name("name")
        .and_then(|node| text(node, source).ok())
        .unwrap_or_default();
    if class_name.is_empty() {
        return;
    }
    let Some(body) = class_node.child_by_field_name("body") else {
        return;
    };

    let mut model_name = None;
    let mut inherit = Vec::new();
    let mut inherits = "{}".to_string();
    let mut pending_decorators = Vec::new();

    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        match child.kind() {
            "assignment" | "expression_statement" => {
                if let Some((left, right)) = assignment_node(child).and_then(assignment_parts) {
                    let left_text = text(left, source).unwrap_or_default();
                    if left_text == "_name" {
                        model_name = string_literal_value(right, source);
                    } else if left_text == "_inherit" {
                        inherit = string_list_value(right, source);
                    } else if left_text == "_inherits" {
                        inherits = text(right, source).unwrap_or_else(|_| "{}".to_string());
                    }
                }
            }
            "decorated_definition" => {
                let decorators = decorators(child, source);
                if let Some(function) = first_child_kind(child, "function_definition") {
                    pending_decorators = decorators;
                    parse_method(
                        function,
                        source,
                        module,
                        file_path,
                        &class_name,
                        None,
                        &pending_decorators,
                        result,
                    );
                    pending_decorators.clear();
                }
            }
            "function_definition" => {
                parse_method(
                    child,
                    source,
                    module,
                    file_path,
                    &class_name,
                    None,
                    &pending_decorators,
                    result,
                );
                pending_decorators.clear();
            }
            _ => {}
        }
    }

    let effective_model = model_name.clone().or_else(|| {
        if inherit.len() == 1 {
            inherit.first().cloned()
        } else {
            None
        }
    });

    if model_name.is_some() || !inherit.is_empty() || inherits != "{}" {
        result.models.push(PythonModel {
            module: module.to_string(),
            file_path: file_path.to_string(),
            class_name: class_name.clone(),
            model_name: effective_model.clone(),
            inherit: inherit.clone(),
            inherits,
            line_start: line_start(class_node),
            line_end: line_end(class_node),
        });
    }

    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        if matches!(child.kind(), "assignment" | "expression_statement") {
            if let Some(field) = assignment_node(child).and_then(|assignment| {
                parse_field(
                    assignment,
                    source,
                    module,
                    file_path,
                    effective_model.clone(),
                )
            }) {
                result.fields.push(field);
            }
        } else if child.kind() == "function_definition" {
            update_method_model(result, &class_name, child, source, effective_model.clone());
        } else if child.kind() == "decorated_definition" {
            if let Some(function) = first_child_kind(child, "function_definition") {
                update_method_model(
                    result,
                    &class_name,
                    function,
                    source,
                    effective_model.clone(),
                );
            }
        }
    }
}

fn parse_method(
    node: Node<'_>,
    source: &str,
    module: &str,
    file_path: &str,
    class_name: &str,
    model_name: Option<String>,
    decorators: &[String],
    result: &mut PythonParseResult,
) {
    let Some(name_node) = node.child_by_field_name("name") else {
        return;
    };
    let method_name = text(name_node, source).unwrap_or_default();
    let method_text = text(node, source).unwrap_or_default();
    result.methods.push(PythonMethod {
        module: module.to_string(),
        file_path: file_path.to_string(),
        model_name,
        class_name: class_name.to_string(),
        method_name,
        decorators: decorators.to_vec(),
        calls_super: has_super_call(node, source),
        calls: extract_calls(node, source, &method_text),
        line_start: line_start(node),
        line_end: line_end(node),
    });
}

fn update_method_model(
    result: &mut PythonParseResult,
    class_name: &str,
    node: Node<'_>,
    source: &str,
    model_name: Option<String>,
) {
    let Some(name_node) = node.child_by_field_name("name") else {
        return;
    };
    let method_name = text(name_node, source).unwrap_or_default();
    if let Some(method) = result.methods.iter_mut().find(|method| {
        method.class_name == class_name
            && method.method_name == method_name
            && method.line_start == line_start(node)
    }) {
        method.model_name = model_name;
    }
}

fn parse_field(
    node: Node<'_>,
    source: &str,
    module: &str,
    file_path: &str,
    model_name: Option<String>,
) -> Option<PythonField> {
    let (left, right) = assignment_parts(node)?;
    let field_name = text(left, source).ok()?;
    if field_name.contains('.') || field_name.contains(' ') || field_name.is_empty() {
        return None;
    }
    let call = if right.kind() == "call" {
        right
    } else {
        first_child_kind(right, "call")?
    };
    let function = call.child_by_field_name("function")?;
    let function_text = text(function, source).ok()?;
    let field_type = function_text.strip_prefix("fields.")?.to_string();
    if field_type.is_empty() || field_type.contains('.') {
        return None;
    }
    let call_text = text(call, source).ok()?;
    Some(PythonField {
        module: module.to_string(),
        file_path: file_path.to_string(),
        model_name,
        field_name,
        field_type: Some(field_type),
        comodel: first_string_arg(call, source),
        compute: keyword_string(&call_text, "compute"),
        inverse: keyword_string(&call_text, "inverse"),
        search: keyword_string(&call_text, "search"),
        related: keyword_string(&call_text, "related"),
        line_start: line_start(node),
        line_end: line_end(node),
    })
}

fn assignment_parts(node: Node<'_>) -> Option<(Node<'_>, Node<'_>)> {
    let left = node
        .child_by_field_name("left")
        .or_else(|| node.child_by_field_name("target"))?;
    let right = node
        .child_by_field_name("right")
        .or_else(|| node.child_by_field_name("value"))?;
    Some((left, right))
}

fn assignment_node(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "assignment" {
        return Some(node);
    }
    first_child_kind(node, "assignment")
}

fn string_literal_value(node: Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "string" => quoted_string(&text(node, source).ok()?),
        _ => None,
    }
}

fn string_list_value(node: Node<'_>, source: &str) -> Vec<String> {
    if let Some(value) = string_literal_value(node, source) {
        return vec![value];
    }
    let mut values = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "string" {
            if let Some(value) = string_literal_value(child, source) {
                values.push(value);
            }
        }
    }
    values
}

fn first_string_arg(call: Node<'_>, source: &str) -> Option<String> {
    let arguments = call.child_by_field_name("arguments")?;
    let mut cursor = arguments.walk();
    for child in arguments.children(&mut cursor) {
        if child.kind() == "string" {
            return string_literal_value(child, source);
        }
        if child.kind() == "keyword_argument" {
            continue;
        }
    }
    None
}

fn decorators(node: Node<'_>, source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "decorator" {
            continue;
        }
        let value = text(child, source)
            .unwrap_or_default()
            .trim()
            .trim_start_matches('@')
            .split('(')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if !value.is_empty() {
            out.push(value);
        }
    }
    out
}

fn has_super_call(node: Node<'_>, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "call" {
            let value = text(child, source).unwrap_or_default();
            if value.starts_with("super(") || value.starts_with("super().") {
                return true;
            }
        }
        if has_super_call(child, source) {
            return true;
        }
    }
    false
}

fn extract_calls(node: Node<'_>, source: &str, fallback_text: &str) -> Vec<String> {
    let mut calls = BTreeSet::new();
    collect_call_names(node, source, &mut calls);
    if calls.is_empty() {
        calls.extend(extract_calls_from_text(fallback_text));
    }
    calls.into_iter().collect()
}

fn collect_call_names(node: Node<'_>, source: &str, calls: &mut BTreeSet<String>) {
    if node.kind() == "call" {
        if let Some(function) = node.child_by_field_name("function") {
            let name = text(function, source).unwrap_or_default();
            if !name.is_empty() {
                calls.insert(name);
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_call_names(child, source, calls);
    }
}

fn first_child_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == kind {
            return Some(child);
        }
    }
    None
}

fn text(node: Node<'_>, source: &str) -> std::result::Result<String, std::str::Utf8Error> {
    node.utf8_text(source.as_bytes()).map(str::to_string)
}

fn line_start(node: Node<'_>) -> usize {
    node.start_position().row + 1
}

fn line_end(node: Node<'_>) -> usize {
    node.end_position().row + 1
}

fn keyword_string(value: &str, keyword: &str) -> Option<String> {
    let marker = format!("{keyword}=");
    let start = value.find(&marker)? + marker.len();
    let rest = &value[start..];
    let raw = rest.split(',').next()?.trim().trim_end_matches(')');
    quoted_string(raw)
}

fn quoted_string(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let quote = raw.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &raw[1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn extract_calls_from_text(text: &str) -> Vec<String> {
    let mut calls = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    for idx in 0..chars.len() {
        if chars[idx] != '(' {
            continue;
        }
        let mut pos = idx;
        while pos > 0 && chars[pos - 1].is_whitespace() {
            pos -= 1;
        }
        let end = pos;
        while pos > 0
            && (chars[pos - 1].is_ascii_alphanumeric()
                || chars[pos - 1] == '_'
                || chars[pos - 1] == '.')
        {
            pos -= 1;
        }
        if pos < end {
            let name: String = chars[pos..end].iter().collect();
            if !name.is_empty() && !calls.contains(&name) {
                calls.push(name);
            }
        }
    }
    calls.sort();
    calls
}

fn rel_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mini_sale_fixture() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tests/fixtures/addons/mini_sale");
        let path = root.join("models/sale_order.py");
        let parsed = parse_python_file(&path, "mini_sale", &root).unwrap();
        assert_eq!(parsed.models.len(), 1);
        assert_eq!(parsed.models[0].model_name.as_deref(), Some("sale.order"));
        assert_eq!(parsed.fields.len(), 1);
        assert_eq!(parsed.fields[0].field_name, "x_reference");
        assert_eq!(parsed.fields[0].field_type.as_deref(), Some("Char"));
        assert!(parsed
            .methods
            .iter()
            .any(|method| method.method_name == "action_confirm" && method.calls_super));
    }
}
