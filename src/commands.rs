use crate::models::{DiskInfo, DuplicateGroup, FileEntry, RecentFile, SearchResult};
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
    let mut cache = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    cache.push("file-manager");
    fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    cache.push("settings.json");

    let settings = serde_json::json!({ "show_hidden": value });
    fs::write(cache, settings.to_string()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_hidden_setting() -> Result<bool, String> {
    let mut cache = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    cache.push("file-manager");
    cache.push("settings.json");

    if cache.exists() {
        let content = fs::read_to_string(&cache).map_err(|e| e.to_string())?;
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
        Ok(parsed
            .get("show_hidden")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    } else {
        Ok(false)
    }
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
    let mut cmd = std::process::Command::new("xdg-terminal-exec");
    if !path.is_empty() {
        cmd.arg(&path);
    }
    cmd.spawn()
        .map_err(|e| format!("failed to open terminal: {}", e))?;
    Ok(())
}
