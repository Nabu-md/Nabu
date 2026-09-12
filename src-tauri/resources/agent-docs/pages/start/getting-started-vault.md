# Getting Started Vault

Source: start/getting-started-vault.md
URL: /start/getting-started-vault

# Getting Started Vault

The Getting Started vault is a small public sample vault hosted at [Nabu-md/nabu-getting-started](https://github.com/Nabu-md/nabu-getting-started).

It exists to show Nabu's conventions without requiring you to restructure your own notes first.

## What It Demonstrates

- Markdown notes with YAML frontmatter.
- Types such as Note, Project, Person, and Topic.
- Wikilinks in note bodies.
- Relationship fields in frontmatter.
- Rich-editor blocks, callouts, collapsible sections, and durable formatting.
- A spreadsheet with formulas and a cross-note frontmatter reference.
- An HTML dashboard powered by vault expressions.
- A standalone HTML report that can be previewed or edited in raw mode.
- A local Git repository that can be connected to a remote later.
- Vault guidance files plus example workflows for AI agents and direct models.

In the cloned vault, open **Editor Playground** for block editing, **Project Dashboard** for vault expressions, and **Sample Report** for standalone HTML preview.

## Local-Only By Default

When Nabu clones the sample, it removes the remote from the local copy. This makes the sample vault disposable. You can edit it freely, commit locally, and delete it later.

To connect a vault to your own remote, use the bottom status bar remote chip or run `Add Remote` from the command palette.

Nabu also repairs starter-vault guidance files when needed. `AGENTS.md` is the canonical guidance file, `CLAUDE.md` is kept as a compatibility shim, and `GEMINI.md` is only created when you explicitly restore Antigravity/Gemini guidance.

Start with the `Start here!` view and follow **Get familiar with Nabu**. The checklist links to working examples, so you can try each feature without changing your own notes.

## Use It Alongside Your Own Vaults

You can keep the Getting Started vault open while working in your own notes. Enable `Settings` -> `Vaults` -> `Use multiple vaults at the same time`, then use the bottom-left vault menu to include both the sample vault and your real vault in the unified graph.

This lets search, quick open, note lists, backlinks, and wikilink navigation span both vaults. Git actions still stay scoped to each vault's own repository, and new notes go to the default vault you choose in `Manage vaults`.

## When To Move On

After you understand the sample, open your own vault. Nabu does not require a special folder structure: a folder of Markdown files is enough to start. You can remove the sample from Nabu's vault list later without deleting its files from disk.