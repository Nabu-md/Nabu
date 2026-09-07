# Docs Maintenance

Source: reference/docs-maintenance.md
URL: /reference/docs-maintenance

# Docs Maintenance

The public docs live in the app repo so documentation changes can ship with behavior changes.

## Update Docs When You Change

- A Tauri command.
- A new component or hook that changes user behavior.
- A data model or frontmatter convention.
- Git, AI, onboarding, or release behavior.
- Public release pages, download metadata, or updater channels.
- Platform support.
- Keyboard shortcuts.
- Stable release highlights.
- Examples in the Getting Started vault.

## Suggested Workflow

1. Make the code change.
2. Update the matching concept, guide, or reference page.
3. Add a troubleshooting page if the change creates a new failure mode.
4. Run `pnpm docs:build`.
5. Check the home page, search, release/download links, and changed docs pages in a browser.

## Release Communication Check

For each stable release:

1. Use the public release history as the source of truth for what shipped.
2. Map every user-facing highlight to a concept, guide, reference, or troubleshooting page.
3. Add or refresh a Getting Started vault example when the feature is best learned by opening a real file.
4. Review the landing page separately and promote only the capabilities that explain Nabu's durable value.
5. Remove instructions for features that were withheld from the final release.

## Page Types

| Type | Purpose |
| --- | --- |
| Start | Helps a new user get into the app. |
| Concepts | Explains mental models. |
| Guides | Teaches workflows. |
| Reference | Gives stable facts and tables. |
| Troubleshooting | Starts from a symptom and ends with recovery. |

## Review Checklist

- Does the page describe current behavior?
- Does it mention macOS primary and Windows/Linux supported-early status when platform support matters?
- Are links relative and VitePress-compatible?
- Can a user discover the page with local search?
- Does the Getting Started vault demonstrate the workflow when an example would be clearer than prose?