/**
 * widget-template.ts
 *
 * Returns the HTML string for the clipboard-widget BrowserWindow.
 * The widget is a small floating pill that expands to show clipboard history
 * or mode shortcuts. Inspired by Wispr Flow's UX.
 *
 * States:
 *   pill      → small sparkle icon (top-left, draggable)
 *   modes     → four mode squares (Clipboard / Web Clip / Search / Quick Capture)
 *   clipboard → scrollable list with arrow-key navigation (Enter to paste)
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

  /* ==================================================================
     Container
     ================================================================== */
  #root {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    justify-content: flex-start;
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
     Pill state
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
  #mode-selector {
    display: flex;
    gap: 6px;
    padding: 8px;
  }
  .mode-btn {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    width: 64px;
    padding: 8px 4px 6px;
    border-radius: 6px;
    cursor: pointer;
    border: none;
    background: transparent;
    color: var(--text-dim);
    font-size: 10px;
    transition: background 0.12s, color 0.12s;
  }
  .mode-btn:hover {
    background: var(--accent-alpha);
    color: var(--text);
  }
  .mode-btn.active {
    background: var(--accent-alpha);
    color: var(--accent);
  }
  .mode-btn svg { width: 20px; height: 20px; }

  /* ==================================================================
     Clipboard list
     ================================================================== */
  #clipboard-view {
    width: 100%;
  }
  #clipboard-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px 6px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-dim);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    border-bottom: 1px solid var(--border);
  }
  #clipboard-clear {
    background: none;
    border: none;
    color: var(--text-dim);
    font-size: 10px;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 4px;
  }
  #clipboard-clear:hover { background: var(--accent-alpha); color: var(--accent); }
  #clipboard-list {
    max-height: 340px;
    overflow-y: auto;
    padding: 4px 6px;
  }
  #clipboard-list::-webkit-scrollbar { width: 4px; }
  #clipboard-list::-webkit-scrollbar-track { background: transparent; }
  #clipboard-list::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.1); border-radius: 2px; }

  .clip-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 8px;
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.1s;
    border: 1px solid transparent;
  }
  .clip-item:hover { background: rgba(255,255,255,0.04); }
  .clip-item.highlighted {
    background: var(--accent-alpha);
    border-color: rgba(94, 148, 255, 0.25);
  }
  .clip-text {
    flex: 1;
    font-size: 12px;
    line-height: 1.4;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 240px;
  }
  .clip-time {
    font-size: 10px;
    color: var(--text-dim);
    white-space: nowrap;
    flex-shrink: 0;
  }
  .clip-empty {
    padding: 20px;
    text-align: center;
    color: var(--text-dim);
    font-size: 12px;
  }

  /* ==================================================================
     Placeholder modes
     ================================================================== */
  .placeholder-mode {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 20px 24px;
    width: 100%;
    color: var(--text-dim);
    font-size: 12px;
  }
  .placeholder-mode svg { width: 28px; height: 28px; opacity: 0.4; }

  /* ==================================================================
     Utility: hidden
     ================================================================== */
  .hidden { display: none !important; }

  /* ==================================================================
     Snippet preview for clipboard items
     ================================================================== */
  .clip-line-clamp {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    white-space: normal;
    word-break: break-all;
  }
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
        <rect x="3" y="3" width="14" height="14" rx="2"/>
        <path d="M7 7h6"/>
        <path d="M7 10h6"/>
        <path d="M7 13h4"/>
      </svg>
      Clipboard
    </button>
    <button class="mode-btn" data-mode="web-clip" title="Web Clipper">
      <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M10 5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-4a1 1 0 0 1-1-1"/>
        <path d="M8 7l-3 3 3 3"/>
        <path d="M5 10h8"/>
      </svg>
      Web Clip
    </button>
    <button class="mode-btn" data-mode="search" title="Quick Search">
      <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="8.5" cy="8.5" r="5"/>
        <path d="M12.5 12.5l4 4"/>
      </svg>
      Search
    </button>
    <button class="mode-btn" data-mode="capture" title="Quick Capture">
      <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M10 4v12"/>
        <path d="M4 10h12"/>
      </svg>
      Quick
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

  <!-- ========== Web Clip placeholder ========== -->
  <div id="web-clip-view" class="panel hidden">
    <div class="placeholder-mode">
      <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M10 5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-4a1 1 0 0 1-1-1"/>
        <path d="M8 7l-3 3 3 3"/>
        <path d="M5 10h8"/>
      </svg>
      <span>Web Clipper — coming soon</span>
    </div>
  </div>

  <!-- ========== Search placeholder ========== -->
  <div id="search-view" class="panel hidden">
    <div class="placeholder-mode">
      <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="8.5" cy="8.5" r="5"/>
        <path d="M12.5 12.5l4 4"/>
      </svg>
      <span>Quick Search — coming soon</span>
    </div>
  </div>

  <!-- ========== Capture placeholder ========== -->
  <div id="capture-view" class="panel hidden">
    <div class="placeholder-mode">
      <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M10 4v12"/>
        <path d="M4 10h12"/>
      </svg>
      <span>Quick Capture — coming soon</span>
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
  const DRAG_THRESHOLD = 4; // px before we consider it a drag

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
  const searchView = $('search-view');
  const captureView = $('capture-view');

  const allViews = [pill, modeSelector, clipboardView, webClipView, searchView, captureView];

  // ==================================================================
  // View helpers
  // ==================================================================
  function showView(view) {
    allViews.forEach(v => v.classList.add('hidden'));
    if (view) view.classList.remove('hidden');
  }

  function setWindowSize(width, height) {
    try {
      // Send resize request to main process
      window.electron?.widget?.resize?.({ width, height });
    } catch { /* widget may not have this API yet */ }
  }

  // ==================================================================
  // State transitions
  // ==================================================================
  function goToState(newState) {
    state = newState;
    switch (state) {
      case 'pill':
        showView(pill);
        setWindowSize(38, 38);
        break;
      case 'modes':
        showView(modeSelector);
        setWindowSize(304, 76);
        break;
      case 'clipboard':
        showView(clipboardView);
        loadItems();
        setWindowSize(320, 380);
        break;
      case 'web-clip':
        showView(webClipView);
        setWindowSize(260, 120);
        break;
      case 'search':
        showView(searchView);
        setWindowSize(260, 120);
        break;
      case 'capture':
        showView(captureView);
        setWindowSize(260, 120);
        break;
    }
  }

  // ==================================================================
  // Clipboard loading
  // ==================================================================
  function loadItems() {
    try {
      window.electron?.clipboardHistory?.get(8).then(result => {
        if (result && result.entries) {
          items = result.entries;
          highlightIndex = items.length > 0 ? 0 : -1;
          renderClipboardList();
        }
      }).catch(() => {
        items = [];
        renderClipboardList();
      });
    } catch {
      items = [];
      renderClipboardList();
    }
  }

  function renderClipboardList() {
    if (items.length === 0) {
      clipboardList.innerHTML = '<div class="clip-empty">No clipboard history yet — copy something to get started</div>';
      return;
    }
    clipboardList.innerHTML = items.map((item, i) => {
      const time = formatTime(item.timestamp);
      const text = item.text.length > 200 ? item.text.slice(0, 200) + '…' : item.text;
      const highlighted = i === highlightIndex ? 'highlighted' : '';
      return '<div class="clip-item ' + highlighted + '" data-index="' + i + '">' +
        '<span class="clip-text" title="' + escapeHtml(item.text) + '">' + escapeHtml(text) + '</span>' +
        '<span class="clip-time">' + time + '</span>' +
        '</div>';
    }).join('');
    // Ensure highlighted item is visible
    const highlightedEl = clipboardList.querySelector('.highlighted');
    if (highlightedEl) {
      highlightedEl.scrollIntoView({ block: 'nearest' });
    }
  }

  function formatTime(ts) {
    const diff = Date.now() - ts;
    if (diff < 60000) return 'now';
    if (diff < 3600000) return Math.floor(diff / 60000) + 'm';
    if (diff < 86400000) return Math.floor(diff / 3600000) + 'h';
    return Math.floor(diff / 86400000) + 'd';
  }

  function escapeHtml(str) {
    return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;').replace(/'/g, '&#039;');
  }

  // ==================================================================
  // Actions
  // ==================================================================
  function selectClip(index) {
    if (index < 0 || index >= items.length) return;
    const text = items[index].text;
    // Copy to system clipboard
    try {
      window.electron?.clipboardHistory?.copy(text).then(() => {
        // Dismiss widget — hide and let users paste naturally
        dismissWidget();
      }).catch(() => dismissWidget());
    } catch {
      dismissWidget();
    }
  }

  function clearClipboard() {
    window.electron?.clipboardHistory?.clear().then(() => {
      items = [];
      highlightIndex = -1;
      renderClipboardList();
    }).catch(() => {});
  }

  function dismissWidget() {
    goToState('pill');
  }

  // ==================================================================
  // Click handlers
  // ==================================================================
  pill.addEventListener('click', (e) => {
    goToState('modes');
  });

  modeSelector.addEventListener('click', (e) => {
    const btn = e.target.closest('.mode-btn');
    if (!btn) return;
    const mode = btn.dataset.mode;
    goToState(mode);
  });

  clipboardList.addEventListener('click', (e) => {
    const item = e.target.closest('.clip-item');
    if (!item) return;
    const idx = parseInt(item.dataset.index, 10);
    if (!isNaN(idx)) selectClip(idx);
  });

  clipboardClear.addEventListener('click', () => clearClipboard());

  // ==================================================================
  // Drag-to-move (manual via IPC)
  // ==================================================================
  document.addEventListener('mousedown', (e) => {
    dragState = {
      startX: e.screenX,
      startY: e.screenY,
      moved: false
    };
  });

  document.addEventListener('mousemove', (e) => {
    if (!dragState) return;
    const dx = e.screenX - dragState.startX;
    const dy = e.screenY - dragState.startY;
    if (Math.abs(dx) > DRAG_THRESHOLD || Math.abs(dy) > DRAG_THRESHOLD) {
      dragState.moved = true;
    }
    if (dx !== 0 || dy !== 0) {
      try {
        window.electron?.widget?.move?.(dx, dy);
      } catch {}
    }
  });

  document.addEventListener('mouseup', () => {
    if (!dragState) return;
    const wasClick = !dragState.moved;
    dragState = null;
    if (wasClick && state === 'pill') {
      // If pill was clicked without dragging, expand
      goToState('modes');
    }
  });

  // ==================================================================
  // Keyboard navigation (Clipboard mode only)
  // ==================================================================
  document.addEventListener('keydown', (e) => {
    switch (state) {
      case 'clipboard':
        if (e.key === 'ArrowDown') {
          e.preventDefault();
          highlightIndex = Math.min(highlightIndex + 1, items.length - 1);
          if (highlightIndex < 0) highlightIndex = 0;
          renderClipboardList();
        } else if (e.key === 'ArrowUp') {
          e.preventDefault();
          highlightIndex = Math.max(highlightIndex - 1, 0);
          renderClipboardList();
        } else if (e.key === 'Enter') {
          e.preventDefault();
          selectClip(highlightIndex);
        } else if (e.key === 'Escape') {
          e.preventDefault();
          goToState('modes');
        }
        break;

      case 'modes':
        if (e.key === 'Escape') {
          e.preventDefault();
          goToState('pill');
        }
        break;

      case 'pill':
        if (e.key === 'Escape') {
          // Widget will be hidden by the global shortcut toggle
          // This is handled on the main process side
        }
        break;
    }
  });

  // ==================================================================
  // Initial load — start as pill
  // ==================================================================
  goToState('pill');

  // Always reset to pill when the widget is shown (after a hide)
  window.addEventListener('focus', () => {
    goToState('pill');
  });

  // Pre-load clipboard data so it's ready when user opens clipboard view
  window.electron?.clipboardHistory?.get(8).then(result => {
    if (result && result.entries) items = result.entries;
  }).catch(() => {});

})();
</script>
</body>
</html>`
}
