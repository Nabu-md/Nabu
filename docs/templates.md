# Template Management System

Nabu now supports advanced, reusable object templates that integrate seamlessly with metadata and custom properties.

## Features
- **Object Templates**: Define both document structure (Markdown) and metadata defaults.
- **Per-Folder Defaults**: Configure preferred templates for specific folders via Vault Settings.
- **Template Picker**: Easily browse, search, and preview templates during note creation.
- **Template Editor**: Create and modify template definitions directly.

## Implementation Details
- **Data Model**: `Template` struct in `nabu-core`.
- **Storage**: Templates are stored as Markdown files in `.nabu/templates/` with YAML frontmatter for metadata.
- **Legacy Compatibility**: Automatically supports existing template files without YAML frontmatter.
