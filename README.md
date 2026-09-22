# File Manager

A fast, modern file manager for Ubuntu/Linux built with **Rust** and **Tauri 2**.

> 📖 **Full documentation** (screenshots, guides, FAQ): [`docs/DOCUMENTATION.md`](docs/DOCUMENTATION.md)

## Features

- 📑 **Tabs** — multiple folders in one window (`Ctrl+T` new, `Ctrl+W` close); each tab keeps its own history, selection & search state
- 🔍 **Search box** in the toolbar (magnifier icon, focus glow) + recursive name search
- 📁 Browse directories with breadcrumbs, back/forward/up navigation
- 📋 Copy / Cut / Paste with auto-rename on conflict (`name (copy 1).txt`)
- 🗑️ Move to Trash (XDG trash) or delete permanently — in-app confirm dialog (no native popups)
- ✏️ Rename, create files & folders
- 🖼️ **Custom background image** (set via `🖼️` or context menu, persisted between launches)
- 💻 **Open Terminal here** in the current folder (detects gnome-terminal, konsole, xfce4-terminal, xterm, …)
- 📊 Disk usage panel (per mount point)
- 🔁 Find duplicate files by content hash (SHA-256)
- 📝 Batch rename with pattern + counter
- 👁️ Toggle hidden files (persisted setting)
- 🖱️ Context menu (open, rename, copy, cut, paste, delete, properties) — WebKit "Inspect Element" suppressed
- 🏠 Sidebar with Home, Desktop, and XDG user dirs
- 📱 Dark theme, custom icons per file type, custom app icon
- ⚡ Single binary, minimal memory footprint

## Stack

| Layer | Tech |
|-------|------|
| Backend | Rust 2024 + Tauri 2 |
| Frontend | Vanilla HTML / CSS / JS (no framework, loads instantly) |
| FS ops | `walkdir`, `trash`, `sha2`, `sysinfo` |

## Prerequisites

1. **Rust toolchain** (edition 2024):

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Tauri CLI** (needed for `cargo tauri dev` / `cargo tauri build`):

   ```bash
   cargo install tauri-cli --locked
   ```

3. **Ubuntu / Debian system packages** (see `setup-deps.sh`):

   ```bash
   ./setup-deps.sh
   ```

   Which installs: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libglib2.0-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libssl-dev`, `libxdo-dev`, and build tools.

> The app's own commands are invoked via the Tauri global API, so `tauri.conf.json` sets `app.withGlobalTauri: true`. No `capabilities/` folder is required — custom commands are allowed by default.

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
| `background_setting` / `get_background_setting` | Persist background image |
| `pick_image` / `load_image_data` | Choose & load background (async dialog, base64 data URL) |
| `open_terminal` | Open terminal in a folder (auto-detect emulator) |
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
| `Ctrl+F` | Focus search box |
| `Ctrl+R` | Refresh |
| `Ctrl+N` | New file |
| `Ctrl+T` | New tab |
| `Ctrl+W` | Close tab |
| `Ctrl+Alt+T` | Open terminal here |
| `Ctrl+Click` | Multi-select |

## Troubleshooting

- **`pkg-config ... glib-2.0 was not found` / `webkit2gtk-4.1` missing when running `cargo build`:**
  the Rust -dev system packages aren't installed. Run `./setup-deps.sh`.

- **Window opens but shows an empty "Confirm / Cancel" dialog and nothing works:**
  `src/frontend/styles.css` must define a `.hidden { display: none !important; }` rule (used by the modal/context menu/toast), and `tauri.conf.json` requires `"withGlobalTauri": true` — otherwise `window.__TAURI__` is undefined and `script.js` never starts.

- **`error[E0433]: cannot find module or crate tauri_build` at `build.rs`:** the `tauri-build` build-dependency was dropped from `Cargo.toml`. Keep `[build-dependencies] tauri-build = "2"`.

## Contributing

PRs welcome. Please:

1. Fork & create a feature branch
2. Follow existing code style (no comments unless asked)
3. Ensure `cargo build` passes
4. Open a PR

## License

[MIT](LICENSE)