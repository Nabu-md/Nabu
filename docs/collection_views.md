# Collection Views System

Nabu now supports multiple interactive views over KnowledgeObjects.

## Features
- **Multiple Views**: Table, Board, Gallery, and Calendar views.
- **Shared State**: Search queries, filters, and sorting persist when switching views.
- **Filtering & Sorting**: Integrated with Tantivy for efficient querying.

## Implementation Details
- **UI Structure**: Views are located in `crates/nabu-ui/src/components/collections/`.
- **State Management**: Managed via `CollectionContainer` and `SearchState`.
- **Search Integration**: Extended `Indexer::search` to support `SearchQuery`.
