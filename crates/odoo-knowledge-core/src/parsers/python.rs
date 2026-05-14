use std::path::Path;

use serde::Serialize;

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

#[derive(Debug, Clone)]
struct ClassBlock {
    name: String,
    start: usize,
    end: usize,
}

pub fn parse_python_file(path: &Path, module: &str, root: &Path) -> Result<PythonParseResult> {
    let file_path = rel_path(path, root);
    let text = std::fs::read_to_string(path)?;
    let lines: Vec<&str> = text.lines().collect();
    let mut result = PythonParseResult::default();

    for class in class_blocks(&lines) {
        let body = &lines[class.start..class.end];
        let model_name = find_attr_string(body, "_name");
        let inherit = find_attr_string_list(body, "_inherit");
        let inherits = find_attr_raw(body, "_inherits").unwrap_or_else(|| "{}".to_string());
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
                file_path: file_path.clone(),
                class_name: class.name.clone(),
                model_name: effective_model.clone(),
                inherit: inherit.clone(),
                inherits,
                line_start: class.start + 1,
                line_end: class.end,
            });
        }

        let mut pending_decorators: Vec<String> = Vec::new();
        let mut method_starts: Vec<(usize, String, Vec<String>)> = Vec::new();
        for (offset, line) in body.iter().enumerate() {
            let line_no = class.start + offset + 1;
            let trimmed = line.trim();
            if trimmed.starts_with('@') {
                pending_decorators.push(decorator_name(trimmed));
                continue;
            }
            if let Some(method_name) = method_name(trimmed) {
                method_starts.push((line_no, method_name, pending_decorators.clone()));
                pending_decorators.clear();
                continue;
            }
            pending_decorators.clear();

            if let Some(field) =
                parse_field_line(line, module, &file_path, effective_model.clone(), line_no)
            {
                result.fields.push(field);
            }
        }

        for idx in 0..method_starts.len() {
            let (line_start, method_name, decorators) = method_starts[idx].clone();
            let line_end = method_starts
                .get(idx + 1)
                .map(|(next_start, _, _)| next_start.saturating_sub(1))
                .unwrap_or(class.end);
            let method_text = lines[line_start.saturating_sub(1)..line_end].join("\n");
            result.methods.push(PythonMethod {
                module: module.to_string(),
                file_path: file_path.clone(),
                model_name: effective_model.clone(),
                class_name: class.name.clone(),
                method_name,
                decorators,
                calls_super: method_text.contains("super(") || method_text.contains("super()."),
                calls: extract_calls(&method_text),
                line_start,
                line_end,
            });
        }
    }

    Ok(result)
}

fn class_blocks(lines: &[&str]) -> Vec<ClassBlock> {
    let mut starts = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("class ") {
            continue;
        }
        let indent = indent_width(line);
        let name = trimmed["class ".len()..]
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .collect::<String>();
        if !name.is_empty() {
            starts.push((idx, indent, name));
        }
    }

    let mut blocks = Vec::new();
    for (pos, (start, indent, name)) in starts.iter().enumerate() {
        let end = starts
            .iter()
            .skip(pos + 1)
            .find(|(_, next_indent, _)| next_indent <= indent)
            .map(|(next_start, _, _)| *next_start)
            .unwrap_or(lines.len());
        blocks.push(ClassBlock {
            name: name.clone(),
            start: *start,
            end,
        });
    }
    blocks
}

fn parse_field_line(
    line: &str,
    module: &str,
    file_path: &str,
    model_name: Option<String>,
    line_no: usize,
) -> Option<PythonField> {
    let trimmed = line.trim();
    if !trimmed.contains("fields.") || !trimmed.contains('=') {
        return None;
    }
    let (target, value) = trimmed.split_once('=')?;
    let field_name = target.trim();
    if field_name.is_empty() || field_name.contains(' ') || field_name.contains('.') {
        return None;
    }
    let field_marker = "fields.";
    let field_start = value.find(field_marker)? + field_marker.len();
    let field_type = value[field_start..]
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>();
    if field_type.is_empty() {
        return None;
    }
    Some(PythonField {
        module: module.to_string(),
        file_path: file_path.to_string(),
        model_name,
        field_name: field_name.to_string(),
        field_type: Some(field_type),
        comodel: first_string_arg(value),
        compute: keyword_string(value, "compute"),
        inverse: keyword_string(value, "inverse"),
        search: keyword_string(value, "search"),
        related: keyword_string(value, "related"),
        line_start: line_no,
        line_end: line_no,
    })
}

fn find_attr_string(lines: &[&str], attr: &str) -> Option<String> {
    find_attr_raw(lines, attr).and_then(|raw| quoted_string(&raw))
}

fn find_attr_string_list(lines: &[&str], attr: &str) -> Vec<String> {
    let Some(raw) = find_attr_raw(lines, attr) else {
        return Vec::new();
    };
    if let Some(value) = quoted_string(&raw) {
        return vec![value];
    }
    let Some(open) = raw.find('[').or_else(|| raw.find('(')) else {
        return Vec::new();
    };
    let close = raw
        .rfind(']')
        .or_else(|| raw.rfind(')'))
        .unwrap_or(raw.len());
    raw[open + 1..close]
        .split(',')
        .filter_map(quoted_string)
        .collect()
}

fn find_attr_raw(lines: &[&str], attr: &str) -> Option<String> {
    for line in lines {
        let trimmed = line.trim();
        if !trimmed.starts_with(attr) {
            continue;
        }
        let Some((left, right)) = trimmed.split_once('=') else {
            continue;
        };
        if left.trim() == attr {
            return Some(right.trim().trim_end_matches(',').to_string());
        }
    }
    None
}

fn first_string_arg(value: &str) -> Option<String> {
    let open = value.find('(')?;
    let rest = &value[open + 1..];
    let first = rest.split(',').next()?.trim();
    quoted_string(first)
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

fn method_name(trimmed: &str) -> Option<String> {
    let rest = trimmed
        .strip_prefix("def ")
        .or_else(|| trimmed.strip_prefix("async def "))?;
    let name = rest
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn decorator_name(trimmed: &str) -> String {
    trimmed
        .trim_start_matches('@')
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '.')
        .collect()
}

fn extract_calls(text: &str) -> Vec<String> {
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

fn indent_width(line: &str) -> usize {
    line.chars()
        .take_while(|ch| ch.is_whitespace())
        .map(|ch| if ch == '\t' { 4 } else { 1 })
        .sum()
}

fn rel_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
