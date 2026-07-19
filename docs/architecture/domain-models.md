# Nabu Domain Models

> **Status:** Phase 1.1 — Design (documentation only)
> **Target location:** `src/shared/models/`
> **Companion:** [architecture.md](./architecture.md)

Domain models represent **business concepts only** (Architecture Goal 3). They are the innermost dependency ring (Architecture Goal 8) and are imported by both the main and renderer layers.

## Global Layer Restrictions (apply to every model)

Domain models **must never** depend on:

- **Electron** (no `electron`, no `ipcMain`/`ipcRenderer`, no `BrowserWindow`)
- **React** (no components, hooks, JSX, or React types)
- **Browser APIs** (no `window`, `document`, `DOM`, `fetch`)
- **Node APIs** (no `fs`, `path`, `process`, `Buffer`)

Domain models **may** depend on:

- Other domain models in `src/shared/models/`
- Type-only / pure libraries (e.g., `zod` for schema-derived types, `mdast` type-only imports)

Models are **plain data shapes** (interfaces/types). Behavior belongs in services (Architecture Goal 2), not in models.

---

## Model Index

| Model | Responsibility | Owner |
| --- | --- | --- |
| [`Note`](#note) | A single markdown document in a vault | `shared/models/Note.ts` |
| [`Vault`](#vault) | A root directory of notes and its metadata | `shared/models/Vault.ts` |
| [`Workspace`](#workspace) | The set of open vaults and active view state | `shared/models/Workspace.ts` |
| [`Tag`](#tag) | A frontmatter/inline tag and its usage | `shared/models/Tag.ts` |
| [`GraphNode`](#graphnode) | A node in the note-link graph | `shared/models/GraphNode.ts` |
| [`Attachment`](#attachment) | A non-note file (image, PDF, audio) referenced by notes | `shared/models/Attachment.ts` |

---

## Note

**Responsibility:** Represents a single markdown document in a vault — its identity, metadata, and derived properties. It is the primary content unit of the application.

**Ownership:** `src/shared/models/Note.ts`. Consumed by `NoteService`, `SearchService`, `IndexService` (main) and the `notes` feature (renderer).

**Layer restrictions:** Pure. No `Root`/AST runtime dependency — the parsed AST is a *derived* artifact held by services, not part of the persisted domain model. If AST typing is needed, it is imported **type-only** from `mdast`.

**Primary fields:**

| Field | Type | Description |
| --- | --- | --- |
| `id` | `string` | Stable identifier (vault-relative path or hash) |
| `path` | `string` | Vault-relative file path (e.g., `folder/note.md`) |
| `title` | `string` | Display title (frontmatter `title` or filename) |
| `frontmatter` | `Record<string, unknown>` | Parsed YAML frontmatter properties |
| `tags` | `string[]` | Tag names referenced by this note |
| `links` | `string[]` | Wiki-link targets found in this note |
| `mtime` | `number` | Last-modified timestamp (epoch ms) |
| `hash` | `string` | Content hash for change detection |
| `wordCount` | `number` | Derived word count |

**Relationships:**

- Belongs to one **Vault**.
- References many **Tag**s (by name).
- Links to many other **Note**s (via `links` → wiki-link targets).
- May reference many **Attachment**s.
- Maps 1:1 to a **GraphNode** in the link graph.

**Dependencies:** `Tag` (by name), optionally type-only `mdast` types. No service/infra dependencies.

---

## Vault

**Responsibility:** Represents a user's vault — a root directory containing notes, attachments, and Nabu cache/config. Embodies Product Principle 2 (User Ownership): the vault is the user's data location.

**Ownership:** `src/shared/models/Vault.ts`. Consumed by `VaultService` (main) and the `vault` feature (renderer).

**Layer restrictions:** Pure. Holds the vault **path as a string only** — no `fs` handles, no Electron directory objects.

**Primary fields:**

| Field | Type | Description |
| --- | --- | --- |
| `id` | `string` | Stable vault identifier |
| `path` | `string` | Absolute root directory path |
| `name` | `string` | Display name (folder basename or user-set) |
| `noteCount` | `number` | Number of notes in the vault |
| `lastOpened` | `number` | Timestamp of last open (epoch ms) |
| `settings` | `VaultSettings` | Per-vault configuration snapshot |

**Relationships:**

- Contains many **Note**s.
- Contains many **Attachment**s.
- Belongs to a **Workspace** (as one of its open/recent vaults).

**Dependencies:** `VaultSettings` type (shared). No service/infra dependencies.

---

## Workspace

**Responsibility:** Represents the user's working session — which vaults are open/recent and the active navigation/view state. Coordinates *which vault is active* without owning UI layout details.

**Ownership:** `src/shared/models/Workspace.ts`. Consumed by `VaultService` and the app shell / `vault` feature.

**Layer restrictions:** Pure. Describes *what* is open, not *how* it is rendered. React/UI layout state stays in the renderer.

**Primary fields:**

| Field | Type | Description |
| --- | --- | --- |
| `activeVaultId` | `string \| null` | The currently focused vault |
| `openVaultIds` | `string[]` | Vaults open in the current session |
| `recentVaultIds` | `string[]` | Recently opened vaults (MRU order) |
| `activeNoteId` | `string \| null` | The currently active note |

**Relationships:**

- References many **Vault**s (open + recent, by id).
- References the active **Note** (by id).

**Dependencies:** References `Vault` and `Note` by id (no hard object coupling required). No service/infra dependencies.

---

## Tag

**Responsibility:** Represents a tag (from YAML frontmatter or inline `#tag`) and its aggregate usage across a vault.

**Ownership:** `src/shared/models/Tag.ts`. Consumed by `IndexService` (main) and the `search` / tags UI (renderer).

**Layer restrictions:** Pure. A tag is a name plus derived usage; no storage or UI concerns.

**Primary fields:**

| Field | Type | Description |
| --- | --- | --- |
| `name` | `string` | Normalized tag name (without leading `#`) |
| `count` | `number` | Number of notes using the tag |
| `noteIds` | `string[]` | Ids of notes that reference the tag |

**Relationships:**

- Referenced by many **Note**s.
- Aggregated per **Vault** by the index.

**Dependencies:** None (references notes by id). No service/infra dependencies.

---

## GraphNode

**Responsibility:** Represents a node in the note-link graph derived from wiki-link relationships. Used by the graph visualization feature.

**Ownership:** `src/shared/models/GraphNode.ts`. Produced by `IndexService`/graph builder (main, pure `shared` graph utilities) and consumed by the `graph` feature (renderer).

**Layer restrictions:** Pure. Layout coordinates (`x`, `y`, `vx`, `vy`) are *simulation* fields, not DOM/canvas objects — they carry no rendering dependency.

**Primary fields:**

| Field | Type | Description |
| --- | --- | --- |
| `id` | `string` | Matches the corresponding `Note.id` |
| `label` | `string` | Display label (note title) |
| `x` | `number` | Simulation x-coordinate |
| `y` | `number` | Simulation y-coordinate |
| `vx?` | `number` | Simulation x-velocity (optional) |
| `vy?` | `number` | Simulation y-velocity (optional) |

> A companion `GraphEdge` type (`source`, `target`, `snippet`) describes links between nodes and lives alongside `GraphNode`.

**Relationships:**

- Maps 1:1 to a **Note** (`GraphNode.id === Note.id`).
- Connected to other **GraphNode**s via `GraphEdge`s (derived from `Note.links`).

**Dependencies:** Conceptually tied to `Note` by id. No service/infra dependencies.

---

## Attachment

**Responsibility:** Represents a non-note file referenced by notes — images, PDFs, audio. Included because it is named in Architecture Goal 3 and clarifies ownership of binary assets.

**Ownership:** `src/shared/models/Attachment.ts`. Consumed by `PdfService`, `VaultService` (main) and the `pdf` / `notes` features (renderer).

**Layer restrictions:** Pure. Holds a path and metadata only — no `Buffer`, no file handle, no `fs`.

**Primary fields:**

| Field | Type | Description |
| --- | --- | --- |
| `id` | `string` | Stable identifier (vault-relative path or hash) |
| `path` | `string` | Vault-relative file path |
| `kind` | `'image' \| 'pdf' \| 'audio' \| 'other'` | Attachment type |
| `mtime` | `number` | Last-modified timestamp (epoch ms) |
| `sizeBytes` | `number` | File size in bytes |

**Relationships:**

- Belongs to one **Vault**.
- Referenced by many **Note**s.

**Dependencies:** None. No service/infra dependencies.

---

## Relationship Summary

```
Workspace ──references──▶ Vault (open / recent, by id)
Workspace ──references──▶ Note  (active, by id)

Vault ──contains──▶ Note
Vault ──contains──▶ Attachment

Note ──references──▶ Tag        (by name)
Note ──links──────▶ Note        (wiki-links)
Note ──references──▶ Attachment
Note ──maps 1:1───▶ GraphNode

Tag  ──used by────▶ Note

GraphNode ──edge──▶ GraphNode   (GraphEdge, derived from Note.links)
```

All relationships are expressed by **id/name references**, keeping each model small, serializable, and free of hard object coupling — consistent with Architecture Goal 3 (Small Domain Models) and Goal 8 (models import nothing but siblings).
