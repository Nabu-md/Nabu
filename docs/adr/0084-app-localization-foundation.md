---
type: ADR
id: "0084"
title: "App-owned localization foundation"
status: active
date: 2026-04-26
---

## Context

Nabu was effectively English-only. Users requested a general i18n foundation and Chinese-language support. We need a path that lets the UI adopt additional locales without pushing UI-language preferences into vault files or making every partially translated string a runtime failure.

## Decision

Nabu owns a dependency-free frontend localization layer in `src/lib/i18n.ts`.

- English is the canonical fallback locale.
- Simplified Chinese (`zh-Hans`) is the first additional locale.
- `ui_language` is an installation-local app setting in `~/.config/com.nabu.app/settings.json`; `null` means "follow system language when supported, otherwise English".
- Missing translation keys fall back to English.
- App-level chrome receives locale through props from `App.tsx`, following the existing props-down/callbacks-up architecture instead of introducing global React context.
- Language switching is exposed in Settings and through command-palette actions.

## Alternatives considered

- **Add an i18n dependency now**: useful long term, but unnecessary for the first locale and would add framework surface before we know Nabu's locale workflow.
- **Store language in the vault**: rejected because UI language is an installation preference, not content structure.
- **Translate ad hoc strings inline**: rejected because it would make fallback behavior inconsistent and future locales expensive.

## Consequences

- New UI strings should be added to `src/lib/i18n.ts` first and rendered through `translate()` / `createTranslator()` where the surface already receives locale.
- Partially translated locales remain usable because English is the fallback for missing keys.
- Locale choice changes UI chrome immediately after settings save or command-palette language commands without reopening the vault.
- Larger feature surfaces can migrate to the shared localization module incrementally.
