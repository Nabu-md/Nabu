# Types

Types describe what kind of thing a note represents: Project, Person, Topic, Procedure, Event, or any category you create.

## Type Field

The `type:` field assigns a note to a type.

```yaml
type: Project
```

Nabu does not infer type from folder location. Moving a file into another folder does not change its type.

## Prefer Types Over Folders

Types are the preferred way to group notes in Nabu. Folders are supported for existing vaults and fallback organization, but Nabu is built around types and relationships because they carry stronger meaning than file paths.

Use types for semantic groups such as Projects, People, Topics, Procedures, Events, and Essays. Use relationships to connect notes across those groups. This gives Nabu better structure for navigation, filtering, properties, templates, and future automation than folder location alone.

## Type Documents

Type documents are Markdown notes with `type: Type` in frontmatter. They describe how a type should appear and what new notes of that type should start with.
Use Phosphor icon names in kebab-case for `_icon`, such as `folder` or `briefcase`.

```yaml
---
type: Type
_icon: folder
_color: blue
_sidebar_label: Projects
_order: 10
---

# Project
```

Type templates can live in the Type document's `template` frontmatter field. When a hand-edited Type body contains template-like structure after its own `# TypeName` heading, Nabu also uses that body content as the new-note template. Plain descriptive body text stays documentation-only.

## What Types Control

- Sidebar grouping.
- Type icon and color.
- Sidebar order and label.
- Pinned properties.
- New-note templates.

## New Note Defaults

Type documents can define empty properties and relationships. When you create a new note of that type, Nabu shows placeholders for those fields so you can fill them in from the Properties panel.

If a type document gives a property a value, that value becomes the default for new notes of that type. For example, a Project type can define `status: Active` so every new project starts active until you change it.
