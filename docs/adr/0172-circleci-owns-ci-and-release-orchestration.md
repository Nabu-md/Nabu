# ADR 0172: CircleCI Owns CI and Release Orchestration

## Status

Accepted

## Context

Nabu previously split validation between GitHub Actions and CircleCI Chunk sidecars. The
sidecars already provided the faster inner-loop execution model, while GitHub Actions repeated
dependency setup and separately orchestrated frontend, Rust, Playwright, documentation, and
four cross-platform release builds.

The duplicated orchestration made performance tuning harder and left the environment used before
push different from the authoritative outer-loop pipeline. Release behavior was also distributed
across alpha, stable, shared-artifact, documentation, and pull-request maintenance workflows.

## Decision

CircleCI is the sole custom CI/CD orchestrator for Nabu. A single `.circleci/config.yml` owns:

- frontend, Rust, CodeScene, Codacy, coverage, and Playwright validation;
- Linux build verification;
- signed and notarized macOS Apple Silicon and Intel release builds;
- Linux AppImage, DEB, and RPM release builds;
- Windows NSIS builds, optional Authenticode signing, and updater signatures;
- alpha and stable GitHub Release publication;
- documentation generation and publication to the `gh-pages` branch;
- automatic updates of open pull-request branches.

Chunk sidecars remain the inner-loop and pre-push validation environment. The CircleCI pipeline
reuses the same lane scripts so local-agent validation and authoritative CI execute the same gates.

GitHub remains the source host, release registry, and Pages host. CircleCI accesses GitHub through a
least-privilege `GH_TOKEN` stored in the restricted `nabu-github` context. CodeScene credentials
live separately in the `nabu-ci` context. Release signing and telemetry secrets live in
`nabu-release`, so build jobs cannot write to GitHub and publication jobs cannot read signing
credentials.

GitHub Pages publishes from the `gh-pages` branch. GitHub may show its managed Pages deployment,
but the repository contains no custom GitHub Actions workflows after cutover.

## Consequences

- Validation and releases can use explicit CircleCI resource classes and shared cache conventions.
- macOS, Linux, and Windows artifacts continue to build natively and in parallel.
- CircleCI context provisioning becomes a release prerequisite.
- GitHub Actions' automatic `GITHUB_TOKEN` and OIDC behavior are replaced by explicit,
  least-privilege CircleCI credentials.
- A shadow run must validate every platform artifact before the GitHub workflows are removed.
- ADR 0131's GitHub `workflow_call` implementation is superseded; shared release behavior now lives
  in parameterized CircleCI jobs.
