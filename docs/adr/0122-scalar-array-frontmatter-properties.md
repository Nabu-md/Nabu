# 0122. Scalar Array Frontmatter Properties

Status: active

Date: 2026-05-15

## Context

Saved Views filter against `VaultEntry.properties` in the renderer and in the Rust view evaluator. Before this decision, Nabu preserved custom scalar frontmatter values as properties but dropped multi-element non-wikilink arrays during a full vault scan. That made a view such as `tags / contains / blues` unstable: optimistic renderer state could see a changed array for a while, but reload, view switch, or restart rebuilt the entry without the array-backed property.

Relationship arrays already have separate semantics because wikilink-bearing fields are stored in `VaultEntry.relationships`. Plain scalar arrays need to stay queryable as custom properties without being treated as relationship fields.

## Decision

**Nabu preserves custom scalar-array frontmatter fields in `VaultEntry.properties`, while wikilink-bearing arrays remain relationships.** Single-item scalar arrays continue to normalize to a scalar value for compatibility; multi-item scalar arrays remain arrays.

Saved View filters evaluate scalar-array properties with set semantics. `contains` and `any_of` match exact case-insensitive elements, not substrings inside an element. Scalar properties keep their existing case-insensitive text matching behavior.

The vault cache version is bumped so existing caches that dropped array properties are rebuilt from disk.

## Options considered

- **Option A (chosen): Preserve scalar arrays as properties** - keeps YAML frontmatter expressive, fixes reload/restart behavior, and avoids hardcoded fields such as `tags`. The cost is widening `VaultEntry.properties` from scalar-only to scalar-or-array.
- **Option B: Flatten arrays to comma-delimited strings** - keeps the old property type, but cannot distinguish exact elements from substrings and makes filters ambiguous.
- **Option C: Treat every array as a relationship** - reuses existing relationship matching, but non-wikilink values such as tags are not graph edges and should not appear as relationships.

## Consequences

Views can filter custom scalar arrays consistently across save, reload, view switches, and app restart.

Property chips and sorting must tolerate property arrays. The note-list chip resolver already expands array values; custom-property sorting falls back to string comparison for arrays.

Any future custom-property logic must handle `VaultPropertyValue` rather than assuming every property is a scalar.
