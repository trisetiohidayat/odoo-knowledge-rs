use std::path::Path;

use serde::Serialize;
use tree_sitter::{Node, Parser};

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
    pub confidence: String,
}

pub fn parse_js_file(path: &Path, module: &str, root: &Path) -> Result<Vec<FrontendSymbol>> {
    let source = std::fs::read_to_string(path)?;
    let file_path = rel_path(path, root);
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_javascript::LANGUAGE.into())
        .map_err(|err| {
            crate::Error::InvalidConfig(format!("failed to load JavaScript grammar: {err}"))
        })?;
    let Some(tree) = parser.parse(&source, None) else {
        return Ok(Vec::new());
    };

    let mut symbols = Vec::new();
    collect_symbols(tree.root_node(), &source, module, &file_path, &mut symbols);
    Ok(symbols)
}

fn collect_symbols(
    node: Node<'_>,
    source: &str,
    module: &str,
    file_path: &str,
    symbols: &mut Vec<FrontendSymbol>,
) {
    match node.kind() {
        "import_statement" => collect_import(node, source, module, file_path, symbols),
        "call_expression" => collect_call(node, source, module, file_path, symbols),
        "class_declaration" => collect_class(node, source, module, file_path, symbols),
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_symbols(child, source, module, file_path, symbols);
    }
}

fn collect_import(
    node: Node<'_>,
    source: &str,
    module: &str,
    file_path: &str,
    symbols: &mut Vec<FrontendSymbol>,
) {
    let import_text = text(node, source).unwrap_or_default();
    let Some(source_module) = import_text
        .split(" from ")
        .nth(1)
        .map(trim_js_string)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    symbols.push(FrontendSymbol {
        module: module.to_string(),
        kind: "js_import".to_string(),
        name: source_module.to_string(),
        target: Some(source_module.to_string()),
        category: None,
        file_path: file_path.to_string(),
        line_start: line_start(node),
        line_end: line_end(node),
        confidence: "medium".to_string(),
    });
}

fn collect_call(
    node: Node<'_>,
    source: &str,
    module: &str,
    file_path: &str,
    symbols: &mut Vec<FrontendSymbol>,
) {
    let function_text = node
        .child_by_field_name("function")
        .and_then(|function| text(function, source).ok())
        .unwrap_or_default();

    if function_text == "patch" {
        if let Some(target) = first_argument(node, source).filter(|target| !target.is_empty()) {
            symbols.push(FrontendSymbol {
                module: module.to_string(),
                kind: "js_patch".to_string(),
                name: format!("patch:{target}"),
                target: Some(target),
                category: None,
                file_path: file_path.to_string(),
                line_start: line_start(node),
                line_end: line_end(node),
                confidence: "high".to_string(),
            });
        }
    }

    if function_text.ends_with(".add") && function_text.contains("registry.category") {
        if let (Some(category), Some(name)) = (
            registry_category(&function_text),
            first_argument(node, source),
        ) {
            if !category.is_empty() && !name.is_empty() {
                symbols.push(FrontendSymbol {
                    module: module.to_string(),
                    kind: "js_registry".to_string(),
                    name: trim_js_string(&name).to_string(),
                    target: None,
                    category: Some(category),
                    file_path: file_path.to_string(),
                    line_start: line_start(node),
                    line_end: line_end(node),
                    confidence: "high".to_string(),
                });
            }
        }
    }
}

fn collect_class(
    node: Node<'_>,
    source: &str,
    module: &str,
    file_path: &str,
    symbols: &mut Vec<FrontendSymbol>,
) {
    let Some(name) = node
        .child_by_field_name("name")
        .and_then(|name| text(name, source).ok())
        .filter(|name| !name.is_empty())
    else {
        return;
    };
    symbols.push(FrontendSymbol {
        module: module.to_string(),
        kind: "js_class".to_string(),
        name,
        target: None,
        category: None,
        file_path: file_path.to_string(),
        line_start: line_start(node),
        line_end: line_end(node),
        confidence: "high".to_string(),
    });
}

fn first_argument(node: Node<'_>, source: &str) -> Option<String> {
    let arguments = node.child_by_field_name("arguments")?;
    let mut cursor = arguments.walk();
    let argument = arguments
        .named_children(&mut cursor)
        .next()
        .and_then(|argument| text(argument, source).ok());
    argument
}

fn registry_category(function_text: &str) -> Option<String> {
    let marker = "registry.category(";
    let start = function_text.find(marker)? + marker.len();
    let rest = &function_text[start..];
    let end = rest.find(')')?;
    Some(trim_js_string(rest[..end].trim()).to_string())
}

fn trim_js_string(value: &str) -> &str {
    value
        .trim()
        .trim_end_matches(';')
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches('`')
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
    fn parses_pos_patch_fixture() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf();
        let path = root.join("tests/fixtures/js/pos_patch.js");
        let symbols = parse_js_file(&path, "point_of_sale", &root).unwrap();

        assert!(symbols.iter().any(|symbol| {
            symbol.kind == "js_patch"
                && symbol.name == "patch:PaymentScreen.prototype"
                && symbol.target.as_deref() == Some("PaymentScreen.prototype")
                && symbol.line_start == 5
                && symbol.line_end == 9
        }));
        assert!(symbols.iter().any(|symbol| {
            symbol.kind == "js_registry"
                && symbol.category.as_deref() == Some("pos_screens")
                && symbol.name == "CustomPaymentScreen"
        }));
        assert!(symbols.iter().any(|symbol| {
            symbol.kind == "js_import" && symbol.target.as_deref() == Some("@web/core/utils/patch")
        }));
        assert!(symbols
            .iter()
            .any(|symbol| symbol.kind == "js_class" && symbol.name == "CustomPaymentScreen"));
    }
}
