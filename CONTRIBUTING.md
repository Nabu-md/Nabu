# Contributing to Nemo

Thanks for your interest in contributing. Nemo is AGPL-3.0 open-source, and every contribution makes the project stronger.

## Quick Start

```bash
# Prerequisites: Node.js 20+, npm 9+
git clone https://github.com/Nemo-notes/Nemo.git
cd nemo
npm install
npm run dev
```

## Branch Strategy

No direct pushes to `main`. All work happens on feature branches.

```bash
# Create a branch from main
git checkout -b feat/your-feature-name
# or
git checkout -b fix/your-bug-fix

# Push and open a pull request
git push -u origin feat/your-feature-name
```

Pull requests must be reviewed before merging. Only maintainers can merge to `main`.

## Before You Open a PR

Run these locally to make sure the CI will pass:

```bash
npm run lint          # ESLint — code must pass
npx prettier --check . # Prettier — formatting must be consistent
npm run typecheck     # TypeScript — no type errors
npm run test          # Vitest — all tests pass
npm run build         # electron-vite — production build succeeds
```

The PR Gate workflow runs these automatically. If any fail, the PR is blocked from merging.

## Code Standards

- **TypeScript strict mode** is enabled. Avoid `any` — use proper types or `unknown`.
- **Prettier** handles formatting. Run `npm run format` before committing.
- **ESLint** enforces style. No disabling rules without a comment explaining why.
- **Zod schemas** must be used for all new IPC message validation.
- **Tests are mandatory.** New features must include unit tests. Bug fixes must include a regression test.

## Testing

```bash
# Run all unit and property-based tests
npm run test

# Run tests in watch mode during development
npm run test:watch

# Run e2e tests (requires Playwright browsers installed)
npx playwright install
npm run test:e2e
```

The test suite uses [Vitest](https://vitest.dev) with [fast-check](https://github.com/dubzzz/fast-check) for property-based testing. If you're adding a new module, include property-based tests for its invariants — they catch edge cases example-based tests miss.

## Architecture Overview

```
src/
├── main/          # Electron main process
├── preload/       # Context bridge (ipcRenderer exposed to renderer)
├── renderer/      # React 19 UI (Vite + Tailwind v4)
└── shared/        # Types, schemas, utilities shared between main and renderer
```

- **Main process** handles file I/O, markdown parsing, file watching, and IPC.
- **Renderer process** handles UI rendering, state management, and user interaction.
- **Shared** contains types, Zod schemas, and pure utilities used by both.

## Questions?

Open a [Discussions](https://github.com/Nemo-notes/Nemo/discussions) thread — we're responsive and friendly.
