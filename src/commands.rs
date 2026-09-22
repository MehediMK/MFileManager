use crate::models::{DiskInfo, DuplicateGroup, FileEntry, PreviewInfo, RecentFile, SearchResult};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Local};
use mime_guess::from_path;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[tauri::command]
pub fn home_dir() -> Result<String, String> {
    dirs_home().map_err(|e| e.to_string())
}

fn dirs_home() -> Result<String> {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| anyhow!("could not determine home directory"))
}

fn format_time(ts: Option<std::time::SystemTime>) -> String {
    match ts {
        Some(t) => {
            let dt: DateTime<Local> = t.into();
            dt.format("%Y-%m-%d %H:%M:%S").to_string()
        }
        None => String::new(),
    }
}

#[tauri::command]
pub fn list_dir(path: String) -> Result<Vec<FileEntry>, String> {
    let p = PathBuf::from(&path);
    if !p.is_dir() {
        return Err(format!("{} is not a directory", path));
    }

    let mut entries = Vec::new();
    let read_dir = fs::read_dir(&p).map_err(|e| format!("failed to read {}: {}", path, e))?;

    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let name = entry.file_name().to_string_lossy().to_string();
        let hidden = name.starts_with('.') || meta.file_type().is_symlink();

        let extension = if meta.is_file() {
            Path::new(&name)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default()
        } else {
            String::new()
        };

        let mime = from_path(&name).first_or_octet_stream().to_string();
        let perm = format!("{:o}", meta.permissions().mode() & 0o777);

        entries.push(FileEntry::new(
            name,
            entry.path().to_string_lossy().to_string(),
            meta.is_dir(),
            meta.file_type().is_symlink(),
            meta.len(),
            format_time(meta.modified().ok()),
            format_time(meta.created().ok()),
            perm,
            extension,
            mime,
            hidden,
        ));
    }

    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

#[tauri::command]
pub fn file_info(path: String) -> Result<FileEntry, String> {
    let p = PathBuf::from(&path);
    let meta = fs::metadata(&p).map_err(|e| format!("{}", e))?;
    let name = p
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    let extension = if meta.is_file() {
        p.extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    } else {
        String::new()
    };

    let mime = from_path(&name).first_or_octet_stream().to_string();

    Ok(FileEntry::new(
        name,
        p.to_string_lossy().to_string(),
        meta.is_dir(),
        meta.file_type().is_symlink(),
        meta.len(),
        format_time(meta.modified().ok()),
        format_time(meta.created().ok()),
        format!("{:o}", meta.permissions().mode() & 0o777),
        extension,
        mime,
        path.starts_with("."),
    ))
}

#[tauri::command]
pub fn create_dir(parent: String, name: String) -> Result<(), String> {
    let dir = Path::new(&parent).join(&name);
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create directory: {}", e))
}

#[tauri::command]
pub fn create_file(parent: String, name: String) -> Result<(), String> {
    let file = Path::new(&parent).join(&name);
    if file.exists() {
        return Err(format!("{} already exists", file.display()));
    }
    fs::File::create(&file)
        .map_err(|e| format!("failed to create file: {}", e))
        .map(|_| ())
}

#[tauri::command]
pub fn delete_path(path: String, permanent: bool) -> Result<(), String> {
    if permanent {
        let p = PathBuf::from(&path);
        if p.is_dir() {
            fs::remove_dir_all(&p).map_err(|e| format!("failed to delete directory: {}", e))
        } else {
            fs::remove_file(&p).map_err(|e| format!("failed to delete file: {}", e))
        }
    } else {
        // Move to trash using trash crate
        let p = PathBuf::from(&path);
        trash::delete(&p).map_err(|e| format!("failed to move to trash: {}", e))
    }
}

#[tauri::command]
pub fn rename_path(old_path: String, new_name: String) -> Result<(), String> {
    let src = PathBuf::from(&old_path);
    let parent = src
        .parent()
        .ok_or_else(|| format!("cannot determine parent of {}", old_path))?;
    let dst = parent.join(&new_name);

    if dst.exists() {
        return Err(format!("{} already exists", new_name));
    }

    fs::rename(&src, &dst).map_err(|e| format!("failed to rename: {}", e))
}

#[tauri::command]
pub fn copy_item(src: String, dst_dir: String) -> Result<(), String> {
    let source = PathBuf::from(&src);
    let name = source
        .file_name()
        .ok_or_else(|| "cannot determine filename".to_string())?;
    let dest = PathBuf::from(&dst_dir).join(name);

    // Handle name conflicts
    let mut final_dest = dest.clone();
    let mut counter = 1;
    while final_dest.exists() {
        let stem = source
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let ext = source
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();

        let new_name = if ext.is_empty() {
            format!("{} (copy {})", stem, counter)
        } else {
            format!("{} (copy {}).{}", stem, counter, ext)
        };
        final_dest = PathBuf::from(&dst_dir).join(new_name);
        counter += 1;
    }

    if source.is_dir() {
        copy_dir_recursive(&source, &final_dest)
            .map_err(|e| format!("failed to copy directory: {}", e))?;
    } else {
        fs::copy(&source, &final_dest).map_err(|e| format!("failed to copy file: {}", e))?;
    }

    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn move_item(src: String, dst_dir: String) -> Result<(), String> {
    let source = PathBuf::from(&src);
    let name = source
        .file_name()
        .ok_or_else(|| "cannot determine filename".to_string())?;
    let dest = PathBuf::from(&dst_dir).join(name);

    if dest.exists() {
        return Err(format!(
            "{} already exists at destination",
            name.to_string_lossy()
        ));
    }

    fs::rename(&source, &dest).map_err(|e| format!("failed to move: {}", e))
}

#[tauri::command]
pub fn search_files(
    base_dir: String,
    query: String,
    case_sensitive: bool,
    max_depth: usize,
) -> Result<Vec<SearchResult>, String> {
    let base = PathBuf::from(&base_dir);
    let q = if case_sensitive {
        query.clone()
    } else {
        query.to_lowercase()
    };

    let mut results = Vec::new();
    let walker = WalkDir::new(&base)
        .max_depth(max_depth)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_hidden_dir(e));

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let name = entry.file_name().to_string_lossy().to_string();
        let matched = if case_sensitive {
            name.contains(&q)
        } else {
            name.to_lowercase().contains(&q)
        };

        if matched {
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            results.push(SearchResult {
                path: entry.path().to_string_lossy().to_string(),
                name,
                is_dir: meta.is_dir(),
                size: meta.len(),
                modified: format_time(meta.modified().ok()),
            });
        }
    }

    results.truncate(1000);
    Ok(results)
}

fn is_hidden_dir(entry: &walkdir::DirEntry) -> bool {
    entry.file_name().to_string_lossy().starts_with(".") && entry.depth() > 0
}

#[tauri::command]
pub fn duplicates_search(base_dir: String) -> Result<Vec<DuplicateGroup>, String> {
    let mut size_map: HashMap<u64, Vec<PathBuf>> = HashMap::new();

    for entry in WalkDir::new(&base_dir)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_file()
            && let Ok(meta) = entry.metadata()
        {
            size_map
                .entry(meta.len())
                .or_default()
                .push(entry.path().to_path_buf());
        }
    }

    let mut groups = Vec::new();
    for (size, paths) in size_map {
        if paths.len() < 2 {
            continue;
        }

        let mut hash_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
        for p in &paths {
            if let Ok(h) = hash_file(p) {
                hash_map.entry(h).or_default().push(p.clone());
            }
        }

        for (hash, files) in hash_map {
            if files.len() > 1 {
                groups.push(DuplicateGroup {
                    size,
                    hash,
                    files: files
                        .into_iter()
                        .map(|f| f.to_string_lossy().to_string())
                        .collect(),
                });
            }
        }
    }

    groups.sort_by_key(|g| std::cmp::Reverse(g.size));
    Ok(groups)
}

fn hash_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect())
}

#[tauri::command]
pub fn batch_rename(
    dir: String,
    pattern: String,
    start: u32,
    padding: usize,
) -> Result<Vec<(String, String)>, String> {
    let base = PathBuf::from(&dir);
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(&base).map_err(|e| format!("{}", e))?;

    for entry in read_dir.flatten() {
        entries.push(entry.file_name().to_string_lossy().to_string());
    }
    entries.sort();

    let mut renamed: Vec<(String, String)> = Vec::new();

    for (i, name) in entries.iter().enumerate() {
        let p = base.join(name);
        if !p.is_file() {
            continue;
        }

        let ext = Path::new(name)
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();

        let num = start + i as u32;
        let number = format!("{:0width$}", num, width = padding);
        let new_name = if ext.is_empty() {
            format!("{}_{}", pattern, number)
        } else {
            format!("{}_{}.{}", pattern, number, ext)
        };

        let new_path = base.join(&new_name);
        if fs::rename(&p, &new_path).is_ok() {
            renamed.push((name.clone(), new_name));
        }
    }

    Ok(renamed)
}

#[tauri::command]
pub fn disk_usage() -> Result<Vec<DiskInfo>, String> {
    use sysinfo::Disks;

    let mut disks = Disks::new_with_refreshed_list();
    disks.refresh(false);

    let mut result = Vec::new();
    for disk in disks.list() {
        result.push(DiskInfo {
            device: disk.name().to_string_lossy().to_string(),
            mount_point: disk.mount_point().to_string_lossy().to_string(),
            total: disk.total_space(),
            free: disk.available_space(),
            used: disk.total_space().saturating_sub(disk.available_space()),
            used_percent: if disk.total_space() > 0 {
                ((disk.total_space() - disk.available_space()) * 100 / disk.total_space()) as u8
            } else {
                0
            },
        });
    }

    Ok(result)
}

#[tauri::command]
pub fn recent_files() -> Result<Vec<RecentFile>, String> {
    let mut files = Vec::new();
    let mut cache = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    cache.push("file-manager");
    cache.push("recent.json");

    if cache.exists() {
        let content = fs::read_to_string(&cache).map_err(|e| e.to_string())?;
        let parsed: Vec<RecentFile> = serde_json::from_str(&content).unwrap_or_default();
        files = parsed;
    }

    Ok(files)
}

#[tauri::command]
pub fn hidden_files_setting(value: bool) -> Result<(), String> {
    let mut settings = read_settings();
    settings["show_hidden"] = serde_json::Value::Bool(value);
    write_settings(&settings)
}

#[tauri::command]
pub fn get_hidden_setting() -> Result<bool, String> {
    Ok(read_settings()
        .get("show_hidden")
        .and_then(|v| v.as_bool())
        .unwrap_or(false))
}

#[tauri::command]
pub fn background_setting(path: Option<String>) -> Result<(), String> {
    let mut settings = read_settings();
    match path {
        Some(p) if !p.is_empty() => {
            settings["background"] = serde_json::Value::String(p);
        }
        _ => {
            settings["background"] = serde_json::Value::Null;
        }
    }
    write_settings(&settings)
}

#[tauri::command]
pub fn get_background_setting() -> Result<Option<String>, String> {
    Ok(read_settings()
        .get("background")
        .and_then(|v| v.as_str())
        .map(String::from))
}

#[tauri::command]
pub async fn pick_image(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter(
            "Images",
            &["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg"],
        )
        .pick_file(move |f| {
            let p = f
                .and_then(|f| f.into_path().ok())
                .map(|p| p.to_string_lossy().to_string());
            let _ = tx.send(p);
        });
    rx.await
        .map_err(|_| "file picker was interrupted".to_string())
}

#[tauri::command]
pub fn load_image_data(path: String) -> Result<String, String> {
    use base64::Engine;
    let p = PathBuf::from(&path);
    let data = fs::read(&p).map_err(|e| format!("failed to read image: {}", e))?;
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        _ => "image/png",
    };
    Ok(format!(
        "data:{};base64,{}",
        mime,
        base64::engine::general_purpose::STANDARD.encode(data)
    ))
}

fn settings_path() -> PathBuf {
    let mut cache = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    cache.push("file-manager");
    cache.push("settings.json");
    cache
}

fn read_settings() -> serde_json::Value {
    let path = settings_path();
    if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        serde_json::json!({})
    }
}

fn write_settings(settings: &serde_json::Value) -> Result<(), String> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cli_open(path: String) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(|e| format!("failed to open {}: {}", path, e))?;
    Ok(())
}

#[tauri::command]
pub fn open_terminal(path: String) -> Result<(), String> {
    use std::process::{Command, Stdio};

    const CANDIDATES: [&str; 10] = [
        "x-terminal-emulator",
        "gnome-terminal",
        "konsole",
        "xfce4-terminal",
        "alacritty",
        "kitty",
        "terminator",
        "tilix",
        "xterm",
        "xdg-terminal-exec",
    ];

    fn present(name: &str) -> bool {
        ["/bin", "/usr/bin", "/usr/local/bin", "/snap/bin"]
            .iter()
            .any(|d| Path::new(d).join(name).exists())
    }

    let term = CANDIDATES
        .iter()
        .find(|t| present(t))
        .ok_or_else(|| "no terminal emulator found".to_string())?;

    let dir = if path.is_empty() || path == "~" {
        dirs_home().unwrap_or_else(|_| "/".to_string())
    } else {
        path
    };
    let quoted = format!("'{}'", dir.replace('\'', "'\\''"));
    let script = format!("cd {} && exec {}", quoted, term);

    Command::new("sh")
        .arg("-c")
        .arg(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to open terminal ({}): {}", term, e))?;

    log::info!("opened terminal {} in {}", term, dir);
    Ok(())
}

#[tauri::command]
pub fn preview_file(path: String) -> Result<PreviewInfo, String> {
    use base64::Engine;

    let p = PathBuf::from(&path);
    let meta = fs::metadata(&p).map_err(|e| format!("cannot preview: {}", e))?;
    let name = p
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());
    let mime_type = from_path(&p).first_or_octet_stream().to_string();
    let size = meta.len();

    if meta.is_dir() {
        return Ok(PreviewInfo {
            name,
            kind: "dir".into(),
            size,
            mime_type,
            text: None,
            data_url: None,
        });
    }

    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    const TEXT_EXTS: [&str; 24] = [
        "txt", "md", "rs", "c", "h", "cpp", "py", "js", "ts", "tsx", "jsx", "json", "toml", "yaml",
        "yml", "sh", "html", "css", "xml", "log", "csv", "ini", "conf", "svg",
    ];

    let kind = if mime_type.starts_with("image/") {
        "image"
    } else if mime_type.starts_with("text/") || TEXT_EXTS.contains(&ext.as_str()) {
        "text"
    } else if mime_type == "application/pdf" {
        "pdf"
    } else if mime_type.starts_with("audio/") {
        "audio"
    } else if mime_type.starts_with("video/") {
        "video"
    } else {
        "binary"
    };

    let mut text = None;
    let mut data_url = None;
    match kind {
        "text" => {
            if size < 2_000_000 {
                let bytes = fs::read(&p).unwrap_or_default();
                let mut s = String::from_utf8_lossy(&bytes).into_owned();
                if s.chars().count() > 5000 {
                    s = s.chars().take(5000).collect::<String>() + "\n… [preview truncated]";
                }
                text = Some(s);
            }
        }
        "image" | "audio" | "video" => {
            let max = if kind == "image" {
                8_000_000u64
            } else {
                12_000_000u64
            };
            if size <= max {
                let bytes = fs::read(&p).map_err(|e| e.to_string())?;
                data_url = Some(format!(
                    "data:{};base64,{}",
                    mime_type,
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                ));
            }
        }
        _ => {}
    }

    Ok(PreviewInfo {
        name,
        kind: kind.to_string(),
        size,
        mime_type,
        text,
        data_url,
    })
}

#[tauri::command]
pub fn compress_zip(items: Vec<String>, dest_dir: String, name: String) -> Result<(), String> {
    use std::io::{BufWriter, Seek, Write};
    use zip::CompressionMethod;
    use zip::write::SimpleFileOptions;

    let trimmed = name.trim().to_string();
    if trimmed.is_empty() {
        return Err("archive name cannot be empty".to_string());
    }
    let dest = PathBuf::from(&dest_dir).join(if trimmed.to_lowercase().ends_with(".zip") {
        trimmed
    } else {
        format!("{}.zip", trimmed)
    });

    let file = fs::File::create(&dest).map_err(|e| format!("failed to create archive: {}", e))?;
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut zip = zip::ZipWriter::new(BufWriter::new(file));

    fn add_path<W: Write + Seek>(
        zip: &mut zip::ZipWriter<W>,
        path: &Path,
        prefix: &str,
        options: &SimpleFileOptions,
    ) -> std::io::Result<()> {
        let leaf = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let zip_name = if prefix.is_empty() {
            leaf
        } else {
            format!("{}/{}", prefix, leaf)
        };

        if path.is_dir() {
            zip.add_directory(format!("{}/", zip_name), *options)?;
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                add_path(zip, &entry.path(), &zip_name, options)?;
            }
        } else if path.is_symlink() {
            let link = fs::read_link(path)?;
            zip.start_file(zip_name, *options)?;
            zip.write_all(link.to_string_lossy().as_bytes())?;
        } else {
            zip.start_file(zip_name, *options)?;
            let mut f = fs::File::open(path)?;
            std::io::copy(&mut f, zip)?;
        }
        Ok(())
    }

    for item in &items {
        let p = PathBuf::from(item);
        if p.exists() {
            add_path(&mut zip, &p, "", &options)
                .map_err(|e| format!("failed to add {}: {}", p.display(), e))?;
        }
    }

    zip.finish()
        .map_err(|e| format!("failed to write archive: {}", e))?;
    log::info!("created archive {}", dest.display());
    Ok(())
}
