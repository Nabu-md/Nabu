# Custom Properties System

Nabu now supports user-defined custom properties at the vault level.

## Features
- **Vault-wide Definitions**: Define properties in the vault configuration.
- **Typed Values**: Properties are strongly typed (Text, Number, Date, Select, Multi-select, URL).
- **Sidebar Integration**: Edit properties directly from the note sidebar using the `PropertyEditor`.
- **Searchable**: Custom properties are indexed in Tantivy and are searchable.

## Implementation Details
- **Data Model**: `PropertyDefinition` and `PropertyValue` in `nabu-core`.
- **Storage**: Properties are stored within `ObjectMetadata::custom`.
- **Validation**: Strict validation is applied based on the property type and options.
