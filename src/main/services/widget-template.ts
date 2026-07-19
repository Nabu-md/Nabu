/**
 * widget-template.ts
 *
 * Returns the HTML string for the clipboard-widget BrowserWindow.
 * The widget is a small floating pill that expands to show clipboard history,
 * clipnote, web clip, or search. Inspired by Wispr Flow's UX.
 *
 * States:
 *   pill      → small sparkle icon (top-left, draggable)
 *   modes     → four mode squares (Clipboard / Web Clip / Search / ClipNote)
 *   clipboard → scrollable clipboard list with arrow-key navigation
 *   clipnote  → text input to create a quick note
 *   web-clip  → URL input to save a web page reference
 *   search    → vault search with result list
 */

export function getWidgetHTML(): string {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>
  /* ==================================================================
     Reset & Base
     ================================================================== */
  *, *::before, *::after { margin:0; padding:0; box-sizing:border-box; }

  :root {
    --bg: rgba(22, 22, 26, 0.94);
    --bg-hover: rgba(39, 39, 45, 0.96);
    --border: rgba(255,255,255,0.06);
    --text: rgba(255,255,255,0.88);
    --text-dim: rgba(255,255,255,0.40);
    --accent: rgb(94, 148, 255);
    --accent-alpha: rgba(94, 148, 255, 0.15);
    --radius: 10px;
    --pill-size: 36px;
  }

  body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    font-size: 13px;
    color: var(--text);
    background: transparent;
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
    height: 100vh;
    width: 100vw;
  }

  #root {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: transparent;
    border-radius: var(--radius);
    overflow: hidden;
  }

  .panel {
    background: var(--bg);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: 0 8px 32px rgba(0,0,0,0.5);
    width: 100%;
  }

  /* ==================================================================
     Pill
     ================================================================== */
  #pill {
    width: var(--pill-size);
    height: var(--pill-size);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: background 0.15s;
    border-radius: var(--radius);
    background: var(--bg);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border: 1px solid var(--border);
    box-shadow: 0 4px 12px rgba(0,0,0,0.4);
  }
  #pill:hover { background: var(--bg-hover); }
  #pill svg { width: 18px; height: 18px; }

  /* ==================================================================
     Mode selector
     ================================================================== */
  #mode-selector { display: flex; gap: 6px; padding: 8px; }
  .mode-btn {
    display: flex; flex-direction: column; align-items: center; gap: 4px;
    width: 64px; padding: 8px 4px 6px; border-radius: 6px;
    cursor: pointer; border: none; background: transparent;
    color: var(--text-dim); font-size: 10px;
    transition: background 0.12s, color 0.12s;
  }
  .mode-btn:hover { background: var(--accent-alpha); color: var(--text); }
  .mode-btn.active { background: var(--accent-alpha); color: var(--accent); }
  .mode-btn svg { width: 20px; height: 20px; }

  /* ==================================================================
     Clipboard list
     ================================================================== */
  #clipboard-view { width: 100%; }
  #clipboard-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 8px 12px 6px; font-size: 11px; font-weight: 600;
    color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.04em;
    border-bottom: 1px solid var(--border);
  }
  #clipboard-clear {
    background: none; border: none; color: var(--text-dim);
    font-size: 10px; cursor: pointer; padding: 2px 6px; border-radius: 4px;
  }
  #clipboard-clear:hover { background: var(--accent-alpha); color: var(--accent); }
  #clipboard-list {
    max-height: 340px; overflow-y: auto; padding: 4px 6px;
  }
  #clipboard-list::-webkit-scrollbar { width: 4px; }
  #clipboard-list::-webkit-scrollbar-track { background: transparent; }
  #clipboard-list::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.1); border-radius: 2px; }

  .clip-item {
    display: flex; align-items: center; gap: 8px;
    padding: 7px 8px; border-radius: 6px; cursor: pointer;
    transition: background 0.1s; border: 1px solid transparent;
  }
  .clip-item:hover { background: rgba(255,255,255,0.04); }
  .clip-item.highlighted {
    background: var(--accent-alpha);
    border-color: rgba(94, 148, 255, 0.25);
  }
  .clip-text {
    flex: 1; font-size: 12px; line-height: 1.4; color: var(--text);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 240px;
  }
  .clip-time {
    font-size: 10px; color: var(--text-dim); white-space: nowrap; flex-shrink: 0;
  }
  .clip-empty {
    padding: 20px; text-align: center; color: var(--text-dim); font-size: 12px;
  }

  /* ==================================================================
     Shared mode form styles (ClipNote, Web Clip, Search)
     ================================================================== */
  .mode-panel { width: 100%; }
  .mode-panel-header {
    display: flex; align-items: center; gap: 8px;
    padding: 8px 12px 6px; font-size: 11px; font-weight: 600;
    color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.04em;
    border-bottom: 1px solid var(--border);
  }
  .mode-panel-body { padding: 10px 12px; }

  .mode-input {
    width: 100%; padding: 7px 9px; border-radius: 6px;
    border: 1px solid var(--border); background: rgba(255,255,255,0.05);
    color: var(--text); font-size: 12px; font-family: inherit;
    outline: none; transition: border-color 0.15s;
  }
  .mode-input:focus { border-color: var(--accent); }
  .mode-input::placeholder { color: var(--text-dim); }

  .mode-textarea {
    width: 100%; min-height: 72px; resize: none;
    padding: 7px 9px; border-radius: 6px;
    border: 1px solid var(--border); background: rgba(255,255,255,0.05);
    color: var(--text); font-size: 12px; font-family: inherit;
    outline: none; transition: border-color 0.15s;
  }
  .mode-textarea:focus { border-color: var(--accent); }
  .mode-textarea::placeholder { color: var(--text-dim); }

  .mode-actions {
    display: flex; gap: 6px; margin-top: 8px; justify-content: flex-end;
  }
  .mode-btn-primary {
    padding: 5px 14px; border-radius: 6px; border: none;
    background: var(--accent); color: #fff; font-size: 11px; font-weight: 600;
    cursor: pointer; transition: opacity 0.15s;
  }
  .mode-btn-primary:hover { opacity: 0.85; }
  .mode-btn-primary:disabled { opacity: 0.4; cursor: default; }

  .mode-btn-secondary {
    padding: 5px 14px; border-radius: 6px; border: 1px solid var(--border);
    background: transparent; color: var(--text-dim); font-size: 11px;
    cursor: pointer; transition: background 0.15s;
  }
  .mode-btn-secondary:hover { background: rgba(255,255,255,0.05); }

  .mode-status {
    font-size: 11px; color: var(--text-dim); padding: 6px 0 0;
    text-align: center;
  }
  .mode-status.success { color: #4ade80; }
  .mode-status.error { color: #f87171; }

  /* ==================================================================
     Search results
     ================================================================== */
  #search-results {
    max-height: 260px; overflow-y: auto; margin-top: 6px;
  }
  #search-results::-webkit-scrollbar { width: 4px; }
  #search-results::-webkit-scrollbar-track { background: transparent; }
  #search-results::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.1); border-radius: 2px; }

  .search-result-item {
    padding: 6px 8px; border-radius: 5px; cursor: pointer;
    transition: background 0.1s; font-size: 12px;
  }
  .search-result-item:hover { background: var(--accent-alpha); }
  .search-result-name { color: var(--text); }
  .search-result-path { color: var(--text-dim); font-size: 10px; }

  /* ==================================================================
     Utility
     ================================================================== */
  .hidden { display: none !important; }
</style>
</head>
<body>
<div id="root">
  <!-- ========== Pill ========== -->
  <div id="pill" class="panel hidden">
    <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
      <path d="M13 2l3-1 1 3-2 2-4 4-2 1"/>
      <path d="M11 9l-4 4"/>
      <path d="M4 13l-2 4 4-2"/>
    </svg>
  </div>

  <!-- ========== Mode selector ========== -->
  <div id="mode-selector" class="panel hidden">
    <button class="mode-btn" data-mode="clipboard" title="Clipboard History">
      <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <rect x="3" y="3" width="14" height="14" rx="2"/><path d="M7 7h6"/><path d="M7 10h6"/><path d="M7 13h4"/>
      </svg>
      Clipboard
    </button>
    <button class="mode-btn" data-mode="web-clip" title="Save a URL to your vault">
      <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M10 5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-4a1 1 0 0 1-1-1"/>
        <path d="M8 7l-3 3 3 3"/><path d="M5 10h8"/>
      </svg>
      Web Clip
    </button>
    <button class="mode-btn" data-mode="search" title="Search your vault">
      <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="8.5" cy="8.5" r="5"/><path d="M12.5 12.5l4 4"/>
      </svg>
      Search
    </button>
    <button class="mode-btn" data-mode="clipnote" title="Quickly capture a thought">
      <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 3h4a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h4"/>
        <path d="M13 1v4"/><path d="M7 1v4"/>
        <path d="M7 10l2 2 4-4"/>
      </svg>
      ClipNote
    </button>
  </div>

  <!-- ========== Clipboard view ========== -->
  <div id="clipboard-view" class="panel hidden">
    <div id="clipboard-header">
      <span>Clipboard History</span>
      <button id="clipboard-clear">Clear</button>
    </div>
    <div id="clipboard-list"></div>
  </div>

  <!-- ========== Web Clip view ========== -->
  <div id="web-clip-view" class="panel hidden">
    <div class="mode-panel">
      <div class="mode-panel-header">
        <svg width="14" height="14" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M10 5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-4a1 1 0 0 1-1-1"/><path d="M8 7l-3 3 3 3"/><path d="M5 10h8"/></svg>
        Web Clip
      </div>
      <div class="mode-panel-body">
        <input id="webclip-url" class="mode-input" type="text" placeholder="Paste a URL…" autofocus spellcheck="false" />
        <div class="mode-actions">
          <button id="webclip-back" class="mode-btn-secondary">Back</button>
          <button id="webclip-save" class="mode-btn-primary" disabled>Clip</button>
        </div>
        <div id="webclip-status" class="mode-status"></div>
      </div>
    </div>
  </div>

  <!-- ========== Search view ========== -->
  <div id="search-view" class="panel hidden">
    <div class="mode-panel">
      <div class="mode-panel-header">
        <svg width="14" height="14" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="8.5" cy="8.5" r="5"/><path d="M12.5 12.5l4 4"/></svg>
        Search Vault
      </div>
      <div class="mode-panel-body">
        <input id="search-input" class="mode-input" type="text" placeholder="Type to search…" autofocus spellcheck="false" />
        <div id="search-results"></div>
        <div id="search-status" class="mode-status"></div>
        <div class="mode-actions" style="margin-top:4px">
          <button id="search-back" class="mode-btn-secondary">Back</button>
        </div>
      </div>
    </div>
  </div>

  <!-- ========== ClipNote view ========== -->
  <div id="clipnote-view" class="panel hidden">
    <div class="mode-panel">
      <div class="mode-panel-header">
        <svg width="14" height="14" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M12 3h4a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h4"/><path d="M13 1v4"/><path d="M7 1v4"/><path d="M7 10l2 2 4-4"/></svg>
        ClipNote
      </div>
      <div class="mode-panel-body">
        <textarea id="clipnote-input" class="mode-textarea" placeholder="Quick thought…" autofocus></textarea>
        <div class="mode-actions">
          <button id="clipnote-back" class="mode-btn-secondary">Back</button>
          <button id="clipnote-save" class="mode-btn-primary" disabled>Save</button>
        </div>
        <div id="clipnote-status" class="mode-status"></div>
      </div>
    </div>
  </div>
</div>

<script>
(function() {
  'use strict';

  // ==================================================================
  // State
  // ==================================================================
  let state = 'pill';
  let items = [];
  let highlightIndex = -1;
  let dragState = null;
  const DRAG_THRESHOLD = 4;

  // ==================================================================
  // DOM refs
  // ==================================================================
  const $ = (id) => document.getElementById(id);
  const pill = $('pill');
  const modeSelector = $('mode-selector');
  const clipboardView = $('clipboard-view');
  const clipboardList = $('clipboard-list');
  const clipboardClear = $('clipboard-clear');
  const webClipView = $('web-clip-view');
  const webclipUrl = $('webclip-url');
  const webclipSave = $('webclip-save');
  const webclipBack = $('webclip-back');
  const webclipStatus = $('webclip-status');
  const searchView = $('search-view');
  const searchInput = $('search-input');
  const searchResults = $('search-results');
  const searchBack = $('search-back');
  const searchStatus = $('search-status');
  const clipnoteView = $('clipnote-view');
  const clipnoteInput = $('clipnote-input');
  const clipnoteSave = $('clipnote-save');
  const clipnoteBack = $('clipnote-back');
  const clipnoteStatus = $('clipnote-status');

  const allViews = [pill, modeSelector, clipboardView, webClipView, searchView, clipnoteView];

  // ==================================================================
  // View helpers
  // ==================================================================
  function showView(view) {
    allViews.forEach(v => v.classList.add('hidden'));
    if (view) view.classList.remove('hidden');
  }

  function setWindowSize(w, h) {
    try { window.electron?.widget?.resize?.({ width: w, height: h }); } catch {}
  }

  function escapeHtml(str) {
    return str.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;')
      .replace(/"/g,'&quot;').replace(/'/g,'&#039;');
  }

  function formatTime(ts) {
    const diff = Date.now() - ts;
    if (diff < 60000) return 'now';
    if (diff < 3600000) return Math.floor(diff / 60000) + 'm';
    if (diff < 86400000) return Math.floor(diff / 3600000) + 'h';
    return Math.floor(diff / 86400000) + 'd';
  }

  // ==================================================================
  // State transitions
  // ==================================================================
  function goToState(newState) {
    state = newState;
    switch (state) {
      case 'pill':
        showView(pill); setWindowSize(38, 38); break;
      case 'modes':
        showView(modeSelector); setWindowSize(304, 76); break;
      case 'clipboard':
        showView(clipboardView); loadItems(); setWindowSize(320, 380); break;
      case 'web-clip':
        showView(webClipView);
        setWindowSize(320, 160);
        webclipUrl.value = '';
        webclipStatus.textContent = '';
        webclipSave.disabled = true;
        setTimeout(() => webclipUrl.focus(), 50);
        break;
      case 'search':
        showView(searchView);
        setWindowSize(320, 380);
        searchInput.value = '';
        searchResults.innerHTML = '';
        searchStatus.textContent = '';
        setTimeout(() => searchInput.focus(), 50);
        break;
      case 'clipnote':
        showView(clipnoteView);
        setWindowSize(320, 210);
        clipnoteInput.value = '';
        clipnoteStatus.textContent = '';
        clipnoteSave.disabled = true;
        setTimeout(() => clipnoteInput.focus(), 50);
        break;
    }
  }

  // ==================================================================
  // Clipboard
  // ==================================================================
  function loadItems() {
    try {
      window.electron?.clipboardHistory?.get(8).then(r => {
        if (r && r.entries) { items = r.entries; highlightIndex = items.length > 0 ? 0 : -1; renderClipboardList(); }
      }).catch(() => { items = []; renderClipboardList(); });
    } catch { items = []; renderClipboardList(); }
  }

  function renderClipboardList() {
    if (items.length === 0) {
      clipboardList.innerHTML = '<div class="clip-empty">No clipboard history yet — copy something to get started</div>';
      return;
    }
    clipboardList.innerHTML = items.map((item, i) => {
      const t = item.text.length > 200 ? item.text.slice(0,200) + '…' : item.text;
      const h = i === highlightIndex ? 'highlighted' : '';
      return '<div class="clip-item ' + h + '" data-index="' + i + '">' +
        '<span class="clip-text" title="' + escapeHtml(item.text) + '">' + escapeHtml(t) + '</span>' +
        '<span class="clip-time">' + formatTime(item.timestamp) + '</span></div>';
    }).join('');
    const el = clipboardList.querySelector('.highlighted');
    if (el) el.scrollIntoView({ block: 'nearest' });
  }

  function selectClip(index) {
    if (index < 0 || index >= items.length) return;
    window.electron?.clipboardHistory?.copy(items[index].text)
      .then(() => goToState('pill'))
      .catch(() => goToState('pill'));
  }

  function clearClipboard() {
    window.electron?.clipboardHistory?.clear().then(() => {
      items = []; highlightIndex = -1; renderClipboardList();
    }).catch(() => {});
  }

  // ==================================================================
  // Web Clip
  // ==================================================================
  let webclipFetching = false;

  async function doWebClip() {
    const url = webclipUrl.value.trim();
    if (!url) return;
    webclipSave.disabled = true;
    webclipStatus.textContent = 'Fetching page…';
    webclipStatus.className = 'mode-status';
    try {
      const resp = await window.electron?.widget?.fetchTitle?.(url);
      const title = resp?.title || url;
      const noteResp = await window.electron?.widget?.createNote?.({
        name: 'Web Clip - ' + title.slice(0, 60),
        content: '# ' + title + '\n\nSource: ' + url + '\n'
      });
      if (noteResp?.success) {
        webclipStatus.textContent = '✓ Saved to vault';
        webclipStatus.className = 'mode-status success';
        setTimeout(() => goToState('pill'), 1200);
      } else {
        throw new Error(noteResp?.error || 'save failed');
      }
    } catch (err) {
      webclipStatus.textContent = '✗ ' + (err.message || 'Failed');
      webclipStatus.className = 'mode-status error';
      webclipSave.disabled = false;
    }
  }

  webclipUrl.addEventListener('input', () => {
    webclipSave.disabled = !webclipUrl.value.trim();
    webclipStatus.textContent = '';
  });
  webclipUrl.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !webclipSave.disabled) doWebClip();
    if (e.key === 'Escape') goToState('modes');
  });
  webclipSave.addEventListener('click', doWebClip);
  webclipBack.addEventListener('click', () => goToState('modes'));

  // ==================================================================
  // Search
  // ==================================================================
  let searchTimer = null;

  async function doSearch(query) {
    if (!query) { searchResults.innerHTML = ''; searchStatus.textContent = ''; return; }
    searchStatus.textContent = 'Searching…';
    try {
      const resp = await window.electron?.search?.query(query);
      const results = resp?.results || [];
      searchStatus.textContent = results.length + ' result' + (results.length !== 1 ? 's' : '');
      if (results.length === 0) {
        searchResults.innerHTML = '<div class="clip-empty">No results found</div>';
        return;
      }
      searchResults.innerHTML = results.map(r =>
        '<div class="search-result-item" data-path="' + escapeHtml(r.filePath) + '">' +
          '<div class="search-result-name">' + escapeHtml(r.name) + '</div>' +
          '<div class="search-result-path">' + escapeHtml(r.relativePath || '') + '</div>' +
        '</div>'
      ).join('');
    } catch {
      searchStatus.textContent = 'Search failed';
    }
  }

  searchInput.addEventListener('input', () => {
    clearTimeout(searchTimer);
    const q = searchInput.value.trim();
    if (!q) { searchResults.innerHTML = ''; searchStatus.textContent = ''; return; }
    searchTimer = setTimeout(() => doSearch(q), 300);
  });
  searchInput.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') goToState('modes');
  });
  searchResults.addEventListener('click', (e) => {
    const item = e.target.closest('.search-result-item');
    if (!item) return;
    const path = item.dataset.path;
    if (path) {
      window.electron?.widget?.openNote?.(path);
      goToState('pill');
    }
  });
  searchBack.addEventListener('click', () => goToState('modes'));

  // ==================================================================
  // ClipNote
  // ==================================================================
  async function doSaveClipnote() {
    const text = clipnoteInput.value.trim();
    if (!text) return;
    clipnoteSave.disabled = true;
    clipnoteStatus.textContent = 'Saving…';
    clipnoteStatus.className = 'mode-status';
    try {
      const resp = await window.electron?.widget?.createNote?.({
        name: 'Clipnote',
        content: text,
        timestamp: true
      });
      if (resp?.success) {
        clipnoteStatus.textContent = '✓ Note saved';
        clipnoteStatus.className = 'mode-status success';
        setTimeout(() => goToState('pill'), 1200);
      } else {
        throw new Error(resp?.error || 'save failed');
      }
    } catch (err) {
      clipnoteStatus.textContent = '✗ ' + (err.message || 'Failed');
      clipnoteStatus.className = 'mode-status error';
      clipnoteSave.disabled = false;
    }
  }

  clipnoteInput.addEventListener('input', () => {
    clipnoteSave.disabled = !clipnoteInput.value.trim();
    clipnoteStatus.textContent = '';
  });
  clipnoteInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && !clipnoteSave.disabled) doSaveClipnote();
    if (e.key === 'Escape' && !clipnoteInput.value.trim()) goToState('modes');
  });
  clipnoteSave.addEventListener('click', doSaveClipnote);
  clipnoteBack.addEventListener('click', () => goToState('modes'));

  // ==================================================================
  // Click handlers
  // ==================================================================
  modeSelector.addEventListener('click', (e) => {
    const btn = e.target.closest('.mode-btn');
    if (!btn) return;
    goToState(btn.dataset.mode);
  });

  clipboardList.addEventListener('click', (e) => {
    const item = e.target.closest('.clip-item');
    if (!item) return;
    const idx = parseInt(item.dataset.index, 10);
    if (!isNaN(idx)) selectClip(idx);
  });
  clipboardClear.addEventListener('click', clearClipboard);

  // ==================================================================
  // Drag-to-move
  // ==================================================================
  document.addEventListener('mousedown', (e) => {
    dragState = { startX: e.screenX, startY: e.screenY, moved: false };
  });
  document.addEventListener('mousemove', (e) => {
    if (!dragState) return;
    const dx = e.screenX - dragState.startX;
    const dy = e.screenY - dragState.startY;
    if (Math.abs(dx) > DRAG_THRESHOLD || Math.abs(dy) > DRAG_THRESHOLD) dragState.moved = true;
    if (dx !== 0 || dy !== 0) {
      try { window.electron?.widget?.move?.(dx, dy); } catch {}
    }
  });
  document.addEventListener('mouseup', () => {
    if (!dragState) return;
    const wasClick = !dragState.moved;
    dragState = null;
    if (wasClick && state === 'pill') goToState('modes');
  });

  // ==================================================================
  // Keyboard navigation
  // ==================================================================
  document.addEventListener('keydown', (e) => {
    switch (state) {
      case 'clipboard':
        if (e.key === 'ArrowDown') { e.preventDefault(); highlightIndex = Math.min(highlightIndex + 1, items.length - 1); if (highlightIndex < 0) highlightIndex = 0; renderClipboardList(); }
        else if (e.key === 'ArrowUp') { e.preventDefault(); highlightIndex = Math.max(highlightIndex - 1, 0); renderClipboardList(); }
        else if (e.key === 'Enter') { e.preventDefault(); selectClip(highlightIndex); }
        else if (e.key === 'Escape') { e.preventDefault(); goToState('modes'); }
        break;
      case 'modes':
        if (e.key === 'Escape') { e.preventDefault(); goToState('pill'); }
        break;
    }
  });

  // ==================================================================
  // Init
  // ==================================================================
  goToState('pill');
  window.addEventListener('focus', () => { goToState('pill'); });

  // Preload clipboard data
  window.electron?.clipboardHistory?.get(8).then(r => {
    if (r && r.entries) items = r.entries;
  }).catch(() => {});

  // ==================================================================
  // Listen for "show clipboard" from main process (fn key hold)
  // ==================================================================
  window.electron?.on?.showClipboard?.(() => {
    goToState('clipboard');
  });
})();
</script>
</body>
</html>`
}
