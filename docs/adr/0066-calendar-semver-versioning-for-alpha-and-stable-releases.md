---
type: ADR
id: "0066"
title: "Calendar-semver versioning for alpha and stable releases"
status: active
date: 2026-04-16
supersedes: "0057"
---

## Context

ADR-0057 kept Nabu on two updater channels and used "next stable patch" semver for alpha builds. That preserved ordering, but it no longer matched the agreed product naming:

- Alpha should display as `Alpha YYYY.M.D.N`
- Alpha should ship the technical version `YYYY.M.D-alpha.N`
- Stable should ship and display as `YYYY.M.D`

The naming change still needs to stay semver-safe when users switch between Stable and Alpha. A pure same-day calendar alpha would become older than a same-day stable promotion, so the workflow needs a monotonicity guard in addition to cleaner display strings.

## Decision

**Nabu keeps exactly two updater channels (`stable` and `alpha`), but both now use calendar-semver release numbers.** Stable promotions use `stable-vYYYY.M.D` tags and stamp the technical version `YYYY.M.D`. Every push to `main` publishes an alpha build with technical version `YYYY.M.D-alpha.N` and display label `Alpha YYYY.M.D.N`.

If the latest stable tag already uses the current UTC calendar date, the alpha workflow advances to the next calendar day before assigning `-alpha.N`. That keeps alpha semver-newer than the most recent stable build even after a same-day promotion.

## Options considered

- **Calendar semver with a next-day safeguard** (chosen): matches the agreed naming, keeps user-facing labels clean, and preserves updater ordering across channel switches.
- **Calendar semver without a safeguard**: simplest display model, but alpha can become semver-older than Stable after a same-day promotion.
- **Keep ADR-0057's next-patch prerelease numbering**: semver-safe, but it does not match the agreed release naming or the product surfaces that should show calendar-based versions.

## Consequences

- Release workflows now compute both a technical version and a display version.
- User-facing version surfaces strip technical prerelease noise into clean labels (`Alpha YYYY.M.D.N` or `YYYY.M.D`).
- Stable promotions must use `stable-vYYYY.M.D` tags instead of patch-based semver tags.
- Alpha sequence numbers are scoped to a calendar core date and remain compatible with the updater manifests.
