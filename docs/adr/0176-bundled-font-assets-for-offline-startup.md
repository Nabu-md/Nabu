# 0176. Bundled font assets for offline startup

Status: active

Date: 2026-08-14

## Context

The HTML bootstrap loaded Inter, IBM Plex Mono, and JetBrains Mono through a render-blocking Google Fonts stylesheet. Offline, air-gapped, and firewall-blocked desktop launches could wait on that request before painting Nabu's startup shell, breaking the product's offline-first contract.

System-font fallbacks would remove the network dependency but would also change typography between machines and between online and offline sessions.

## Decision

Bundle the existing font families with the renderer through Fontsource packages. Load variable Inter and JetBrains Mono weight assets plus IBM Plex Mono weights 400, 500, and 600 from `src/main.tsx`, and remove Google Fonts from the HTML bootstrap and both Tauri content-security policies.

## Consequences

The startup document has no external font or stylesheet request, so font endpoint failures cannot delay its first paint. Online and offline sessions use the same typography, and the production CSP no longer grants Google Fonts access.

The application bundle grows by the local WOFF2 assets and Fontsource packages must be kept current with normal dependency maintenance.
