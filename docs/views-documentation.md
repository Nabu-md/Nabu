# Nabu Views Documentation

## Overview

This document describes the production-ready UI views implemented as projections on top of `KnowledgeObject` entities. Views never own data — they project existing `KnowledgeObject` instances from the storage layer.

## Reading Queue

**Component**: `crates/nabu-ui/src/components/reading_queue.rs`

### Features
- **Status Management**: Unread, Reading, Completed, Archived
- **Priority Levels**: Low, Normal, High
- **Progress Tracking**: 0–100% progress per item
- **Filtering**: By status, priority, and free-text search
- **Sorting**: By date, title, priority, progress
- **Batch Actions**: Mark as Reading, Mark as Done, Archive
- **Event-Driven**: Uses Tauri IPC (no polling)

### Tauri Commands
| Command | Description |
|---------|-------------|
| `queue_get_all` | Fetch all KnowledgeObjects with reading metadata |
| `queue_set_status` | Update reading status for an item |
| `queue_set_priority` | Update priority for an item |
| `queue_set_progress` | Update progress (0.0–1.0) for an item |
| `queue_batch_set_status` | Batch update status for selected items |
| `queue_archive_completed` | Archive all completed items |

### Data Flow
```
StorageManager → KnowledgeObject → ReadingMetadata (in custom metadata)
→ Tauri IPC → ReadingQueue UI (Leptos component)
```

## Property Editor

**Component**: `crates/nabu-ui/src/components/property_editor.rs`

### Supported Field Types
| Type | Input | Validation |
|------|-------|------------|
| Text | `<input type="text">` | Always valid |
| Number | `<input type="number">` | Must parse as `f64` |
| Date | `<input type="date">` | Must be valid date string |
| Select | `<select>` dropdown | Must be one of `options` |
| MultiSelect | Toggle buttons | All values must be in `options` |
| URL | `<input type="url">` | Must start with `http://`, `https://`, or `mailto:` |

### Features
- Real-time validation with `ValidationState` (Valid/Invalid)
- Autocomplete for URL fields (browser-native)
- Per-field validation callbacks
- Unit tests for all property type validations

### Data Flow
```
KnowledgeObject.metadata.custom → PropertyDefinition + PropertyValue
→ PropertyEditor → PropertyField (per type) → Validation
```

## Relation Editor

**Component**: `crates/nabu-ui/src/components/relation_editor.rs`

### Features
- **Existing Relations Tab**: View and remove existing graph edges
- **Search Tab**: Autocomplete entity search by title and type
- **Create Entity Tab**: Create new entities with relationship type
- **Relationship Picker**: BelongsTo, WorksOn, RelatedTo, CreatedBy, References, MemberOf, DependsOn
- **Semantic Edge Editing**: Add/remove typed relationships

### Tauri Commands
| Command | Description |
|---------|-------------|
| (Uses VaultGraph directly) | Entity lookup and relation management |

### Data Flow
```
VaultGraph → GraphEdge → RelationEditor UI
→ on_add_relation / on_remove_relation / on_create_entity callbacks
```

## Collection Views

### Table View
**Component**: `crates/nabu-ui/src/components/collections/table_view.rs`

- Column configuration with visibility, sortability, and width
- Filtering by search query and object type
- Sorting by any column
- Grouping support (via `group_by` filter)

### Board View
**Component**: `crates/nabu-ui/src/components/collections/board_view.rs`

- Kanban-style columns with drag-and-drop
- Auto-generated columns from grouped data or custom column definitions
- Filtering by search query and object type
- Grouping by status, type, or priority

### Gallery View
**Component**: `crates/nabu-ui/src/components/collections/gallery_view.rs`

- Card-based layout with responsive grid
- Filtering by search query and object type
- Sorting by title, type, modified date, or created date
- Hover effects and type badges

### Calendar View
**Component**: `crates/nabu-ui/src/components/collections/calendar_view.rs`

- Date-based grid layout
- Month, Week, and Day view modes
- Filtering by search query and object type
- Items grouped by creation date
- Day cells show item count and truncated titles

### Collection Container
**Component**: `crates/nabu-ui/src/components/collections/container.rs`

- View switcher (Table, Board, Gallery, Calendar)
- Shared search state
- Loads objects via Tauri IPC (`fetch_objects`)
- Passes data to all view components

## Templates

**Component**: `crates/nabu-ui/src/components/template_editor.rs`

### Features
- **Template List**: Browse, search, edit, and delete templates
- **Create Template**: Name, description, default folder, Markdown body
- **Edit Template**: Modify name, description, folder, and body
- **Assign to Folder**: Per-folder template assignment with toggle
- **Property Presets**: Templates can define default property values

### Template Model
```rust
pub struct Template {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub default_folder: Option<String>,
    pub frontmatter_defaults: HashMap<String, String>,
    pub property_presets: HashMap<String, PropertyValue>,
    pub body: String,
    pub object_type: Option<String>,
}
```

### Data Flow
```
TemplateManager → Template → TemplateEditor UI
→ on_save / on_delete / on_assign / on_unassign callbacks
```

## Architecture Principles

### Views Are Projections
All views project existing `KnowledgeObject` entities from the storage layer. Views never own data.

### No Duplicate Storage
Views read from the same `StorageManager` / `VaultGraph` / `Tantivy` sources used by the rest of the application. No separate databases or storage backends are introduced.

### Markdown Remains Canonical
All views display and interact with `KnowledgeObject` entities. The underlying storage remains Markdown files in the vault directory, with metadata in SQLite and search index in Tantivy.

### Event-Driven Updates
Views subscribe to data changes via Tauri IPC commands. No polling is used.