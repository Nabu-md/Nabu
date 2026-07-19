# Phase 7.1 — Storage Map

**Nabu Recovery Program — Storage, Search & PDF Inventory (Prompt A)**

This document inventories every persistence mechanism in the Nabu repository.
No production code was modified during this discovery phase.

---

## 1. Storage Technologies Discovered

| # | Technology | Location | Format | Owner |
|---|------------|----------|--------|-------|
| 1 | App Settings (userData) | `userData/settings.json` | JSON | `settings.ts` |
| 2 | Vault `.nabu/` cache dir | `<vault>/.nabu/` | Directory | `state.ts`, `vault-service.ts` |
| 3 | Vectra vector index | `<vault>/.nabu/vectra/` | Vectra binary + JSON | `vector.ts` |
| 4 | BGE-micro ONNX model | `resources/models/bge-micro-v2/` (dev) / `process.resourcesPath/models/` (prod) | ONNX | `vector.ts` |
| 5 | PDF annotations | `<vault>/.nabu/pdf-annotations/<name>.json` | JSON | `pdf-viewer.ts` |
| 6 | View state (fold states) | `<vault>/.nabu/view-state/<note>.json` | JSON | `view-state.ts` |
| 7 | Bookmarks | `<vault>/.nabu/bookmarks.json` | JSON | `bookmarks.ts` |
| 8 | Favorites | `<vault>/.nabu/favorites.json` | JSON | `favorites.ts` |
| 9 | Snapshots (recovery) | `<vault>/.nabu/snapshots/<note>-<ts>.json` | JSON | `snapshots.ts` |
| 10 | Clipboard history | `userData/clipboard-history.json` | JSON | `clipboard-history.ts` |
| 11 | Note markdown files | `<vault>/**/*.md` | Markdown (UTF-8) | `notes.ts` IPC |
| 12 | Templates | `<vault>/_templates/*.md` | Markdown | `vault-service.ts` |
| 13 | Whisper models | `resources/whisper-models/` (dev) / `process.resourcesPath/whisper-models/` (prod) | GGML binary | `whisper.ts` |
| 14 | In-memory AST store | `StateManager.astStore` (Map) | mdast Root objects | `state.ts` |
| 15 | In-memory indexes | `StateManager.fullTextIndex`, `tagIndex`, `extendedIndex` | Maps/Sets | `state.ts` |
| 16 | Workspace state | In-memory + `settings.json` | JSON | `workspace-service.ts` |

---

## 2. Persistence Paths & Formats

### 2.1 App Settings — `userData/settings.json`
- **Owner:** `src/main/services/settings.ts`
- **Format:** Pretty-printed JSON (`JSON.stringify(settings, null, 2)`)
- **Schema:** `AppSettings` interface (lines 25–41)
  - `lastVaultPath: string | null`
  - `windowBounds: {x,y,width,height} | null`
  - `theme: 'dark' | 'light' | 'system'`
  - `autoProperties: boolean`
  - `dailyNoteDateFormat`, `dailyNoteFolder`, `dailyNoteTemplate`
  - `recentVaults: RecentVaultEntry[]` (`{path, name, lastOpened}`)
  - `clipboardShortcut: string`
- **Save path:** `app.getPath('userData') + '/settings.json'`
- **Load path:** same
- **Migration logic:** `updateRecentVaults()` migrates v1 `lastVaultPath` → `recentVaults` list (lines 96–114)
- **Recovery logic:** On load failure, returns `DEFAULT_SETTINGS` (lines 78–80). Save failures are logged, never thrown (lines 120–125).

### 2.2 Vault `.nabu/` Cache Directory
- **Owner:** `src/main/services/state.ts` (`openVault`, lines 79–104) + `vault-service.ts`
- **Created at:** vault open time via `fs.mkdir(nabuDir, { recursive: true })`
- **`.gitignore` sync:** Appends `.nabu/` to `.gitignore` if missing (lines 86–100)
- **Sub-directories:**
  - `.nabu/vectra/` — vector index (see 2.3)
  - `.nabu/pdf-annotations/` — PDF annotations (see 2.5)
  - `.nabu/view-state/` — fold states (see 2.6)
  - `.nabu/bookmarks.json` — bookmarks (see 2.7)
  - `.nabu/favorites.json` — favorites (see 2.8)
  - `.nabu/snapshots/` — recovery snapshots (see 2.9)

### 2.3 Vectra Vector Index — `.nabu/vectra/`
- **Owner:** `src/main/services/vector.ts` (`VectorManager`)
- **Format:** Vectra `LocalIndex` (binary + JSON shard files on disk)
- **Index path:** `path.join(vaultPath, '.nabu', 'vectra')` (line 121)
- **Metadata per vector:** `{ path, name, mtime, charCount }` (lines 36–41)
- **Model:** BGE-micro-v2 ONNX (384-dim embeddings)
- **Serialization:** Vectra handles its own on-disk format; upsert via `index.upsertItem({id, vector, metadata})`
- **Recovery logic:** If index corrupted (`getIndexStats()` throws), rebuilds in background via `rebuildIndex()` (lines 133–142, 363–375)
- **Disabled path:** If ONNX model missing, embeddings disabled, index not used (lines 304–327)

### 2.4 BGE-micro ONNX Model
- **Owner:** `src/main/services/vector.ts` (`loadModel`, lines 304–327)
- **Dev path:** `resources/models/bge-micro-v2/` (relative to repo root)
- **Prod path:** `process.resourcesPath/models/bge-micro-v2/`
- **Format:** ONNX model directory loaded via `@xenova/transformers` pipeline
- **Config:** `env.localModelPath`, `env.allowRemoteModels = false` (no network at runtime)

### 2.5 PDF Annotations — `.nabu/pdf-annotations/`
- **Owner:** `src/main/services/pdf-viewer.ts` (`getAnnotationsPath`, lines 208–215)
- **Path:** `<vault>/.nabu/pdf-annotations/<pdfName>.json`
- **Format:** JSON array of `PDFAnnotation` objects
  ```ts
  interface PDFAnnotation {
    id: string; page: number;
    rect: {x,y,w,h}; text: string;
    color: 'yellow'|'green'|'blue'|'pink'|'orange';
    comment?: string; timestamp: number; linkedNotePath?: string;
  }
  ```
- **Save:** `fs.writeFile(path, JSON.stringify(annotations, null, 2))` (line 253)
- **Load:** `fs.readFile` → `JSON.parse`; returns `[]` on `ENOENT` (lines 221–236)
- **Cache:** `clearPDFCache()` / `clearAllPDFCache()` are no-ops (pdfjs handles internally)

### 2.6 View State (Fold States) — `.nabu/view-state/`
- **Owner:** `src/main/services/view-state.ts`
- **Path:** `<vault>/.nabu/view-state/<vaultRelativeNotePath>.json`
  - Note path converted via `toVaultRelative()` then `/` → `--` (lines 39–47)
- **Format:** `{ foldStates: Record<headingId, boolean> }`
- **In-memory cache:** `viewStateCache: Map<string, ViewState>` keyed by `vaultPath:notePath` (line 23)
- **Save:** `saveViewState()` merges and writes JSON (lines 96–116)
- **Load:** Checks memory cache first, then disk; empty state on miss (lines 69–90)

### 2.7 Bookmarks — `.nabu/bookmarks.json`
- **Owner:** `src/main/bookmarks.ts`
- **Path:** `<vault>/.nabu/bookmarks.json`
- **Format:** `Record<listName, string[]>` (named lists of file paths)
- **Operations:** `readBookmarks`, `writeBookmarks`, `addBookmark`, `removeBookmark`, `removeFileFromBookmarks`
- **Recovery:** Returns `{}` on parse error (lines 30–41)

### 2.8 Favorites — `.nabu/favorites.json`
- **Owner:** `src/main/favorites.ts`
- **Path:** `<vault>/.nabu/favorites.json`
- **Format:** `string[]` (file paths)
- **Operations:** `readFavorites`, `writeFavorites`, `toggleFavorite`, `removeFavorite`
- **Recovery:** Returns `[]` on parse error (lines 25–36)

### 2.9 Snapshots (Recovery) — `.nabu/snapshots/`
- **Owner:** `src/main/snapshots.ts`
- **Path:** `<vault>/.nabu/snapshots/<relativeNotePath>-<timestamp>.json`
- **Format:** `{ timestamp, content, path }`
- **Caps:** Per-note cap = 50, per-vault cap = 1000 (lines 61–64)
- **Pruning:** `pruneNoteSnapshots()` and `pruneVaultSnapshots()` (lines 73–129)
- **Restore:** `restoreSnapshot()` writes pre-restore snapshot then overwrites (lines 162–189)
- **Recovery logic:** Corrupted snapshots skipped on list (lines 141–151)

### 2.10 Clipboard History — `userData/clipboard-history.json`
- **Owner:** `src/main/services/clipboard-history.ts` (`ClipboardHistory`)
- **Path:** `app.getPath('userData') + '/clipboard-history.json'`
- **Format:** `ClipboardEntry[]` (`{id, text, timestamp}`)
- **Max entries:** 50 (DEFAULT_MAX_ENTRIES)
- **Polling:** 500ms interval, dedup consecutive identical copies
- **Recovery:** Starts fresh on load error (lines 160–175)

### 2.11 Note Markdown Files — `<vault>/**/*.md`
- **Owner:** `src/main/ipc/notes.ts` (IPC handlers)
- **Format:** UTF-8 Markdown with optional YAML frontmatter
- **Write paths:** `note:create`, `note:save`, `note:rename`, `note:delete`, `note:daily`, `note:unique`, `note:export-html`, `properties:write`
- **Pending write lock:** `stateManager.setPendingWrite()` / `clearPendingWrite()` guards watcher from re-parsing app writes (e.g. notes.ts lines 161–166)
- **Auto-properties:** `injectAutoProperty()` adds `created`/`modified` timestamps (settings.ts, shared.ts)

### 2.12 Templates — `<vault>/_templates/`
- **Owner:** `src/main/services/vault-service.ts` (`copyDefaultTemplates`, lines 85–111)
- **Source (dev):** `resources/default-templates/*.md`
- **Source (prod):** `process.resourcesPath/default-templates/`
- **Copied on first vault open only** (skips if `_templates` exists)

### 2.13 Whisper Models — `whisper-models/`
- **Owner:** `src/main/services/whisper.ts`
- **Dev path:** `resources/whisper-models/`
- **Prod path:** `process.resourcesPath/whisper-models/`
- **Format:** GGML binary (`ggml-base.en.bin`, `ggml-large-v3-turbo-q5_0.bin`)
- **Download:** `downloadModel()` with SHA256 verification (lines 276–348)

---

## 3. In-Memory Caches (Non-Persistent)

| Cache | Owner | Type | Purpose |
|-------|-------|------|---------|
| `astStore` | `state.ts` | `Map<string, Root>` | Parsed markdown ASTs keyed by path |
| `fullTextIndex` | `state.ts` | `Map<string, Set<string>>` | Word → file paths |
| `tagIndex` | `state.ts` | `Map<string, Set<string>>` | Tag → file paths |
| `extendedIndex` | `state.ts` | `ExtendedSearchIndex` | Positions, snippets, aliases, properties, blockRefs |
| `pendingWrites` | `state.ts` | `Map<string, {timeout}>` | App-initiated write lock (2s auto-expire) |
| `viewStateCache` | `view-state.ts` | `Map<string, ViewState>` | Fold states per note |
| `history` | `clipboard-history.ts` | `ClipboardEntry[]` | Clipboard items (also persisted) |
| `sessions` | `vault-registry.ts` | `Map<string, VaultSession>` | Multi-vault sessions |

---

## 4. Persistence Flow (Per Object Document)

### 4.1 Note Content
```
Note (markdown string)
  ↓ owner: NoteView (renderer) / noteCommands
Serialization: UTF-8 write via fs.writeFile
  ↓ IPC: note:save (preload → ipcMain)
Storage Medium: <vault>/<name>.md
  ↓ Loading Path: note:get-raw / file watcher (chokidar)
Consumer: StateManager.getAST → renderer NoteView
```

### 4.2 Settings
```
AppSettings object
  ↓ owner: WorkspaceService / settings IPC
Serialization: JSON.stringify(settings, null, 2)
  ↓ saveSettings()
Storage Medium: userData/settings.json
  ↓ Loading Path: loadSettings() at startup
Consumer: index.ts, vault-service.ts, workspace-service.ts
```

### 4.3 Vector Embedding
```
Note text
  ↓ owner: VectorManager.embedFile()
Serialization: BGE-micro ONNX → 384-dim float vector
  ↓ index.upsertItem({id: path, vector, metadata})
Storage Medium: <vault>/.nabu/vectra/
  ↓ Loading Path: LocalIndex queryItems()
Consumer: context:query IPC → CommandPalette / ContextPane
```

### 4.4 PDF Annotation
```
PDFAnnotation[]
  ↓ owner: PdfViewer (renderer) → pdf:saveAnnotations IPC
Serialization: JSON.stringify(annotations, null, 2)
  ↓ savePDFAnnotations()
Storage Medium: <vault>/.nabu/pdf-annotations/<name>.json
  ↓ Loading Path: loadPDFAnnotations() on PDF open
Consumer: PdfViewer (renders highlight overlays)
```

### 4.5 View State (Fold)
```
ViewState { foldStates }
  ↓ owner: NoteView → view-state:set-fold IPC
Serialization: JSON.stringify(merged, null, 2)
  ↓ saveViewState()
Storage Medium: <vault>/.nabu/view-state/<note>.json
  ↓ Loading Path: loadViewState() / view-state:get-fold
Consumer: NoteView (collapsible headings)
```

### 4.6 Recovery Snapshot
```
Note content (pre-save)
  ↓ owner: snapshots.createSnapshot() (called before note:save)
Serialization: JSON.stringify({timestamp, content, path})
  ↓ fs.writeFile()
Storage Medium: <vault>/.nabu/snapshots/<note>-<ts>.json
  ↓ Loading Path: listSnapshots() / restoreSnapshot()
Consumer: Recovery UI (future) / restoreSnapshot()
```

---

## 5. Migration & Recovery Logic Summary

| Mechanism | Migration | Recovery |
|-----------|-----------|----------|
| Settings | v1 `lastVaultPath` → `recentVaults` | Defaults on parse error |
| Vector index | n/a | Rebuild on corruption detection |
| Embeddings | n/a | Disabled if model missing |
| Snapshots | n/a | Skip corrupted; prune to caps |
| Bookmarks/Favorites | n/a | `{}` / `[]` on parse error |
| Clipboard | n/a | Fresh start on load error |
| PDF annotations | n/a | `[]` on `ENOENT` |

---

## 6. Ownership Summary

| Subsystem | Owning Feature | Owning Service | Persistence Owner |
|-----------|---------------|----------------|-------------------|
| App settings | Settings | `settings.ts` | `settings.ts` |
| Vault cache | Vault | `vault-service.ts` | `state.ts` |
| Vector index | Search/Semantic | `vector.ts` | `vector.ts` |
| PDF annotations | PDF | `pdf-service.ts` → `pdf-viewer.ts` | `pdf-viewer.ts` |
| View state | Notes (headings) | `view-state.ts` | `view-state.ts` |
| Bookmarks | Bookmarks | `bookmarks.ts` | `bookmarks.ts` |
| Favorites | Favorites | `favorites.ts` | `favorites.ts` |
| Snapshots | Recovery | `snapshots.ts` | `snapshots.ts` |
| Clipboard | Widget | `clipboard-history.ts` | `clipboard-history.ts` |
| Notes | Notes | `notes.ts` IPC | `notes.ts` |
| Workspace | Workspace | `workspace-service.ts` | `workspace-service.ts` |
| Whisper models | Dictation | `whisper.ts` | `whisper.ts` |

---

*End of Storage Map — Phase 7.1 Prompt A. No production code modified.*
