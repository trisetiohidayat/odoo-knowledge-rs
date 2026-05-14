use std::path::{Path, PathBuf};

use walkdir::WalkDir;

pub fn find_manifests(root: &Path) -> Vec<PathBuf> {
    let mut manifests = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if entry.file_type().is_file() && entry.file_name() == "__manifest__.py" {
            manifests.push(entry.into_path());
        }
    }
    manifests.sort();
    manifests
}
