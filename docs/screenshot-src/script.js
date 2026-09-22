const { invoke } = window.__TAURI__.core;

// Shared (app-wide) settings live on `shared`; per-tab navigation state
// (currentPath/history/selection/search) lives on the active tab object.
const shared = {
    clipboard: null, // { items: [], action: 'copy' | 'cut' }
    hiddenShown: false,
};

const perTab = new Map();
let activeTabId = null;

const state = new Proxy(shared, {
    get(t, prop) {
        if (prop in t) return t[prop];
        const tab = perTab.get(activeTabId);
        return tab ? tab[prop] : undefined;
    },
    set(t, prop, value) {
        if (prop in t) {
            t[prop] = value;
            return true;
        }
        const tab = perTab.get(activeTabId);
        if (tab) tab[prop] = value;
        return true;
    },
});

function makeTab(path) {
    return {
        currentPath: path,
        history: [path],
        historyIndex: 0,
        selectedItems: new Set(),
        contextTarget: null, // path under cursor
        isSearching: false,
        searchResults: [],
    };
}

// ----- Tabs -----

function renderTabs() {
    const cont = document.getElementById('tabs');
    cont.innerHTML = '';
    perTab.forEach((tab, id) => {
        const el = document.createElement('div');
        el.className = 'tab' + (id === activeTabId ? ' active' : '');
        el.addEventListener('click', () => switchTab(id));
        const label = document.createElement('span');
        label.className = 'tab-label';
        label.textContent = tab.currentPath.split('/').filter(Boolean).pop() || tab.currentPath || '/';
        label.title = tab.currentPath;
        const close = document.createElement('button');
        close.className = 'tab-close';
        close.textContent = '×';
        close.title = 'Close tab';
        close.addEventListener('click', (e) => {
            e.stopPropagation();
            closeTab(id);
        });
        el.append(label, close);
        cont.appendChild(el);
    });
}

function newTab(path, activate = true) {
    if (!path) path = state.currentPath || '/';
    const id = 't' + Date.now().toString(36) + Math.random().toString(36).slice(2, 7);
    perTab.set(id, makeTab(path));
    if (activate) {
        activeTabId = id;
        renderTabs();
        navigate(path);
    } else {
        renderTabs();
    }
    return id;
}

function switchTab(id) {
    if (!perTab.has(id) || id === activeTabId) return;
    activeTabId = id;
    renderTabs();
    navigate(state.currentPath);
}

function closeTab(id) {
    if (perTab.size <= 1) return;
    const ids = [...perTab.keys()];
    const idx = ids.indexOf(id);
    const wasActive = id === activeTabId;
    perTab.delete(id);
    if (wasActive) {
        const next = ids[idx + 1] ?? ids[idx - 1];
        activeTabId = next;
    }
    renderTabs();
    if (wasActive) navigate(perTab.get(activeTabId).currentPath);
}

document.getElementById('btn-new-tab').addEventListener('click', () => newTab());

// ----- Helper functions -----

async function send(path, args) {
    return await invoke(path, args || {});
}

function showToast(message, error = false) {
    const toast = document.getElementById('toast');
    toast.textContent = message;
    toast.className = 'toast' + (error ? ' error' : '');
    toast.classList.remove('hidden');
    setTimeout(() => toast.classList.add('hidden'), 3000);
}

function formatSize(bytes) {
    if (bytes === 0) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return (bytes / Math.pow(1024, i)).toFixed(i === 0 ? 0 : 1) + ' ' + units[i];
}

function formatDate(s) {
    if (!s) return '';
    const d = new Date(s.replace(' ', 'T'));
    return d.toLocaleDateString() + ' ' + d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

function getIcon(entry) {
    if (entry.isSymlink) return '🔗';
    if (entry.isDir) return '📁';
    const mime = entry.mimeType || '';
    if (entry.hidden) return '👁️';
    if (mime.startsWith('image/')) return '🖼️';
    if (mime.startsWith('video/')) return '🎬';
    if (mime.startsWith('audio/')) return '🎵';
    switch (entry.extension) {
        case 'pdf': return '📕';
        case 'zip': case 'tar': case 'gz': case '7z': case 'rar': return '🗜️';
        case 'deb': return '🐧';
        case 'sh': return '⚡';
        case 'txt': case 'md': return '📝';
        case 'rs': case 'c': case 'cpp': case 'py': case 'js': case 'ts': case 'go': case 'java': return '💻';
        default: return '📄';
    }
}

function renderProps(entry) {
    return `
        <table class="properties-grid">
            <tr><td class="label">Name</td><td class="value">${escapeHTML(entry.name)}</td></tr>
            <tr><td class="label">Path</td><td class="value">${escapeHTML(entry.path)}</td></tr>
            <tr><td class="label">Type</td><td class="value">${entry.isDir ? 'Directory' : 'File'}</td></tr>
            <tr><td class="label">Size</td><td class="value">${formatSize(entry.size)}</td></tr>
            <tr><td class="label">Modified</td><td class="value">${entry.modified}</td></tr>
            <tr><td class="label">Created</td><td class="value">${entry.created}</td></tr>
            <tr><td class="label">Permissions</td><td class="value">${entry.permissions}</td></tr>
            <tr><td class="label">MIME</td><td class="value">${entry.mimeType}</td></tr>
        </table>
    `;
}

function escapeHTML(s) {
    return String(s)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
}

// ----- Navigation -----

async function navigate(path) {
    if (!path) return;
    try {
        const entries = await send('list_dir', { path });
        state.currentPath = path;
        state.isSearching = false;
        state.searchResults = [];
        if (state.historyIndex === -1 || state.history[state.historyIndex] !== path) {
            state.history = state.history.slice(0, state.historyIndex + 1);
            state.history.push(path);
            state.historyIndex = state.history.length - 1;
        }
        state.selectedItems.clear();
        updatePathInput();
        renderList(entries);
        updateNavButtons();
        updateStatusBar();
        updateSidebar();
        updatePreview();
    } catch (e) {
        showToast(typeof e === 'string' ? e : String(e), true);
    }
}

function updatePathInput() {
    document.getElementById('path-input').value = state.currentPath;
}

function updateNavButtons() {
    const back = document.getElementById('btn-back');
    const fwd = document.getElementById('btn-forward');
    const up = document.getElementById('btn-up');
    back.disabled = state.historyIndex <= 0;
    fwd.disabled = state.historyIndex >= state.history.length - 1;
    up.disabled = !state.currentPath || state.currentPath === '/';
}

function updateStatusBar() {
    const countEl = document.getElementById('status-count');
    const selEl = document.getElementById('status-selection');
    if (state.isSearching) {
        countEl.textContent = `${state.searchResults.length} results`;
    } else {
        countEl.textContent = document.querySelectorAll('.file-row').length + ' items';
    }
    if (state.selectedItems.size) {
        selEl.textContent = state.selectedItems.size + ' selected';
    } else {
        selEl.textContent = '';
    }
}

// ----- Rendering -----

function renderList(entries) {
    if (state.hiddenShown) {
        entries = entries.filter(e => !e.hidden);
    }

    const list = document.getElementById('file-list');
    list.innerHTML = '';

    // Go up row
    if (state.currentPath && state.currentPath !== '/') {
        const parent = state.currentPath.split('/').slice(0, -1).join('/') || '/';
        const upRow = document.createElement('div');
        upRow.className = 'file-row';
        upRow.dataset.up = 'true';
        upRow.dataset.path = parent;
        upRow.innerHTML = `
            <div class="file-name"><span class="icon">⬆️</span><span data-name>..</span></div>
            <span class="file-size"></span>
            <span class="file-modified"></span>
            <span class="file-perms"></span>
        `;
        upRow.addEventListener('click', () => {
            navigate(parent);
        });
        list.appendChild(upRow);
    }

    for (const entry of entries) {
        const row = createFileRow(entry);
        list.appendChild(row);
    }

    addDragBehavior();
}

function createFileRow(entry) {
    const row = document.createElement('div');
    row.className = 'file-row' + (entry.isDir ? ' dir-row' : '');
    row.dataset.path = entry.path;
    row.dataset.isDir = entry.isDir;
    row.draggable = true;
    row.innerHTML = `
        <div class="file-name"><span class="icon">${getIcon(entry)}</span><span data-name title="${escapeHTML(entry.name)}">${escapeHTML(entry.name)}</span></div>
        <span class="file-size">${entry.isDir ? '' : formatSize(entry.size)}</span>
        <span class="file-modified">${formatDate(entry.modified)}</span>
        <span class="file-perms">${entry.permissions}</span>
    `;

    row.addEventListener('click', (e) => {
        if (e.ctrlKey || e.metaKey) {
            toggleSelect(row);
        } else {
            selectOnly(row);
        }
    });

    row.addEventListener('dblclick', (e) => {
        if (entry.isDir) {
            navigate(entry.path);
        } else {
            send('cli_open', { path: entry.path }).catch(err => showToast(err, true));
        }
    });

    row.addEventListener('contextmenu', (e) => {
        e.preventDefault();
        if (!row.classList.contains('selected')) {
            selectOnly(row);
        }
        showContextMenu(e.clientX, e.clientY, { path: entry.path, isDir: entry.isDir });
    });

    return row;
}

function selectOnly(row) {
    document.querySelectorAll('.file-row.selected').forEach(r => r.classList.remove('selected'));
    row.classList.add('selected');
    state.selectedItems = new Set([row.dataset.path]);
    updateStatusBar();
    updatePreview();
}

function toggleSelect(row) {
    row.classList.toggle('selected');
    if (row.classList.contains('selected')) {
        state.selectedItems.add(row.dataset.path);
    } else {
        state.selectedItems.delete(row.dataset.path);
    }
    updateStatusBar();
    updatePreview();
}

function addDragBehavior() {
    const rows = document.querySelectorAll('.file-row[data-path]');
    rows.forEach(row => {
        row.addEventListener('dragstart', (e) => {
            let paths;
            if (state.selectedItems.size) paths = [...state.selectedItems];
            else if (row.dataset.path) paths = [row.dataset.path];
            if (!paths.length) {
                e.preventDefault();
                return;
            }
            state.draggingPaths = paths;
            e.dataTransfer.setData('application/json', JSON.stringify(paths));
            e.dataTransfer.effectAllowed = 'copyMove';
            row.classList.add('dragging');
        });

        row.addEventListener('dragend', () => {
            row.classList.remove('dragging');
            document.querySelectorAll('.drop-target').forEach(el => el.classList.remove('drop-target'));
        });

        // Folders and the ".." row are drop targets
        if (row.classList.contains('dir-row') || row.dataset.up) {
            row.addEventListener('dragover', (e) => {
                if (state.draggingPaths) {
                    e.preventDefault();
                    e.dataTransfer.dropEffect = e.ctrlKey || e.shiftKey ? 'copy' : 'move';
                    row.classList.add('drop-target');
                }
            });
            row.addEventListener('dragleave', () => row.classList.remove('drop-target'));
            row.addEventListener('drop', (e) => {
                e.preventDefault();
                row.classList.remove('drop-target');
                moveDropped(e, row.dataset.path);
            });
        }
    });
}

async function moveDropped(e, targetDir) {
    let paths = [];
    try {
        const raw = e.dataTransfer.getData('application/json');
        if (raw) paths = JSON.parse(raw);
    } catch {
        paths = [];
    }
    if (!paths.length && state.draggingPaths) paths = state.draggingPaths;
    state.draggingPaths = null;
    document.querySelectorAll('.drop-target').forEach(el => el.classList.remove('drop-target'));

    if (!paths.length || !targetDir) return;
    const copy = e.ctrlKey || e.shiftKey;
    let done = 0;
    for (const p of paths) {
        if (p === targetDir) continue;
        try {
            if (copy) await send('copy_item', { src: p, dstDir: targetDir });
            else await send('move_item', { src: p, dstDir: targetDir });
            done++;
        } catch (err) {
            showToast(String(err), true);
        }
    }
    if (done) {
        showToast(`${done} item(s) ${copy ? 'copied' : 'moved'}`);
        navigate(state.currentPath);
    }
}

// ----- Asset actions -----

async function doCopy() {
    const target = state.clipboard;
    if (!target) {
        showToast('Nothing to paste', true);
        return;
    }
    const dstDir = state.currentPath;
    let failed = false;
    for (const item of target.items) {
        try {
            if (target.action === 'copy') {
                await send('copy_item', { src: item, dstDir });
            } else {
                await send('move_item', { src: item, dstDir });
            }
        } catch (e) {
            failed = true;
            showToast(String(e), true);
        }
    }
    if (target.action === 'cut') {
        state.clipboard = null;
        document.getElementById('status-canpaste').textContent = '';
    }
    navigate(state.currentPath);
    if (!failed) {
        showToast(target.action === 'copy' ? 'Pasted' : 'Moved');
    }
}

// ----- Context menu -----

function showContextMenu(x, y, target) {
    const menu = document.getElementById('context-menu');
    state.contextTarget = target || null;

    const needsItem = ['open', 'preview', 'rename', 'copy', 'cut', 'delete', 'delete-permanent', 'compress', 'properties'];
    menu.querySelectorAll('.menu-item').forEach(item => {
        const a = item.dataset.action;
        let display = 'block';
        if (needsItem.includes(a) && (!state.contextTarget || !state.contextTarget.path)) {
            display = 'none';
        } else if (a === 'paste') {
            display = state.clipboard ? 'block' : 'none';
        }
        item.style.display = display;
    });

    menu.style.left = Math.max(4, Math.min(x, window.innerWidth - 260)) + 'px';
    menu.style.top = Math.max(4, Math.min(y, window.innerHeight - 360)) + 'px';
    menu.classList.remove('hidden');
}

document.getElementById('context-menu').addEventListener('click', (e) => {
    const item = e.target.closest('.menu-item');
    if (!item) return;
    const action = item.dataset.action;
    handleAction(action);
    hideContextMenu();
});

function hideContextMenu() {
    document.getElementById('context-menu').classList.add('hidden');
}

document.addEventListener('click', (e) => {
    if (!e.target.closest('#context-menu')) {
        hideContextMenu();
    }
});

// Suppress the WebKit default context menu (e.g. "Inspect Element") everywhere
// and show our own menu on empty space.
document.addEventListener('contextmenu', (e) => {
    if (e.target.closest('#context-menu') || e.target.closest('.file-row')) return;
    e.preventDefault();
    showContextMenu(e.clientX, e.clientY, null);
});

// ----- Action handling -----

async function handleAction(action) {
    const target = state.contextTarget || { path: null, isDir: false };

    switch (action) {
        case 'open':
            if (target.path) {
                if (target.isDir) navigate(target.path);
                else send('cli_open', { path: target.path }).catch(e => showToast(e, true));
            }
            break;
        case 'rename':
            showRenameModal(target.path);
            break;
        case 'copy':
            if (target.path) {
                state.clipboard = { items: [target.path], action: 'copy' };
                document.getElementById('status-canpaste').textContent = '📋 1 item to copy';
                showToast('Copied to clipboard');
            }
            break;
        case 'cut':
            if (target.path) {
                state.clipboard = { items: [target.path], action: 'cut' };
                document.getElementById('status-canpaste').textContent = '✂️ 1 item to move';
                showToast('Cut to clipboard');
            }
            break;
        case 'paste':
            await doCopy();
            break;
        case 'delete':
            if (target.path) {
                if (await safeConfirm(`Move "${target.path.split('/').pop()}" to trash?`)) {
                    try {
                        await send('delete_path', { path: target.path, permanent: false });
                        navigate(state.currentPath);
                    } catch (e) {
                        showToast(String(e), true);
                    }
                }
            }
            break;
        case 'delete-permanent':
            if (target.path) {
                if (await safeConfirm(`Permanently delete "${target.path.split('/').pop()}"? This cannot be undone!`)) {
                    try {
                        await send('delete_path', { path: target.path, permanent: true });
                        navigate(state.currentPath);
                    } catch (e) {
                        showToast(String(e), true);
                    }
                }
            }
            break;
        case 'new-file':
            showCreateModal('file');
            break;
        case 'new-folder':
            showCreateModal('dir');
            break;
        case 'search':
            startSearch();
            break;
        case 'preview':
            togglePreview(true);
            break;
        case 'compress':
            compressZIP();
            break;
        case 'terminal':
            openTerminal();
            break;
        case 'set-background':
            setBackground();
            break;
        case 'reset-background':
            resetBackground();
            break;
        case 'properties':
            if (target.path) {
                try {
                    const info = await send('file_info', { path: target.path });
                    showModal('Properties', renderProps(info));
                } catch (e) {
                    showToast(String(e), true);
                }
            }
            break;
    }
}

// ----- Modals -----

function showModal(title, content) {
    const overlay = document.getElementById('modal-overlay');
    document.getElementById('modal-content').innerHTML = `
        <h3>${title}</h3>
        ${content}
    `;
    overlay.classList.remove('hidden');

    document.getElementById('modal-confirm').onclick = () => overlay.classList.add('hidden');
    document.getElementById('modal-cancel').onclick = () => overlay.classList.add('hidden');
}

function closeModal() {
    document.getElementById('modal-overlay').classList.add('hidden');
}

// Non-native confirm dialog (native confirm() crashes some WebKitGTK builds)
function safeConfirm(message) {
    return new Promise(resolve => {
        const overlay = document.getElementById('modal-overlay');
        document.getElementById('modal-content').innerHTML = `<h3>Confirm</h3><p>${message}</p>`;
        const ok = document.getElementById('modal-confirm');
        const cancel = document.getElementById('modal-cancel');
        ok.onclick = () => { overlay.classList.add('hidden'); resolve(true); };
        cancel.onclick = () => { overlay.classList.add('hidden'); resolve(false); };
        overlay.classList.remove('hidden');
    });
}

function showRenameModal(path) {
    const name = path.split('/').pop();
    showModal('Rename', `
        <div class="field">
            <label>New name</label>
            <input type="text" id="rename-input" value="${escapeHTML(name)}" autofocus>
        </div>
    `);
    document.getElementById('rename-input').select();
    document.getElementById('modal-confirm').onclick = async () => {
        const newName = document.getElementById('rename-input').value.trim();
        if (!newName) return;
        try {
            await send('rename_path', { oldPath: path, newName });
            closeModal();
            navigate(state.currentPath);
        } catch (e) {
            showToast(String(e), true);
        }
    };
}

function showCreateModal(type) {
    showModal(type === 'dir' ? 'New Folder' : 'New File', `
        <div class="field">
            <label>Name</label>
            <input type="text" id="create-input" placeholder="${type === 'dir' ? 'folder name' : 'file name'}" autofocus>
        </div>
    `);
    document.getElementById('create-input').focus();
    document.getElementById('modal-confirm').onclick = async () => {
        const name = document.getElementById('create-input').value.trim();
        if (!name) return;
        try {
            if (type === 'dir') {
                await send('create_dir', { parent: state.currentPath, name });
            } else {
                await send('create_file', { parent: state.currentPath, name });
            }
            closeModal();
            navigate(state.currentPath);
        } catch (e) {
            showToast(String(e), true);
        }
    };
}

// ----- Search -----

async function startSearch(initialQuery) {
    const box = document.getElementById('search-input');
    const query = (initialQuery !== undefined ? initialQuery : box.value).trim();
    if (!query) {
        box.focus();
        return;
    }

    const target = state.contextTarget && state.contextTarget.isDir
        ? state.contextTarget.path
        : state.currentPath;

    document.getElementById('files-view').classList.add('loading');
    try {
        const results = await send('search_files', {
            baseDir: target,
            query,
            caseSensitive: false,
            maxDepth: 10,
        });
        state.isSearching = true;
        state.searchResults = results;
        const list = document.getElementById('file-list');
        list.innerHTML = '';
        for (const r of results.slice(0, 500)) {
            const row = document.createElement('div');
            row.className = 'file-row';
            row.dataset.path = r.path;
            row.innerHTML = `
                <div class="file-name"><span class="icon">${r.isDir ? '📁' : '📄'}</span><span data-name title="${escapeHTML(r.path)}">${escapeHTML(r.name)}</span></div>
                <span class="file-size">${r.isDir ? '' : formatSize(r.size)}</span>
                <span class="file-modified"></span>
                <span class="file-perms"></span>
            `;
            row.addEventListener('dblclick', () => {
                if (r.isDir) {
                    state.isSearching = false;
                    navigate(r.path);
                }
            });
            row.addEventListener('click', () => selectOnly(row));
            row.addEventListener('contextmenu', (e) => {
                e.preventDefault();
                selectOnly(row);
                showContextMenu(e.clientX, e.clientY, { path: r.path, isDir: r.isDir });
            });
            list.appendChild(row);
        }
        updateStatusBar();
    } catch (e) {
        showToast(String(e), true);
    } finally {
        document.getElementById('files-view').classList.remove('loading');
    }
}

// ----- Disk usage -----

async function showDiskUsage() {
    try {
        const disks = await send('disk_usage');
        const rows = disks.map(d => `
            <tr>
                <td class="label">Device</td>
                <td class="value">${escapeHTML(d.device)}</td>
            </tr>
            <tr>
                <td class="label">Mount</td>
                <td class="value">${escapeHTML(d.mountPoint)}</td>
            </tr>
            <tr>
                <td class="label">Total</td>
                <td class="value">${formatSize(d.total)}</td>
            </tr>
            <tr>
                <td class="label">Used</td>
                <td class="value">${formatSize(d.used)} (${d.usedPercent}%)</td>
            </tr>
            <tr>
                <td class="label">Free</td>
                <td class="value">${formatSize(d.free)}</td>
            </tr>
            <tr><td colspan="2"><hr style="border-color: var(--border);"></td></tr>
        `).join('');
        showModal('Disk Usage', `<table class="properties-grid">${rows}</table>`);
    } catch (e) {
        showToast(String(e), true);
    }
}

// ----- Duplicates -----

async function showDuplicates() {
    try {
        const groups = await send('duplicates_search', { baseDir: state.currentPath });
        if (!groups.length) {
            showToast('No duplicates found');
            return;
        }
        const html = groups.slice(0, 50).map(g => `
            <div class="dup-group">
                <div class="dup-size">${formatSize(g.size)} × ${g.files.length} files</div>
                ${g.files.map(f => `<div class="dup-file">${escapeHTML(f)}</div>`).join('')}
            </div>
        `).join('');
        showModal('Duplicate Files', html);
    } catch (e) {
        showToast(String(e), true);
    }
}

// ----- Sidebar -----

// ----- Sidebar -----

function updateSidebar() {
    document.querySelectorAll('.sidebar-item[data-folder]').forEach(item => {
        const folder = item.dataset.folder;
        const home = state.currentPath.split('/').includes(folder) ? true : false;
        item.classList.toggle('active', home);
    });
}

async function loadSidebar() {
    const home = await send('home_dir');
    const places = document.getElementById('recent-places');

    places.innerHTML = '';
    const recentItem = document.createElement('div');
    recentItem.className = 'sidebar-item';
    recentItem.innerHTML = `<span class="icon">🕒</span> Recent Files`;
    recentItem.addEventListener('click', showRecentFiles);
    places.appendChild(recentItem);

    const homeItem = document.createElement('div');
    homeItem.className = 'sidebar-item';
    homeItem.innerHTML = `<span class="icon">🏠</span> Home`;
    homeItem.dataset.dropPath = home;
    homeItem.addEventListener('click', () => navigate(home));
    places.appendChild(homeItem);

    const desktop = document.createElement('div');
    desktop.className = 'sidebar-item';
    desktop.innerHTML = `<span class="icon">🖥️</span> Desktop`;
    desktop.dataset.dropPath = home + '/Desktop';
    desktop.addEventListener('click', () => navigate(home + '/Desktop'));
    places.appendChild(desktop);

    for (const folder of ['Downloads', 'Documents', 'Music', 'Pictures', 'Videos']) {
        const item = document.createElement('div');
        item.className = 'sidebar-item sidebar-shortcut';
        item.dataset.folder = folder.toLowerCase();
        item.dataset.dropPath = home + '/' + folder;
        item.innerHTML = `<span class="icon">${getFolderIcon(folder)}</span> ${folder}`;
        item.addEventListener('click', () => navigate(home + '/' + folder));
        places.appendChild(item);
    }
}

function getFolderIcon(folder) {
    switch (folder) {
        case 'Downloads': return '📥';
        case 'Documents': return '📄';
        case 'Music': return '🎵';
        case 'Pictures': return '🖼️';
        case 'Videos': return '🎬';
        default: return '📁';
    }
}

async function loadDevices() {
    try {
        const disks = await send('disk_usage');
        const devices = document.getElementById('devices');
        devices.innerHTML = '';
        for (const disk of disks) {
            const item = document.createElement('div');
            item.className = 'sidebar-item';
            const mount = disk.mountPoint;
            const label = mount === '/' ? 'Root /' : mount;
            item.innerHTML = `💾 <span title="${disk.device}">${escapeHTML(label)}</span> <span style="font-size: 11px; color: var(--text-dim); margin-left: auto">${disk.usedPercent}%</span>`;
            item.dataset.dropPath = mount;
            item.addEventListener('click', () => navigate(mount));
            devices.appendChild(item);
        }
    } catch (e) {
        // Feature not available
    }
}

// Wire up the static Shortcuts section in the sidebar
document.querySelectorAll('.sidebar-section ul .sidebar-item[data-folder]').forEach(item => {
    item.addEventListener('click', async () => {
        const home = await send('home_dir');
        const f = item.dataset.folder;
        navigate(home + '/' + f.charAt(0).toUpperCase() + f.slice(1));
    });
});

// Sidebar collapse toggle
document.getElementById('btn-sidebar').addEventListener('click', () => {
    document.body.classList.toggle('sidebar-collapsed');
});

// ----- Toolbar wiring -----

document.getElementById('btn-back').addEventListener('click', () => {
    if (state.historyIndex > 0) {
        state.historyIndex--;
        navigate(state.history[state.historyIndex]);
    }
});

document.getElementById('btn-forward').addEventListener('click', () => {
    if (state.historyIndex < state.history.length - 1) {
        state.historyIndex++;
        navigate(state.history[state.historyIndex]);
    }
});

document.getElementById('btn-up').addEventListener('click', () => {
    if (state.currentPath && state.currentPath !== '/') {
        const parent = state.currentPath.split('/').slice(0, -1).join('/') || '/';
        navigate(parent);
    }
});

document.getElementById('btn-home').addEventListener('click', async () => {
    const home = await send('home_dir');
    navigate(home);
});

document.getElementById('btn-new-file').addEventListener('click', () => showCreateModal('file'));
document.getElementById('btn-new-folder').addEventListener('click', () => showCreateModal('dir'));

document.getElementById('btn-rename').addEventListener('click', () => {
    const items = [...state.selectedItems];
    if (items.length === 1) {
        showRenameModal(items[0]);
    } else if (items.length > 1) {
        showBatchRenameModal();
    } else {
        showToast('Select one item to rename', true);
    }
});

function showBatchRenameModal() {
    showModal('Batch Rename', `
        <div class="field">
            <label>Pattern (counter replaces {n})</label>
            <input type="text" id="batch-pattern" value="photo_{n}" autofocus>
        </div>
        <div class="field">
            <label>Start number</label>
            <input type="number" id="batch-start" value="1" min="0">
        </div>
        <div class="field">
            <label>Padding (zeros)</label>
            <input type="number" id="batch-padding" value="3" min="1" max="10">
        </div>
        <p style="font-size: 12px; color: var(--text-dim);">Renames all files in current folder matching pattern.</p>
    `);
    document.getElementById('modal-confirm').onclick = async () => {
        const pattern = document.getElementById('batch-pattern').value.trim();
        const start = parseInt(document.getElementById('batch-start').value, 10);
        const padding = parseInt(document.getElementById('batch-padding').value, 10);
        if (!pattern || isNaN(start)) return;
        try {
            const renamed = await send('batch_rename', { dir: state.currentPath, pattern, start, padding });
            closeModal();
            navigate(state.currentPath);
            showToast(`Renamed ${renamed.length} files`);
        } catch (e) {
            showToast(String(e), true);
        }
    };
}

document.getElementById('btn-copy').addEventListener('click', () => {
    if (state.selectedItems.size) {
        state.clipboard = { items: [...state.selectedItems], action: 'copy' };
        document.getElementById('status-canpaste').textContent = `📋 ${state.selectedItems.size} items to copy`;
        showToast('Copied to clipboard');
    }
});

document.getElementById('btn-paste').addEventListener('click', () => doCopy());

document.getElementById('btn-delete').addEventListener('click', async () => {
    if (!state.selectedItems.size) return;
    const confirmMsg = `Move ${state.selectedItems.size} item(s) to trash?`;
    if (await safeConfirm(confirmMsg)) {
        try {
            for (const item of state.selectedItems) {
                await send('delete_path', { path: item, permanent: false });
            }
            navigate(state.currentPath);
        } catch (e) {
            showToast(String(e), true);
        }
    }
});

document.getElementById('btn-disk').addEventListener('click', showDiskUsage);
document.getElementById('btn-dupes').addEventListener('click', showDuplicates);

document.getElementById('btn-properties').addEventListener('click', () => {
    if (state.selectedItems.size === 1) {
        const path = [...state.selectedItems][0];
        send('file_info', { path }).then(info => showModal('Properties', renderProps(info))).catch(e => showToast(e, true));
    } else {
        showToast('Select one item to view properties', true);
    }
});

document.getElementById('hide-hidden-check').addEventListener('change', (e) => {
    state.hiddenShown = e.target.checked;
    send('hidden_files_setting', { value: e.target.checked }).catch(() => {});
    if (state.currentPath) navigate(state.currentPath);
});

async function openTerminal() {
    try {
        await send('open_terminal', { path: state.currentPath || '~' });
    } catch {
        showToast('No terminal found', true);
    }
}

document.getElementById('btn-terminal').addEventListener('click', openTerminal);

// ----- Background image -----

function applyBackground(dataUrl) {
    const list = document.getElementById('file-list');
    if (dataUrl) {
        list.classList.add('has-bg');
        list.style.backgroundImage = `url("${dataUrl}")`;
    } else {
        list.classList.remove('has-bg');
        list.style.backgroundImage = '';
    }
}

async function setBackground() {
    let path;
    try {
        path = await send('pick_image');
    } catch (e) {
        showToast(String(e), true);
        return;
    }
    if (!path) return;
    try {
        const dataUrl = await send('load_image_data', { path });
        await send('background_setting', { path });
        applyBackground(dataUrl);
        showToast('Background image set');
    } catch (e) {
        showToast(String(e), true);
    }
}

async function resetBackground() {
    try {
        await send('background_setting', { path: null });
        applyBackground(null);
        showToast('Background image removed');
    } catch (e) {
        showToast(String(e), true);
    }
}

document.getElementById('btn-background').addEventListener('click', setBackground);

// ----- Preview pane -----

function previewVisible() {
    return !document.getElementById('preview-pane').classList.contains('hidden');
}

function togglePreview(force) {
    const pane = document.getElementById('preview-pane');
    const show = force === true ? true : force === false ? false : previewVisible();
    pane.classList.toggle('hidden', !show);
    document.getElementById('btn-preview').classList.toggle('previewing', show);
    if (show) updatePreview();
}

async function updatePreview() {
    if (!previewVisible()) return;
    const pane = document.getElementById('preview-pane');
    if (state.selectedItems.size !== 1) {
        pane.innerHTML = '<div class="preview-empty">Select one item to preview</div>';
        return;
    }
    const path = [...state.selectedItems][0];
    pane.innerHTML = '<div class="preview-empty">Loading…</div>';
    try {
        const info = await send('preview_file', { path });
        renderPreview(info, path);
    } catch (e) {
        pane.innerHTML = `<div class="preview-empty">${escapeHTML(String(e))}</div>`;
    }
}

function renderPreview(info, path) {
    const pane = document.getElementById('preview-pane');
    let body = '';
    if (info.kind === 'image' && info.dataUrl) {
        body = `<img class="preview-media" src="${info.dataUrl}" alt="${escapeHTML(info.name)}">`;
    } else if (info.kind === 'audio' && info.dataUrl) {
        body = `<audio class="preview-media" controls src="${info.dataUrl}"></audio>`;
    } else if (info.kind === 'video' && info.dataUrl) {
        body = `<video class="preview-media" controls src="${info.dataUrl}"></video>`;
    } else if (info.kind === 'text' && info.text != null) {
        body = `<pre class="preview-text">${escapeHTML(info.text)}</pre>`;
    } else if (info.kind === 'dir') {
        body = '<div class="preview-empty">📁 Directory — no preview</div>';
    } else {
        body = '<div class="preview-empty">Preview not supported for this type</div>';
    }
    pane.innerHTML = `
        <div class="preview-head">
            <h3>${escapeHTML(info.name)}</h3>
            <div class="preview-meta">${escapeHTML(info.mimeType)} · ${formatSize(info.size)}</div>
        </div>
        ${body}
        <button class="preview-open" data-path="${escapeHTML(path)}">Open in app</button>
    `;
}

document.getElementById('preview-pane').addEventListener('click', (e) => {
    const btn = e.target.closest('.preview-open');
    if (btn) send('cli_open', { path: btn.dataset.path }).catch(err => showToast(err, true));
});

document.getElementById('btn-preview').addEventListener('click', () => togglePreview());

// ----- Compress to ZIP -----

async function compressZIP() {
    let items = [];
    if (state.selectedItems.size) {
        items = [...state.selectedItems];
    } else if (state.contextTarget && state.contextTarget.path) {
        items = [state.contextTarget.path];
    }
    if (!items.length) {
        showToast('Select items first', true);
        return;
    }
    const base = items.length === 1
        ? (items[0].split('/').pop() || 'archive').replace(/\.\w+$/, '')
        : 'archive';
    showModal('Compress to ZIP', `
        <div class="field">
            <label>Archive name</label>
            <input type="text" id="zip-name" value="${escapeHTML(base)}" autofocus>
        </div>
    `);
    document.getElementById('zip-name').select();
    document.getElementById('modal-confirm').onclick = async () => {
        const name = document.getElementById('zip-name').value.trim();
        if (!name) return;
        try {
            await send('compress_zip', { items, destDir: state.currentPath, name });
            closeModal();
            showToast(`Created ${name}.zip`);
            navigate(state.currentPath);
        } catch (e) {
            showToast(String(e), true);
        }
    };
    document.getElementById('modal-cancel').onclick = closeModal;
}

// ----- Recent files -----

async function showRecentFiles() {
    let recs;
    try {
        recs = await send('recent_files');
    } catch (e) {
        showToast(String(e), true);
        return;
    }
    if (!recs.length) {
        showToast('No recent files yet', true);
        return;
    }
    const rows = recs.map(r => `
        <div class="recent-item" data-path="${escapeHTML(r.path)}">
            <span>📄</span>
            <span class="recent-name" title="${escapeHTML(r.path)}">${escapeHTML(r.name)}</span>
            <span class="recent-time">${escapeHTML(r.lastOpened || '')}</span>
        </div>`).join('');
    showModal('Recent Files', `<div class="recent-list">${rows}</div>`);
    document.getElementById('modal-confirm').onclick = closeModal;
    document.getElementById('modal-cancel').onclick = closeModal;
    document.querySelectorAll('.recent-item').forEach(el => {
        el.addEventListener('click', () => {
            const p = el.dataset.path;
            closeModal();
            navigate(p.split('/').slice(0, -1).join('/') || '/');
        });
    });
}

// ----- Sidebar as drop target -----

const sidebarDrop = document.getElementById('sidebar');
sidebarDrop.addEventListener('dragover', (e) => {
    const item = e.target.closest('.sidebar-item');
    if (item && item.dataset.dropPath) {
        e.preventDefault();
        item.classList.add('drop-target');
        e.dataTransfer.dropEffect = e.ctrlKey || e.shiftKey ? 'copy' : 'move';
    }
});
sidebarDrop.addEventListener('dragleave', (e) => {
    const item = e.target.closest('.sidebar-item');
    if (item) item.classList.remove('drop-target');
});
sidebarDrop.addEventListener('drop', (e) => {
    const item = e.target.closest('.sidebar-item');
    if (!item || !item.dataset.dropPath) return;
    e.preventDefault();
    item.classList.remove('drop-target');
    moveDropped(e, item.dataset.dropPath);
});

document.getElementById('path-input').addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
        navigate(e.target.value.trim());
    }
});

document.getElementById('search-input').addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
        startSearch();
    }
});

// Keyboard shortcuts
document.addEventListener('keydown', (e) => {
    if (e.target.tagName === 'INPUT') return;

    switch (e.key) {
        case 'Backspace':
            if (state.currentPath !== '/') {
                const parent = state.currentPath.split('/').slice(0, -1).join('/') || '/';
                navigate(parent);
            }
            break;
        case 'Delete':
            document.getElementById('btn-delete').click();
            break;
        case 'F2':
            document.getElementById('btn-rename').click();
            break;
        case 'r':
        case 'R':
            if (e.ctrlKey) {
                e.preventDefault();
                navigate(state.currentPath);
            }
            break;
        case 'c':
        case 'C':
            if (e.ctrlKey) {
                e.preventDefault();
                document.getElementById('btn-copy').click();
            }
            break;
        case 'v':
        case 'V':
            if (e.ctrlKey) {
                e.preventDefault();
                document.getElementById('btn-paste').click();
            }
            break;
        case 'h':
        case 'H':
            if (e.ctrlKey) {
                e.preventDefault();
                const check = document.getElementById('hide-hidden-check');
                check.checked = !check.checked;
                check.dispatchEvent(new Event('change'));
            }
            break;
        case 't':
        case 'T':
            if (e.ctrlKey && e.altKey) {
                e.preventDefault();
                document.getElementById('btn-terminal').click();
            } else if (e.ctrlKey) {
                e.preventDefault();
                newTab();
            }
            break;
        case 'w':
        case 'W':
            if (e.ctrlKey && !e.altKey) {
                e.preventDefault();
                closeTab(activeTabId);
            }
            break;
        case 'n':
        case 'N':
            if (e.ctrlKey) {
                e.preventDefault();
                showCreateModal('file');
            }
            break;
        case 'f':
        case 'F':
            if (e.ctrlKey) {
                e.preventDefault();
                const box = document.getElementById('search-input');
                box.focus();
                box.select();
            }
            break;
    }
});

// ----- Init -----

(async function init() {
    try {
        // Load hidden setting
        const hiddenSetting = await send('get_hidden_setting');
        document.getElementById('hide-hidden-check').checked = hiddenSetting;
        state.hiddenShown = hiddenSetting;

        // Load background image
        const bgPath = await send('get_background_setting');
        if (bgPath) {
            try {
                const dataUrl = await send('load_image_data', { path: bgPath });
                applyBackground(dataUrl);
            } catch (e) {
                console.warn('Failed to load background:', e);
            }
        }

        // Load sidebar
        await loadSidebar();
        await loadDevices();

        // Navigate to home
        const home = await send('home_dir');
        newTab(home);
    } catch (e) {
        console.error('Init failed:', e);
        showToast('Failed to initialize: ' + e, true);
    }
})();