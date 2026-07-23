# Diagnostic Inventory

Initial Diagnostics: 53

## Category Breakdown

| Category | Count | Root Cause |
| :--- | :--- | :--- |
| **TS18046 (Unknown Narrowing)** | 14 | IPC response types are not correctly narrowed/typed. |
| **TS2339 (Missing Properties)** | 23 | IPC bridge interface mismatch or incorrect type assumptions. |
| **TS2345/TS2322 (Assignment/Mismatch)**| 8 | Parameter/Return type mismatch with IPC definitions. |
| **TS7006 (Implicit Any)** | 6 | Lack of explicit typing on callback parameters. |
| **TS2349/Misc (Callable Errors)** | 4 | Incorrect function signatures/JSX element types. |

## Target Files
- `src/renderer/src/features/pdf/PdfViewer.tsx` (11 diagnostics)
- `src/renderer/src/features/widgets/widgetService.ts` (11 diagnostics)
- `src/renderer/src/features/settings/SettingsPanel.tsx` (9 diagnostics)
- `src/renderer/src/features/vault/vaultCommands.ts` (8 diagnostics)
- `src/renderer/src/App.tsx` (4 diagnostics)
- `src/renderer/src/features/graph/GraphView.tsx` (3 diagnostics)
- `src/renderer/src/features/notes/noteCommands.ts` (3 diagnostics)
- `src/renderer/src/features/vault/SetupWizard.tsx` (3 diagnostics)
- `src/renderer/src/features/notes/blocks/KanbanBlock.tsx` (2 diagnostics)
- `src/renderer/src/features/vault/FileTree.tsx` (2 diagnostics)
- `src/renderer/src/shared/components/SandboxedHtml.tsx` (2 diagnostics)
- `src/renderer/src/features/notes/NoteView.tsx` (1 diagnostics)
