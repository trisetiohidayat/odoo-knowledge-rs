use std::path::Path;

pub fn classify_file(path: &Path) -> (Option<&'static str>, Option<&'static str>) {
    let language = match path.extension().and_then(|value| value.to_str()) {
        Some("py") => Some("python"),
        Some("xml") => Some("xml"),
        Some("csv") => Some("csv"),
        Some("js") => Some("javascript"),
        Some("scss" | "css") => Some("style"),
        _ => None,
    };

    let parts: Vec<String> = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_string())
        .collect();
    let has = |segment: &str| parts.iter().any(|part| part == segment);
    let role = if has("models") {
        Some("models")
    } else if has("controllers") {
        Some("controllers")
    } else if has("views") {
        Some("views")
    } else if has("security") {
        Some("security")
    } else if has("static") {
        Some("static")
    } else if has("data") {
        Some("data")
    } else {
        None
    };
    (language, role)
}
