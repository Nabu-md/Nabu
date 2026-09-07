# Sidebar & Git Panel Redesign Plan

## What Went Wrong

The previous sidebar/gitpanel changes introduced regressions:
- **Opacity broken** — visual opacity changes did not apply correctly
- **Settings not updating** — FluidVoice, Harper, and AnyDoc settings did not persist or reflect in the UI
- **General breakage** — sidebar and git panel functionality was destabilized

## Follow-Up Plan

### Phase 1: Stabilize (Immediate)
1. Revert the three problematic commits:
   - `d345a450` — "Add open-source attribution and remove sidebar feature"
   - `dcd0c09f` — "chore: remove pre-commit hook"
   - `f07c72c5` — "rebrand"
2. Verify all sidebar and gitpanel tests pass
3. Verify manual QA: opacity, settings persistence, git operations

### Phase 2: Audit (Before Re-Implementation)
1. Review current sidebar component architecture
2. Document all FluidVoice/Harper/AnyDoc settings paths
3. Identify the opacity mechanism and why it failed
4. List all gitpanel dependencies and state flows

### Phase 3: Incremental Re-Implementation
1. Make one small change at a time
2. After each change, run targeted tests:
   - `pnpm test` for unit tests
   - Manual QA for opacity and settings
3. Do NOT batch multiple concerns into one commit

### Phase 4: Verification
1. Run full test suite: `pnpm test && pnpm test:coverage`
2. Run Rust tests: `cargo test`
3. Build and install locally: `pnpm tauri build`
4. Manual QA checklist:
   - [ ] Sidebar opacity works
   - [ ] FluidVoice settings save and load
   - [ ] Harper settings save and load
   - [ ] AnyDoc settings save and load
   - [ ] Git panel operations (commit, push, pull) work
   - [ ] No console errors

### Rollback Strategy
- Keep the revert commit ready
- Each subsequent change should be independently revertible
- Tag stable state before each risky change
