# Use Wikilinks

Source: guides/use-wikilinks.md
URL: /guides/use-wikilinks

# Use Wikilinks

Wikilinks connect notes by name.

```md
This project belongs to [[content-systems]] and is related to [[git-workflows]].
```

## Link From The Body

Use body links when the connection is part of the sentence you are writing.

## Link From Frontmatter

Use frontmatter links when the relationship should become structured metadata.

```yaml
related_to:
  - "[[git-workflows]]"
```

## Keep Links Stable

Prefer clear note titles and filenames. Nabu's wikilink autocomplete helps you pick the right target while you type.