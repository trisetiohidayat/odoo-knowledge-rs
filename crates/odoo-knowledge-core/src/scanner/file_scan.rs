use std::path::{Path, PathBuf};

use walkdir::WalkDir;

pub fn relevant_files(module_path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in WalkDir::new(module_path)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.into_path();
        let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        if matches!(ext, "py" | "xml" | "csv" | "js" | "scss" | "css") {
            files.push(path);
        }
    }
    files.sort();
    files
}
