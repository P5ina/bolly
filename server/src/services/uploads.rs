use std::{
    fs,
    io::{self, ErrorKind},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::domain::upload::UploadMeta;

const MAX_FILE_SIZE: u64 = 20 * 1024 * 1024; // 20 MB

const BLOCKED_EXTENSIONS: &[&str] = &[
    "exe", "dll", "so", "dylib", "bin", "msi", "dmg", "iso",
    "bat", "cmd", "com", "scr", "vbs", "wsh",
];

pub fn validate_upload(name: &str, size: u64) -> Result<String, String> {
    if size > MAX_FILE_SIZE {
        return Err(format!("file too large ({} bytes, max {})", size, MAX_FILE_SIZE));
    }

    let ext = name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();

    if ext.is_empty() {
        return Err("file must have an extension".into());
    }

    if BLOCKED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(format!("file type '.{ext}' is not allowed"));
    }

    Ok(ext)
}

fn mime_from_ext(ext: &str) -> &'static str {
    match ext {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "json" => "application/json",
        "csv" => "text/csv",
        "md" => "text/markdown",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "xml" => "text/xml",
        "yaml" | "yml" => "text/yaml",
        "toml" => "text/plain",
        "txt" | "log" | "ini" | "cfg" | "conf" | "env" => "text/plain",
        "py" | "rs" | "js" | "ts" | "jsx" | "tsx" | "go" | "rb" | "java"
        | "c" | "cpp" | "h" | "hpp" | "cs" | "swift" | "kt" | "scala"
        | "sh" | "bash" | "zsh" | "fish" | "ps1"
        | "sql" | "graphql" | "proto"
        | "svelte" | "vue" | "astro"
        | "dockerfile" | "makefile" | "cmake"
        | "r" | "lua" | "php" | "pl" | "ex" | "exs" | "zig" | "nim"
        | "dart" | "elm" | "clj" | "hs" | "ml" | "fs" | "erl" => "text/plain",
        _ => "application/octet-stream",
    }
}

pub fn save_upload(
    workspace_dir: &Path,
    instance_slug: &str,
    original_name: &str,
    bytes: &[u8],
) -> io::Result<UploadMeta> {
    let ext = validate_upload(original_name, bytes.len() as u64)
        .map_err(|e| io::Error::new(ErrorKind::InvalidInput, e))?;

    let uploads_dir = workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("uploads");
    fs::create_dir_all(&uploads_dir)?;

    let ts = unix_millis();
    let id = format!("upload_{ts}");
    let stored_name = format!("{id}.{ext}");
    let mime_type = mime_from_ext(&ext).to_string();

    // Write the file
    fs::write(uploads_dir.join(&stored_name), bytes)?;

    let meta = UploadMeta {
        id: id.clone(),
        original_name: original_name.to_string(),
        stored_name,
        mime_type,
        size: bytes.len() as u64,
        uploaded_at: ts.to_string(),
    };

    // Write metadata sidecar
    let json = serde_json::to_string_pretty(&meta)
        .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
    fs::write(uploads_dir.join(format!("{id}.json")), json)?;

    log::info!("[uploads] saved {id} ({original_name}, {} bytes) for {instance_slug}", bytes.len());
    Ok(meta)
}

pub fn list_uploads(workspace_dir: &Path, instance_slug: &str) -> io::Result<Vec<UploadMeta>> {
    let uploads_dir = workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("uploads");

    if !uploads_dir.is_dir() {
        return Ok(vec![]);
    }

    let mut uploads = Vec::new();
    for entry in fs::read_dir(&uploads_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        match fs::read_to_string(&path) {
            Ok(raw) => match serde_json::from_str::<UploadMeta>(&raw) {
                Ok(meta) => uploads.push(meta),
                Err(e) => log::warn!("skipping malformed upload meta {}: {e}", path.display()),
            },
            Err(e) => log::warn!("failed to read upload meta {}: {e}", path.display()),
        }
    }

    uploads.sort_by(|a, b| b.uploaded_at.cmp(&a.uploaded_at));
    Ok(uploads)
}

pub fn get_upload(workspace_dir: &Path, instance_slug: &str, upload_id: &str) -> io::Result<Option<UploadMeta>> {
    let path = workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("uploads")
        .join(format!("{upload_id}.json"));

    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)?;
    let meta: UploadMeta =
        serde_json::from_str(&raw).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
    Ok(Some(meta))
}

pub fn get_upload_file_path(workspace_dir: &Path, instance_slug: &str, upload_id: &str) -> Option<std::path::PathBuf> {
    let meta = get_upload(workspace_dir, instance_slug, upload_id).ok()??;
    let path = workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("uploads")
        .join(&meta.stored_name);
    path.exists().then_some(path)
}

pub fn delete_upload(workspace_dir: &Path, instance_slug: &str, upload_id: &str) -> io::Result<bool> {
    let uploads_dir = workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("uploads");

    let meta_path = uploads_dir.join(format!("{upload_id}.json"));
    if !meta_path.exists() {
        return Ok(false);
    }

    // Read meta to find the blob file
    if let Ok(raw) = fs::read_to_string(&meta_path) {
        if let Ok(meta) = serde_json::from_str::<UploadMeta>(&raw) {
            let blob_path = uploads_dir.join(&meta.stored_name);
            let _ = fs::remove_file(&blob_path);
        }
    }

    fs::remove_file(&meta_path)?;
    Ok(true)
}

/// Read the content of a text-based upload. Returns None for binary files.
pub fn read_upload_text(workspace_dir: &Path, instance_slug: &str, upload_id: &str) -> io::Result<Option<String>> {
    let meta = match get_upload(workspace_dir, instance_slug, upload_id)? {
        Some(m) => m,
        None => return Err(io::Error::new(ErrorKind::NotFound, "upload not found")),
    };

    let is_text = meta.mime_type.starts_with("text/") || meta.mime_type == "application/json";

    if !is_text {
        return Ok(None);
    }

    let path = workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("uploads")
        .join(&meta.stored_name);

    let content = fs::read_to_string(&path)?;
    // Limit to 10k chars for LLM context
    let truncated: String = content.chars().take(10_000).collect();
    Ok(Some(truncated))
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis()
}
