# Phase 7.1 — PDF Map

**Nabu Recovery Program — Storage, Search & PDF Inventory (Prompt A)**

This document inventories every PDF-related subsystem: rendering, loading, viewer,
annotations, navigation, persistence, caching, IPC, and services.
No production code was modified during this discovery phase.

---

## 1. PDF Subsystems Inventory

| # | Subsystem | Type | Location | Owner |
|---|-----------|------|----------|-------|
| 1 | PDF Service (orchestration) | Main service | `src/main/services/pdf-service.ts` | `PdfService` |
| 2 | PDF Engine (pdfjs + canvas) | Main service | `src/main/services/pdf-viewer.ts` | `pdf-viewer.ts` |
| 3 | PDF IPC handlers | IPC module | `src/main/ipc/pdf.ts` | `pdf.ts` IPC |
| 4 | PDF Viewer (renderer) | React component | `src/renderer/src/features/pdf/PdfViewer.tsx` | `PdfViewer` |
| 5 | PDF Commands (renderer) | Command module | `src/renderer/src/features/pdf/pdfCommands.ts` | `pdfCommands` |
| 6 | PDF Annotation persistence | Main service | `src/main/services/pdf-viewer.ts` | `pdf-viewer.ts` |
| 7 | PDF Preload API | Preload bridge | `src/preload/index.ts` (`electron.pdf`) | Preload |
| 8 | PDF Importer | Main service | `src/main/services/pdf-importer.ts` | `pdf-importer.ts` |
| 9 | Wiki-link PDF refs | Shared types | `src/shared/types.ts` (`WikiLink.pageRef`) | Shared |

---

## 2. Rendering Flow

### 2.1 Open PDF
```
Open PDF (filePath)
  ↓ Renderer: PdfViewer useEffect → window.electron.pdf.open(filePath)
IPC: pdf:open (preload → ipcMain)
  ↓ PdfService.open() → getPDFInfo(filePath)
Service: pdf-viewer.ts getPDFInfo()
  ↓ pdfjs getDocument({data: Uint8Array}) → numPages + metadata
Rendering: returns {totalPages, metadata:{title,author,subject,keywords}}
  ↓ IPC result → PdfViewer setTotalPages + setMetadata
Interaction: Toolbar shows page count, metadata
```

**Files:**
- `PdfViewer.tsx` lines 60–95 (open effect)
- `pdf.ts` lines 38–51 (`PDF_OPEN` handler)
- `pdf-service.ts` lines 51–78 (`open`)
- `pdf-viewer.ts` lines 80–102 (`getPDFInfo`)

### 2.2 Render Page
```
Render Page (filePath, pageNumber, scale)
  ↓ Renderer: PdfViewer.renderPage() → window.electron.pdf.renderPage(...)
IPC: pdf:render-page
  ↓ PdfService.renderPage() → renderPDFPage(filePath, pageNumber, scale)
Service: pdf-viewer.ts renderPDFPage()
  ↓ pdfjs getPage(n) → getViewport({scale})
  ↓ canvas (node) → page.render() → toDataURL('image/png')
Rendering: base64 PNG data URI
  ↓ IPC result → PdfViewer setPageImages[pageNumber] = dataUri
Interaction: <img src={dataUri}> displayed
```

**Files:**
- `PdfViewer.tsx` lines 145–162 (renderPage), 164–172 (lazy render)
- `pdf.ts` lines 56–71 (`PDF_RENDER_PAGE`)
- `pdf-service.ts` lines 84–120 (`renderPage`)
- `pdf-viewer.ts` lines 141–187 (`renderPDFPage`)

### 2.3 Lazy Rendering Strategy
- **Current page + 1 buffer each direction** (PdfViewer.tsx lines 165–172)
- Pages rendered on: `currentPage` change, `totalPages` change
- **Zoom change** invalidates cached rasterizations (`setPageImages({})`, line 175–177)
- **Cache:** `pageImages: Record<number, string>` (in-memory React state)

---

## 3. Annotation Flow

### 3.1 Load Annotations
```
Load Annotations (filePath)
  ↓ Renderer: PdfViewer useEffect → window.electron.pdf.loadAnnotations(filePath)
IPC: pdf:load-annotations
  ↓ PdfService.loadAnnotations() → loadPDFAnnotations(filePath)
Service: pdf-viewer.ts loadPDFAnnotations()
  ↓ fs.readFile(.nabu/pdf-annotations/<name>.json) → JSON.parse
Persistence: returns PDFAnnotation[] (or [] on ENOENT)
  ↓ IPC result → PdfViewer setAnnotations
Interaction: Highlights rendered as absolutely-positioned overlays
```

**Files:**
- `PdfViewer.tsx` lines 106–127 (load effect)
- `pdf.ts` lines 76–88 (`PDF_LOAD_ANNOTATIONS`)
- `pdf-service.ts` lines 126–145 (`loadAnnotations`)
- `pdf-viewer.ts` lines 221–236 (`loadPDFAnnotations`)

### 3.2 Create Annotation (User Interaction)
```
User selects text on page image (mouseup)
  ↓ PdfViewer.handlePageMouseUp() computes rect from selection vs img bounding box
  ↓ Creates PDFAnnotation {id: crypto.randomUUID(), page, rect, text, color, timestamp}
Interaction: setAnnotations([...prev, newAnnotation])
  ↓ Debounced save effect (500ms) → window.electron.pdf.saveAnnotations(filePath, annotations)
IPC: pdf:save-annotations
  ↓ PdfService.saveAnnotations() → savePDFAnnotations(filePath, annotations)
Service: pdf-viewer.ts savePDFAnnotations()
  ↓ fs.mkdir(.nabu/pdf-annotations) → fs.writeFile(JSON.stringify, null, 2)
Persistence: written to disk
```

**Files:**
- `PdfViewer.tsx` lines 130–142 (save effect), 179–212 (handlePageMouseUp)
- `pdf.ts` lines 93–105 (`PDF_SAVE_ANNOTATIONS`)
- `pdf-service.ts` lines 151–170 (`saveAnnotations`)
- `pdf-viewer.ts` lines 242–254 (`savePDFAnnotations`)

### 3.3 Annotation → Note Link
```
User clicks "Create note" on annotation
  ↓ PdfViewer.createNoteFromAnnotation() → pdfCommands.createNoteFromAnnotation(filePath, annotation)
Command: builds markdown body + YAML frontmatter with [[pdf.pdf#page=N]] link
  ↓ ipc.note.create('', title, frontmatter + body)
IPC: note:create
  ↓ Returns linked note path
Interaction: setAnnotations with linkedNotePath updated
```

**Files:**
- `PdfViewer.tsx` lines 235–245, 471–481
- `pdfCommands.ts` lines 35–61 (`createNoteFromAnnotation`)

---

## 4. Navigation

| Control | Handler | Effect |
|---------|---------|--------|
| Previous page | `goToPreviousPage` | `setCurrentPage(max(1, prev-1))` |
| Next page | `goToNextPage` | `setCurrentPage(min(total, prev+1))` |
| Zoom in | `zoomIn` | `setScale(min(2.0, round((prev+0.1)*10)/10))` |
| Zoom out | `zoomOut` | `setScale(max(0.5, round((prev-0.1)*10)/10))` |
| Reset zoom | `resetZoom` | `setScale(1.0)` |
| Initial page | `useEffect` on `totalPages` | Navigate to `initialPage` prop (Req 40.8) |

**Annotation navigation:** Wiki-links support `[[pdf.pdf#page=N]]` form (`WikiLink.pageRef`
in `src/shared/types.ts` line 27) — enables deep-linking from notes to PDF pages.

---

## 5. Persistence

### 5.1 Annotation Storage
- **Path:** `<vault>/.nabu/pdf-annotations/<pdfName>.json`
  - `pdfName = path.basename(filePath, '.pdf')` (pdf-viewer.ts line 213)
  - Resolved via `vaultRegistry.getActive().vaultPath` (line 209)
- **Format:** JSON array of `PDFAnnotation` objects
- **Write:** `fs.writeFile(path, JSON.stringify(annotations, null, 2), 'utf-8')`
- **Read:** `fs.readFile` → `JSON.parse`; `[]` on `ENOENT`
- **Directory:** Created on save via `fs.mkdir(dir, { recursive: true })`

### 5.2 PDF Document Cache
- `clearPDFCache()` / `clearAllPDFCache()` are **no-ops** — pdfjs-dist manages
  document lifecycle internally (pdf-viewer.ts lines 196–198, 260–265)
- **Renderer raster cache:** `pageImages` React state (base64 PNG data URIs)

---

## 6. IPC Channels

| Channel | Direction | Handler | Service |
|---------|-----------|---------|---------|
| `pdf:open` | Renderer → Main | `PDF_OPEN` | `PdfService.open` |
| `pdf:render-page` | Renderer → Main | `PDF_RENDER_PAGE` | `PdfService.renderPage` |
| `pdf:load-annotations` | Renderer → Main | `PDF_LOAD_ANNOTATIONS` | `PdfService.loadAnnotations` |
| `pdf:save-annotations` | Renderer → Main | `PDF_SAVE_ANNOTATIONS` | `PdfService.saveAnnotations` |

**Preload API:** `window.electron.pdf.{open, renderPage, loadAnnotations, saveAnnotations}`
(`src/preload/index.ts` lines 66–84)

---

## 7. Services & Engine

### 7.1 PdfService (`pdf-service.ts`)
- Thin orchestration layer over `pdf-viewer.ts` engine functions
- Applies Zod validation (`PDFOpenSchema`, etc.) + error normalization
- No behavior change from original IPC handlers (pure extraction)

### 7.2 PdfViewer Engine (`pdf-viewer.ts`)
- **PDF.js:** Dynamically imported (`initPDFJS()`) to avoid DOMMatrix at load (lines 35–41)
- **Canvas:** Dynamically imported (`initCanvas()`) for Node.js rasterization (lines 16–22)
- **Functions:**
  - `getPDFInfo()` — metadata + page count
  - `extractPDFText()` — full-text extraction per page (lines 107–126)
  - `renderPDFPage()` — single page → base64 PNG
  - `loadPDFAnnotations()` / `savePDFAnnotations()` — JSON persistence
- **Text extraction:** Available but not wired to viewer UI (used by importer/search)

### 7.3 PDF Importer (`pdf-importer.ts`)
- Imports PDF content as markdown/note (separate from viewer)
- Uses `extractPDFText()` for content extraction

---

## 8. Ownership

| Component | Owning Feature | Owning Service | Renderer Owner | IPC Owner | Persistence Owner |
|-----------|---------------|----------------|---------------|-----------|-------------------|
| PDF open | PDF | `PdfService` → `pdf-viewer` | `PdfViewer` | `pdf.ts` | n/a (read-only) |
| PDF render | PDF | `PdfService` → `pdf-viewer` | `PdfViewer` | `pdf.ts` | n/a (ephemeral) |
| Annotations load | PDF | `PdfService` → `pdf-viewer` | `PdfViewer` | `pdf.ts` | `pdf-viewer.ts` |
| Annotations save | PDF | `PdfService` → `pdf-viewer` | `PdfViewer` | `pdf.ts` | `pdf-viewer.ts` |
| Annotation→Note | PDF | `pdfCommands` (renderer) | `pdfCommands` | `notes.ts` | `notes.ts` |
| Navigation | PDF | n/a | `PdfViewer` | n/a | n/a |
| Engine | PDF | `pdf-viewer.ts` | n/a | n/a | `pdf-viewer.ts` |

---

## 9. Supporting Services

| Service | Role |
|---------|------|
| `PdfService` | Validation + orchestration for all PDF IPC |
| `pdf-viewer.ts` | pdfjs-dist + canvas rendering engine |
| `vaultRegistry` | Resolves active vault path for annotation storage |
| `pdf-importer.ts` | PDF → note import using text extraction |
| `pdfCommands.ts` | Renderer-side annotation→note workflow orchestration |

---

*End of PDF Map — Phase 7.1 Prompt A. No production code modified.*
