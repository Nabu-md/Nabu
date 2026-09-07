---
type: ADR
id: "0057"
title: "Alpha/stable release channels with PostHog beta cohorts"
status: superseded
date: 2026-04-12
superseded_by: "0066"
---

## Context

Nabu's updater and release docs still described a canary branch, a beta updater channel, and a single `latest.json` feed. That no longer matched the desired product model:

- `main` should continuously publish **alpha** builds.
- **Stable** should be promoted manually by pushing `stable-vX.Y.Z` tags.
- "Beta" users should be modeled in PostHog for targeting and analysis, not as a separate binary or updater feed.

The updater also needed semver-safe versioning when a user switches between Stable and Alpha. A date-based alpha version below the latest stable release would cause the updater to ignore newer alpha builds after a stable promotion.

This ADR supersedes ADR-0017's canary-branch updater model.

## Decision

**Nabu exposes exactly two updater channels: `stable` and `alpha`. Stable is the default feed, while every push to `main` publishes a prerelease alpha build to `alpha/latest.json`, and manually promoted `stable-vX.Y.Z` tags publish stable builds to `stable/latest.json`. Beta audiences are handled in PostHog and are not a third updater channel.**

## Options considered

- **Option A** (chosen): Two updater channels (`stable`, `alpha`) plus PostHog beta cohorts. Pros: matches the product requirement, keeps CI simple, keeps updater semantics understandable, and separates release distribution from experimentation audiences. Cons: requires semver-aware alpha versioning and a small migration for legacy channel settings.
- **Option B**: Keep the canary branch / canary channel model. Pros: no workflow redesign. Cons: no longer matches how releases are actually promoted and forces distribution strategy to depend on a long-lived branch.
- **Option C**: Add a third updater channel for beta builds. Pros: direct binary segmentation. Cons: extra CI complexity, extra updater endpoints, and unnecessary duplication because beta targeting is already better handled by PostHog.

## Consequences

- `release.yml` now publishes alpha prereleases from every push to `main`.
- `release-stable.yml` publishes stable releases only from `stable-v*` tags.
- `src-tauri/src/app_updater.rs` selects `alpha/latest.json` or `stable/latest.json` at runtime.
- `release_channel` stays an app setting, but only `alpha` is stored explicitly; Stable serializes to the default `null` value.
- Legacy or invalid persisted channel values fall back to Stable.
- Alpha versions are prereleases of the next stable patch version (for example `1.2.4-alpha.202604122135.7` after stable `1.2.3`) so semver ordering remains valid across channel switches.
- The legacy GitHub Pages aliases `latest.json` and `latest-canary.json` continue to mirror alpha for backward compatibility.
- Beta rollouts and internal-user targeting are done in PostHog using person properties or cohorts rather than updater manifests.

## Advice

If a future release process needs more than two binary distribution rings, re-evaluate this decision only when PostHog cohorting is no longer sufficient and the extra operational cost of another updater feed is justified.
