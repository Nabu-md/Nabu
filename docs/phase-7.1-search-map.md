# Phase 7.1 — Search Map

**Nabu Recovery Program — Storage, Search & PDF Inventory (Prompt A)**

This document inventories every search-related subsystem: indexing, query execution,
ranking, filtering, tokenization, fuzzy search, metadata search, and incremental indexing.
No production code was modified during this discovery phase.

---

## 1. Search Subsystems Inventory

| # | Subsystem | Type | Location | Owner |
|---|-----------|------|----------|-------|
| 1 | Full-text index | In-memory inverted index | `src/shared/indexing.ts` | `state.ts` |
| 2 | Tag index | In-memory inverted index | `src/shared/indexing.ts` | `state.ts` |
| 3 | Extended search index | In-memory multi-map | `src/shared/extended-indexing.ts` | `state.ts` |
| 4 | Knowledge graph edges | In-memory edge list | `src/shared/graph.ts` | `state.ts` |
| 5 | Query parser/executor | Pure function | `src/shared/search-query.ts` | `search-service.ts` |
| 6 | Vector (semantic) index | On-disk Vectra | `src/main/services/vector.ts` | `vector.ts` |
| 7 | Fuzzy matcher | Pure function | `src/renderer/src/features/search/fuzzy.ts` | Renderer (CommandPalette) |
| 8 | Incremental indexing | In-memory update | `state.ts` + `extended-indexing.ts` | `state.ts` |
| 9 | Tokenization | Pure helper | `indexing.ts` / `extended-indexing.ts` | Shared |
| 10 | IPC search channels | IPC handlers | `src/main/ipc/search.ts` | `search.ts` IPC |

---

## 2. Indexing Paths

### 2.1 Full-Text Index (`buildFullTextIndex`)
- **File:** `src/shared/indexing.ts` (lines 36–77)
- **Input:** `FileEntry[]` + `getAST(path)` callback
- **Tokenization:** Lower-case, split on `/\s\p{P}+/u` (whitespace + Unicode punctuation)
- **Excludes:** `yaml` / `toml` frontmatter nodes (SKIP)
- **Output:** `Map<word, Set<filePath>>`
- **Storage:** In-memory only (`StateManager.fullTextIndex`)
- **Triggered by:** `StateManager.buildIndexes()` / `updateIndexesForFile()`

### 2.2 Tag Index (`buildTagIndex`)
- **File:** `src/shared/indexing.ts` (lines 96–128)
- **Source:** First `yaml` frontmatter node, `tags:` field
- **Formats:** Inline array `tags: [a, b]` or block list `tags:\n  - a`
- **Output:** `Map<tag, Set<filePath>>`
- **Storage:** In-memory (`StateManager.tagIndex`)

### 2.3 Extended Search Index (`buildExtendedIndex`)
- **File:** `src/shared/extended-indexing.ts` (lines 90–104)
- **Components:**
  - `positions`: `Map<word, Map<filePath, number[]>>` — token line numbers
  - `lineSnippets`: `Map<filePath, string[]>` — per-line truncated text (120 chars)
  - `tagIndex`: `Map<tag, Set<filePath>>` — frontmatter + inline `#tags`
  - `aliasIndex`: `Map<lowercaseAlias, string[]>` — note aliases
  - `propertyIndex`: `Map<propName, Map<value, Set<filePath>>>` — frontmatter props
  - `blockRefs`: `Map<filePath, Map<blockId, nodeKey>>` — `^block-id` refs
- **Inline tag regex:** `/(?<!\w)(#[\p{L}\p{N}_/\-]+)(?!\w)/gu` (exported `INLINE_TAG_RE`)
- **Namespaced tags:** Parent segments indexed (e.g. `parent/child` → also `parent`)
- **Storage:** In-memory (`StateManager.extendedIndex`)

### 2.4 Knowledge Graph (`buildGraph`)
- **File:** `src/shared/graph.ts` (lines 28–71)
- **Source:** `wikiLink` AST nodes
- **Resolution:** Case-insensitive basename match → fallback to `aliasIndex`
- **Output:** `Edge[]` (`{source, target, snippet}`)
- **Snippet:** First paragraph truncated to 80 chars (populated in `state.ts` buildIndexes)
- **Storage:** In-memory, sent to renderer via `INDEX_BUILD` channel

### 2.5 Vector (Semantic) Index
- **File:** `src/main/services/vector.ts`
- **Engine:** Vectra `LocalIndex` + BGE-micro ONNX embeddings (384-dim)
- **Incremental:** `embedFile()` queues single file through `AsyncQueue` (lines 157–161)
- **Removal:** `removeFile()` on vault file delete (lines 172–179)
- **Reindex:** `reindexAll()` reads all vault files and re-embeds (lines 215–232)
- **Storage:** On-disk `<vault>/.nabu/vectra/`
- **Query:** `search(text, limit, excludePath)` cosine similarity (lines 243–272)

---

## 3. Query Execution Flow

### 3.1 Text Search (`search:query` channel)
```
User Query (string)
  ↓ IPC: search:query (preload → ipcMain)
Query Processing: SearchService.query() → search()
  ↓ parseQuery() → ParsedQuery {path?, tag?, line?, content?, file?, property?, regex?, bareTerms[]}
Index Lookup: getCandidatesByIndexChecks()
  ↓ Intersects: tagIndex, path filter, file filter, propertyIndex, positions (bare terms)
Ranking: filterBySnippetScan() scores by match count
  ↓ line:/content:/regex substring + bare-term position scan
Filtering: AND-combined operators; snippet scan for text queries
  ↓ sort by score desc, path tie-break
Result Delivery: SearchResponseSchema → renderer CommandPalette/SearchView
```

**Files:**
- `src/shared/search-query.ts` — `parseQuery` (101–153), `executeQuery` (171–234), `search` (239–248)
- `src/main/services/search-service.ts` — `query()` (43–72)
- `src/main/ipc/search.ts` — `SEARCH_QUERY` handler (143–145)

### 3.2 Semantic Search (`context:query` channel)
```
User Text (query string)
  ↓ IPC: context:query
Query Processing: vectorManager.getStatus() (disabled check)
  ↓ vectorManager.search(text, 5, excludePath)
Index Lookup: Vectra queryItems(embedding, text, limit+1)
  ↓ generateEmbedding() via BGE-micro
Ranking: Cosine similarity, score rounded 2 decimals
  ↓ exclude path, slice to limit
Filtering: status.disabled → empty; status.items === 0 → empty
Result Delivery: ContextSearchResultSchema → renderer
```

**Files:** `src/main/ipc/search.ts` (35–79), `src/main/services/vector.ts` (243–272)

### 3.3 Fuzzy Search (Renderer-side)
```
User Query (string)
  ↓ fuzzySearch(query, items, opts)
Query Processing: matchScore() per field (name/path/alias/keyword)
  ↓ sequential bonus, word-boundary bonus, leading bonus
Ranking: Weighted by field (NAME=3, PATH=1, ALIAS=0.8, KEYWORD=0.6)
  ↓ sort by score desc, name tie-break
Filtering: threshold + maxResults options
Result Delivery: FuzzyMatch[] → CommandPalette / QuickSwitcher
```

**Files:** `src/renderer/src/features/search/fuzzy.ts` (209–301)

---

## 4. Tokenization

| Subsystem | Tokenizer | Normalization |
|-----------|-----------|---------------|
| Full-text | `value.toLowerCase().split(/[\s\p{P}]+/u)` | Lower-case, strip punctuation |
| Extended positions | `tokenize(value)` (extended-indexing.ts) | Lower-case |
| Inline tags | `INLINE_TAG_RE` regex | Strip `#`, require letter/digit |
| Tag index | YAML `tags:` parse | Trim whitespace |
| Vector | BGE-micro tokenizer (ONNX) | Model-internal |
| Fuzzy | Char-by-char match | Lower-case compare |

---

## 5. Ranking & Filtering

### 5.1 Text Search Ranking
- **Score** = number of matching lines (`matches.length`) in `filterBySnippetScan`
- **Sort:** `score desc`, then `filePath.localeCompare` tie-break
- **Filtering:** All operators AND-combined; bare terms must all appear (intersection)

### 5.2 Semantic Ranking
- **Score** = cosine similarity × 100, rounded to 2 decimals
- **Token count** = `charCount / 4` (approx)
- **Filter:** Excludes `excludePath`; returns up to `limit` results

### 5.3 Fuzzy Ranking
- **Score** = `bestFieldScore × fieldWeight`, clamped [0,1]
- **Bonuses:** sequential (+0.15), word-boundary (+0.2), leading (+0.3), proximity
- **Sort:** `score desc`, then `name.localeCompare`

### 5.4 Property Promotion
- `promoteBarePropertyTerms()` (search-query.ts 265–289): bare `key:value` matching known
  frontmatter property is promoted to `property:` filter (Req 13.2)

---

## 6. Incremental Indexing

| Trigger | Action | File |
|---------|--------|------|
| `note:save` | `updateIndexesForFile()` re-indexes single file | `notes.ts` → `state.ts` |
| `note:delete` | `buildIndexes()` full rebuild | `notes.ts` → `state.ts` |
| External edit (watcher) | `invalidateAST` → re-parse on next access | `watcher.ts` → `state.ts` |
| File added (watcher) | `embedFile()` for vector | `shared.ts` → `vector.ts` |
| File deleted (watcher) | `removeFile()` from vector | `shared.ts` → `vector.ts` |
| OCR companion note | `updateIndexesForFile()` | `shared.ts` → `state.ts` |

**Incremental update logic** (`state.ts` `updateIndexesForFile`, lines 295–384):
1. Invalidate + re-parse AST for changed file
2. Remove all index entries for `filePath` from `fullTextIndex` / `tagIndex`
3. Re-index single file, merge into stored Maps
4. `updateExtendedIndexForFile()` incremental extended index update
5. Rebuild edges (alias-aware), refresh snippets for changed file
6. Serialize Maps → Records for IPC transport

---

## 7. Caches

| Cache | Owner | Scope | Invalidation |
|-------|-------|-------|--------------|
| `astStore` | `state.ts` | Per vault session | `invalidateAST()` on external edit / save |
| `fullTextIndex` | `state.ts` | Per vault session | Incremental update |
| `tagIndex` | `state.ts` | Per vault session | Incremental update |
| `extendedIndex` | `state.ts` | Per vault session | Incremental update |
| Vectra index | `vector.ts` | On-disk per vault | `reindexAll()` / `removeFile()` |
| `viewStateCache` | `view-state.ts` | In-memory | `clearViewStateCache()` on vault switch |

---

## 8. Ownership

| Component | Owning Feature | Owning Service | IPC Owner | Renderer Owner |
|-----------|---------------|----------------|-----------|----------------|
| Full-text index | Search | `state.ts` | `search.ts` | CommandPalette |
| Tag index | Search | `state.ts` | `search.ts` | CommandPalette |
| Extended index | Search | `state.ts` | `search.ts` | CommandPalette |
| Graph edges | Graph | `state.ts` | `search.ts` (INDEX_BUILD) | GraphView |
| Query executor | Search | `search-service.ts` | `search.ts` | CommandPalette |
| Vector index | Semantic | `vector.ts` | `search.ts` | ContextPane |
| Fuzzy matcher | Quick Switcher | n/a (renderer) | n/a | CommandPalette |
| Incremental indexing | Search | `state.ts` | `notes.ts` | NoteView |

---

## 9. Supporting Services

| Service | Role |
|---------|------|
| `StateManager` | Owns all in-memory indexes + AST store |
| `SearchService` | Orchestrates `search:query` execution |
| `VectorManager` | Owns semantic embeddings + Vectra index |
| `VaultWatcher` | Triggers incremental re-index on file changes |
| `Parser` (`parser.ts`) | Produces mdast ASTs consumed by indexes |
| `buildGraph` | Produces knowledge-graph edges |
| `fuzzy.ts` | Renderer-side fuzzy ranking for navigation |

---

*End of Search Map — Phase 7.1 Prompt A. No production code modified.*
