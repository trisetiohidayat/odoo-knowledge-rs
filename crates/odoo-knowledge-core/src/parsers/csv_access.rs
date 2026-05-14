use std::path::Path;

use serde::Serialize;

use crate::error::Result;

#[derive(Debug, Clone, Serialize)]
pub struct SecurityRule {
    pub module: String,
    pub kind: String,
    pub name: Option<String>,
    pub model_ref: Option<String>,
    pub group_ref: Option<String>,
    pub permissions: Option<String>,
    pub file_path: String,
    pub line_start: usize,
}

pub fn parse_security_csv(path: &Path, module: &str, root: &Path) -> Result<Vec<SecurityRule>> {
    if path.file_name().and_then(|name| name.to_str()) != Some("ir.model.access.csv") {
        return Ok(Vec::new());
    }
    let mut reader = csv::Reader::from_path(path)?;
    let headers = reader.headers()?.clone();
    let index = |name: &str| headers.iter().position(|header| header == name);
    let id_idx = index("id");
    let name_idx = index("name");
    let model_idx = index("model_id:id");
    let group_idx = index("group_id:id");
    let perm_read_idx = index("perm_read");
    let perm_write_idx = index("perm_write");
    let perm_create_idx = index("perm_create");
    let perm_unlink_idx = index("perm_unlink");
    let mut rules = Vec::new();

    for (offset, record) in reader.records().enumerate() {
        let record = record?;
        let value = |idx: Option<usize>| {
            idx.and_then(|idx| record.get(idx))
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        };
        let mut permissions = Vec::new();
        for (idx, name) in [
            (perm_read_idx, "perm_read"),
            (perm_write_idx, "perm_write"),
            (perm_create_idx, "perm_create"),
            (perm_unlink_idx, "perm_unlink"),
        ] {
            if matches!(value(idx).as_deref(), Some("1" | "True" | "true")) {
                permissions.push(name);
            }
        }
        rules.push(SecurityRule {
            module: module.to_string(),
            kind: "ir.model.access".to_string(),
            name: value(name_idx).or_else(|| value(id_idx)),
            model_ref: value(model_idx),
            group_ref: value(group_idx),
            permissions: Some(permissions.join(",")),
            file_path: rel_path(path, root),
            line_start: offset + 2,
        });
    }

    Ok(rules)
}

fn rel_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
