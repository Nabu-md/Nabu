# Open Or Create A Vault

A Nabu vault is a folder on disk. The folder can contain Markdown notes, attachments, type definitions, saved views, and Git metadata.

## Open An Existing Folder

Choose an existing folder if you already have Markdown notes. Nabu scans `.md` files and uses frontmatter when it exists.

Good starting points:

- A folder of plain Markdown files.
- An Obsidian-style vault.
- A Git repository containing notes.
- A copy of the Getting Started vault.

## Create A New Vault

Choose a new empty folder if you want Nabu conventions from the start. New notes and optional type definitions are created as Markdown files.

## Use More Than One Vault

You do not have to merge everything into one folder. Register each local folder as its own vault, then turn on `Use multiple vaults at the same time` in `Settings` -> `Vaults`.

Once enabled, the bottom-left vault menu lets you include vaults in the unified graph. Search, quick open, wikilinks, and note lists can span the included vaults, while Git sync and commits remain tied to each vault's own repository.

## Git Is Recommended, Not Required

Nabu works well with a plain folder of Markdown files. You can open, edit, organize, and search notes without making the vault a Git repository.

Git is recommended when you want local history, diff views, recovery, pull, push, and remote sync without a proprietary backend. If a vault is not already a repository, Nabu can initialize one when you explicitly ask it to.
