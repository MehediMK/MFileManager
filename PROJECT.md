# Project Architecture & Maintenance Guide

> Purpose: explain how this Tauri 2 file manager works — the logic, how files
> connect, and the exact places to touch when adding a feature. Written so an AI
> model (or future maintainer) can make safe edits with minimal exploration.

## 1. TL;DR

- **App**: a single-window file manager for Ubuntu/Linux.
- **UI**: vanilla HTML/CSS/JS in `src/frontend/` (no framework, no bundler, no npm).
  Tauri serves these files as the app's `frontendDist`.
- **Backend**: Rust commands in `src/commands.rs`, models in `src/models.rs`,
  registered in `src/lib.rs`. JS calls them via `window.__TAURI__.core.invoke`.
- **Key config facts that MUST stay true** (breaking them breaks the app):
  - `tauri.conf.json`: `frontendDist = "src/frontend"`, `app.withGlobalTauri: true`.
  - `src/frontend/styles.css` MUST keep `.hidden { display: none !important; }`
    (toggle for modal/context menu/toast/preview pane).
  - `src/main.rs` calls `file_manager::run()` (the lib target is named `file_manager`).
- `cargo build`, `cargo check`, `cargo clippy -- -D warnings`, `node --check src/frontend/script.js` must all pass before committing.

## 2. Project layout

```
file-manager/
├── Cargo.toml            # deps. NOTE: the app depends on tauri plugins; see §7
├── build.rs              # tauri-build (required — do not delete)
├── tauri.conf.json       # window size, bundling, withGlobalTauri, frontendDist
├── .gitignore            # ignores /target, bundle outputs, /gen, logs
├── icon-source.png       # 1024px source for `cargo tauri icon`
├── icons/                # generated icons (do not edit by hand)
├── setup-deps.sh         # installs Ubuntu dev packages for a clean build
├── docs/                 # user documentation + screenshot pipeline (§9)
├── target/               # build output (gitignored)
└── src/
    ├── main.rs           # thin entry → file_manager::run()
    ├── lib.rs            # Tauri builder: plugins + invoke_handler (the registry)
    ├── models.rs         # serializable structs shared with the frontend
    ├── commands.rs       # ALL backend commands (filesystem + helpers)
    └── frontend/
        ├── index.html    # full UI markup
        ├── styles.css    # dark theme, responsive rules, component styles
        └── script.js     # ALL app logic (tabs, navigation, rendering, IPC)
```

## 3. How the pieces connect (data flow)

```
index.html ──loads──► styles.css (style)
   └────────────loads──► script.js (logic)
                            │
                window.__TAURI__.core.invoke('cmd', args)
                            │   (JSON-RPC via Tauri IPC)
                            ▼
                 src/lib.rs  invoke_handler
                            │   (tauri::generate_handler![...])
                            ▼
                   src/commands.rs  #[tauri::command] fns
                            │   (use models::* from src/models.rs)
                            ▼
              filesystem / sysinfo / trash / zip / gvfs
```

- `script.js` communicates exclusively through a helper `send(cmd, args)` which wraps
  `invoke`. Nothing else touches `__TAURI__` directly.
- Every backend command that returns data returns a `serde`-serializable struct from
  `models.rs` (camelCase on the wire). Command args arrive camelCase too
  (e.g. `destDir`, `isDir`, `lastOpened`).
- Errors: commands return `Result<T, String>`; the `Err(String)` surfaces in
  `script.js` as the rejected promise value and is shown via `showToast(err, true)`.

## 4. Frontend logic (script.js) — the part that matters most

### 4.1 State model (multi-tab via Proxy)
`state` is a `Proxy` over two layers:
- `shared` — app-wide: `clipboard` ({items, action}), `hiddenShown`.
- per-tab object (created by `makeTab(path)`) — navigation scope:
  `currentPath`, `history[]`, `historyIndex`, `selectedItems (Set)`,
  `contextTarget`, `isSearching`, `searchResults`.

How it works:
```js
const shared = { clipboard: null, hiddenShown: false };
const perTab = new Map();   // id -> tab state object
let activeTabId = null;
const state = new Proxy(shared, {
  get(t, p){ if (p in t) return t[p]; const tab=perTab.get(activeTabId); return tab ? tab[p] : undefined; },
  set(t, p, v){ if (p in t){ t[p]=v; return true; } const tab=perTab.get(activeTabId); if(tab) tab[p]=v; return true; }
});
```
- Reading/writing `state.currentPath` etc. always hits the ACTIVE tab.
- `makeTab` must be updated if you add a new per-tab field.
- Tab functions: `newTab(path, activate)`, `switchTab(id)`, `closeTab(id)`, `renderTabs()`.
- Tab bar markup lives in `index.html` (`#tab-bar` / `#tabs` / `#btn-new-tab`).

### 4.2 Navigation lifecycle
`navigate(path)` (async):
1. `send('list_dir', { path })` → entries → `renderList(entries)`.
2. Sets `state.currentPath`, resets `isSearching`/`searchResults`.
3. Maintains per-tab `history`/`historyIndex` (guarded push — no duplicates).
4. Clears `selectedItems`, updates path input, nav buttons, statusbar, sidebar highlight, and `updatePreview()`.
5. Errors → toast (e.g. deleted folder).

Back/forward/up use `historyIndex`; these buttons read/write the active tab's history.

### 4.3 Rendering
- `renderList(entries)` renders a `..` up-row (if not root) then one `.file-row` per entry via `createFileRow(entry)`.
- Row events: click (`selectOnly`/`toggleSelect`), dblclick (dir→navigate, file→`cli_open`),
  contextmenu (→`showContextMenu(x, y, {path, isDir})`), plus HTML5 drag events.
- `addDragBehavior()` is called AFTER rendering — it attaches drag/drop to freshly created rows.
- Search results render through a separate path (`renderSearchResults`-style block setting `state.isSearching`), not `renderList`.

### 4.4 Context menu
- `showContextMenu(x, y, target)` shows `#context-menu`; `needsItem` list controls which
  items are hidden when `target` is null (empty-area right-click).
  - Always-visible: New File/Folder, Search, Open Terminal, Set/Reset Background, Paste (only if clipboard).
  - Add new per-target actions to `needsItem`.
- A document-level `contextmenu` listener calls `e.preventDefault()` EVERYWHERE except on
  `.file-row`/`#context-menu`. This kills WebKitGTK's default "Inspect Element" menu — do not remove.
- `handleAction(action)` is big `switch` — add new cases there.

### 4.5 Modals & confirmations
- Native `confirm()`/`prompt()` are FORBIDDEN (README trouble #2; WebKitGTK+Wayland crashes).
  Use `safeConfirm(message)` / `showModal(title, contentHTML)` / `closeModal()`.
- `#modal-overlay` in index.html; `modal-content` is injected HTML; wire `#modal-confirm`/`#modal-cancel` in each caller.

### 4.6 Preview pane
- `btn-preview` toggles `#preview-pane` visibility; `togglePreview(force)`, `updatePreview()`, `renderPreview(info, path)`.
- Renders: image/audio/video → `dataUrl` (base64 from backend); text → `<pre>`; pdf/binary → "Open in app".
- Selection changes call `updatePreview()`; navigate clears it.

### 4.7 Drag & drop
- `row.draggable = true` in `createFileRow`. `dragstart` stores paths (multi-selection aware) in
  `dataTransfer` + `state.draggingPaths`; effectAllowed `copyMove`.
- Drop targets: `.dir-row`, the `..` row (`dataset.path` = parent), and sidebar items with `dataset.dropPath`.
- `moveDropped(e, targetDir)` → `move_item` (default) or `copy_item` on `Ctrl/Shift`.
- Sidebar delegation on `#sidebar` (`dragover`/`drop`); highlight via `.drop-target` class.

### 4.8 Search
- Toolbar `#search-input` (focus via `Ctrl+F`). Enter → `startSearch()` → `send('search_files')`.
- Per-tab `isSearching`/`searchResults`; result rows reuse row events (contextmenu passes search result object).

## 5. Backend commands (src/commands.rs) — full inventory

Commands registered in `src/lib.rs` invoke_handler (17 + 4 new):
`list_dir`, `file_info`, `create_dir`, `create_file`, `delete_path`, `rename_path`,
`copy_item`, `move_item`, `home_dir`, `search_files`, `duplicates_search`,
`batch_rename`, `disk_usage`, `recent_files`, `hidden_files_setting`,
`get_hidden_setting`, `cli_open`, `open_terminal`, `preview_file`, `compress_zip`,
plus background: `background_setting`, `get_background_setting`, `pick_image`, `load_image_data`.

Key signatures / behavior:
- `list_dir(path)` → `Vec<FileEntry>`; hides hidden files ONLY if `state.hiddenShown` (frontend filters; backend returns `hidden` flag per entry).
- `delete_path(path, permanent)` — false → XDG trash via `trash` crate; true → `remove_dir_all`/`remove_file`.
- `copy_item(src, dst_dir)` / `move_item(src, dst_dir)` — auto-rename `name (copy N).ext` on conflict.
- `search_files(path, query, depth)` — case-insensitive walk via `walkdir`.
- `duplicates_search(path)` → groups by SHA-256 (`sha2 0.11` — hex via iterator, NOT `LowerHex` on finalize output, which changed in 0.11).
- `disk_usage()` → `Vec<DiskInfo>` via `sysinfo`.
- `open_terminal(path)` — probes `/bin,/usr/bin,/usr/local/bin,/snap/bin` for
  `x-terminal-emulator`, `gnome-terminal`, `konsole`, ... then runs
  `sh -c "cd '<quoted>' && exec <term>"` with stdin/stdout/stderr nulled.
- `preview_file(path)` → `PreviewInfo { name, kind: image|text|pdf|audio|video|dir|binary, size, mimeType, text?, dataUrl? }`.
  - text read if < 2 MB, truncated to 5000 chars; media base64 if under size caps.
- `compress_zip(items, dest_dir, name)` → ZIP via `zip 4.0` (feature `deflate`+`deflate-flate2`),
  recursive dirs + symlink handling; appends `.zip`.
- `background_setting(path: Option<String>)` / `get_background_setting()` — stored in settings.json (see §6).
- `pick_image(app)` — ASYNC command; uses `app.dialog().file()...pick_file(callback)` bridged with `tokio::sync::oneshot`.
  DO NOT use `blocking_pick_file()` — it freezes the main thread → GNOME "not responding".
- `load_image_data(path)` → `data:<mime>;base64,...`.

## 6. Persistence (important)

- **Hidden files / background**: single JSON at
  `$XDG_CACHE_HOME/file-manager/settings.json` (usually `~/.cache/file-manager/settings.json`).
  Shape: `{ "show_hidden": bool, "background": "/path/image" | null }`.
  Access ONLY through `read_settings()` / `write_settings()` helpers in commands.rs — `hidden_files_setting`
  merges (does not overwrite) so the background survives.
- **Recent files**: `~/.cache/file-manager/recent.json` — array of `RecentFile` (path, name, lastOpened).
- There is no other state; the frontend keeps everything in memory per-tab.

## 7. Dependencies (Cargo.toml)

Key crates: `tauri 2.x`, plugins `dialog`, `opener`, `clipboard-manager`, `shell`,
`notification`, `process`, `http`, `log` (all registered in lib.rs — must match),
`anyhow`, `chrono`(serde), `dirs`, `mime_guess`, `notify`, `serde`+`serde_json`,
`sha2 0.11`, `sysinfo 0.39`, `tokio`(full), `trash`, `walkdir`, `base64 0.22`,
`zip 4` (default-features=false, deflate).
`[build-dependencies] tauri-build = "2.6"`.
NOT present on purpose: `tauri-plugin-fs`, `tauri-plugin-global-shortcut`, macOS process-relaunch flag.

## 8. Editing conventions (for safe changes)

- **Add a backend command**: (1) add `#[tauri::command] pub fn name(...)` in commands.rs
  (use `models.rs` structs for returns), (2) register in lib.rs invoke_handler, (3) call
  from script.js via `send('name', {...})` with camelCase args.
- **Add a UI element**: edit index.html + reuse existing CSS variables (see `:root` vars in styles.css:
  `--bg, --bg-secondary, --bg-hover, --bg-active, --text, --text-dim, --accent, --border, --danger, --success`).
- **Add a context menu action**: add `.menu-item[data-action="x"]`, case in `handleAction`, and if it needs a target add to `needsItem`.
- **No comments in code** unless the task asks for them (existing comments are deliberate/logic markers — keep them).
- Run: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo build`, `node --check src/frontend/script.js`.
- Never commit secrets; keep `Cargo.lock` and `icons/` tracked.

## 9. Documentation & screenshots pipeline (docs/)

- `docs/DOCUMENTATION.md` — user-facing SEO page (Jekyll front matter first; `permalink: /`); served by GitHub Pages.
- `docs/screenshot-src/` — screenshot harness:
  - `index.screenshot.html` = copy of `index.html` + a MOCK `window.__TAURI__` (faked `core.invoke`)
    + query-param post-render hooks (`?shot=main|properties|disk|menu`).
  - `styles.css`, `script.js` are COPIES of `src/frontend/*` — **resync before regenerating**.
- `docs/screenshots/*.png` — 2x captures made by `docs/regenerate-screenshots.sh` using headless Chrome
  (`--headless=new --virtual-time-budget=4000 --window-size=600,400 --force-device-scale-factor=2`).
- `docs/_config.yml` — Jekyll config. `.github/workflows/pages.yml` deploys `./docs` via GitHub Actions.

## 10. Known quirks / gotchas (read before merging changes)

1. WebKitGTK native `confirm()`/`prompt()` can crash on Wayland → always use in-app modals.
2. `renderList` is called after navigating; `addDragBehavior` re-binds each time (rows are recreated).
3. `.hidden { display:none !important }` rule is load-bearing for modal/context/toast/preview.
4. Multi-select: `state.selectedItems` is per-tab. Context menu uses `state.contextTarget` for the right-clicked row.
5. `open_terminal` and `pick_image` must be async/off-main-thread patterns (see §5).
6. `sha2` 0.11: `finalize()` output is not LowerHex-printable directly — iterate bytes → `{:02x}`.
7. Frontend sends camelCase args; Rust `#[serde(rename_all="camelCase")]` on return structs keeps the wire format consistent.
8. screenshots harness needs the exact same markup ids as `index.html`, or the mock page throws.

## 11. Quick feature recipes

- **New sidebar shortcut**: add entry in `loadSidebar()` with `data-dropPath` + click → `navigate(path)`.
- **New toolbar button**: button in `#toolbar .toolbar-actions`; listener in script.js; consider hiding at `≤1150px` if secondary.
- **New file action requiring backend**: follow §8 recipe; return through `models.rs`.
- **New per-tab field**: add to `makeTab()` in script.js (e.g. `sortMode`).
- **New persistent toggle**: extend `read_settings`/`write_settings` key in commands.rs; expose get/set commands.

## 12. Building / packaging

```bash
./setup-deps.sh                 # first time: system dev packages
cargo tauri dev                 # dev with hot reload
cargo tauri build --bundles deb,appimage,rpm   # installers
cargo build --release           # standalone binary (frontend embedded)
./package-tar.sh                # (see §13) portable .tar.gz build
```
`cargo tauri icon ./icon-source.png` regenerates all `icons/` from the source.

## 13. Tar.gz release for Ubuntu

`package-tar.sh` builds `cargo build --release` and packs into
`dist/MFileManager-0.1.0-linux-amd64.tar.gz`. Contents:
- `file-manager` (self-contained ELF — frontend assets are embedded, no extra runtime dirs)
- `file-manager.desktop` (edit the `Exec=` line to the real install path, e.g. `/opt/file-manager/file-manager`)
- `README.md`, `LICENSE`
- `icons/128x128.png` → `file-manager.png` (use as desktop icon)

How to run from the extracted tar:
```bash
tar -xzf MFileManager-0.1.0-linux-amd64.tar.gz
chmod +x file-manager
./file-manager
```

## 14. Current feature checklist (as of last update)

- Tabs (Ctrl+T / Ctrl+W), per-tab history/selection/search
- Toolbar search box (Ctrl+F), recursive search
- Copy/Cut/Paste + conflict rename; Move to Trash / permanent delete (in-app confirm)
- Create/rename files & folders; batch rename; duplicates (SHA-256); disk usage
- Open Terminal here (Ctrl+Alt+T), open files (xdg-open)
- Preview pane (image/audio/video/text), Compress to ZIP (drag & drop move/copy)
- Recent Files dialog; background image (persisted); hidden-files toggle (persisted)
- Custom app icon; dark theme; responsive layout (toolbar wrap, sidebar collapse, secondary-button hiding ≤1150px)
- Context menu with Search/Terminal/Background/ZIP items; WebKit "Inspect Element" suppressed