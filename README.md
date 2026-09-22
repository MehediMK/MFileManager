# File Manager

A fast, modern file manager for Ubuntu/Linux built with **Rust** and **Tauri 2**.

## Features

- 📁 Browse directories with breadcrumbs, back/forward/up navigation
- 🔍 Search files by name (case-insensitive, configurable depth)
- 📋 Copy / Cut / Paste with auto-rename on conflict (`name (copy 1).txt`)
- 🗑️ Move to Trash (XDG trash) or delete permanently
- ✏️ Rename, create files & folders
- 📊 Disk usage panel (per mount point)
- 🔁 Find duplicate files by content hash (SHA-256)
- 📝 Batch rename with pattern + counter
- 👁️ Toggle hidden files (persisted setting)
- 🖱️ Context menu (open, rename, copy, cut, delete, properties)
- ⌨️ Keyboard shortcuts: `Ctrl+C/V`, `F2`, `Delete`, `Backspace`, `Ctrl+H`, `Ctrl+F`, `Ctrl+R`
- 🏠 Sidebar with Home, Desktop, and XDG user dirs
- 📱 Dark theme, custom icons per file type
- ⚡ Single binary, minimal memory footprint

## Stack

| Layer | Tech |
|-------|------|
| Backend | Rust 2024 + Tauri 2 |
| Frontend | Vanilla HTML / CSS / JS (no framework, loads instantly) |
| FS ops | `walkdir`, `trash`, `sha2`, `sysinfo` |

## Prerequisites

Ubuntu / Debian system packages (see `setup-deps.sh`):

```bash
./setup-deps.sh
```

Which installs: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, and build tools.

## Build

```bash
# Debug build
cargo build

# Run dev
cargo tauri dev

# Release build
cargo tauri build --bundles deb,appimage,rpm
```

## Project Structure

```
file-manager/
├── Cargo.toml
├── build.rs               # tauri-build
├── tauri.conf.json        # Tauri window/bundle config
├── setup-deps.sh          # Install system deps
├── icons/                 # Generated app icons
├── src/
│   ├── main.rs            # Entry point
│   ├── lib.rs             # Tauri builder + command registry
│   ├── models.rs          # FileEntry, SearchResult, DiskInfo, etc.
│   ├── commands.rs        # All file-system Tauri commands
│   └── frontend/
│       ├── index.html     # UI layout
│       ├── styles.css     # Dark theme styling
│       └── script.js      # UI logic + invoke() calls
├── LICENSE                # MIT
└── README.md
```

## Tauri Commands

| Command | Purpose |
|---------|---------|
| `list_dir` | List directory entries with metadata |
| `file_info` | Detailed info for a single path |
| `create_dir` / `create_file` | Create items |
| `delete_path` | Trash or permanent delete |
| `rename_path` | Rename in place |
| `copy_item` / `move_item` | Copy/move with conflict handling |
| `search_files` | Recursive name search |
| `duplicates_search` | SHA-256 duplicate finder |
| `batch_rename` | Pattern-based bulk rename |
| `disk_usage` | Disk info via sysinfo |
| `hidden_files_setting` / `get_hidden_setting` | Persist UI prefs |
| `home_dir` / `cli_open` | Helpers |

## Shortcuts

| Key | Action |
|-----|--------|
| `Ctrl+C` | Copy |
| `Ctrl+V` | Paste |
| `F2` | Rename |
| `Delete` | Move to trash |
| `Backspace` | Go up |
| `Ctrl+H` | Toggle hidden files |
| `Ctrl+F` | Search |
| `Ctrl+R` | Refresh |
| `Ctrl+Click` | Multi-select |

## Contributing

PRs welcome. Please:

1. Fork & create a feature branch
2. Follow existing code style (no comments unless asked)
3. Ensure `cargo build` passes
4. Open a PR

## License

[MIT](LICENSE)