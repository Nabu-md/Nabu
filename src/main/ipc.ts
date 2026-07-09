/**
 * ipc.ts
 *
 * IPC Handler Registration — registers all Renderer→Main `ipcMain.handle()`
 * channels with Zod validation, and provides `sendToRenderer()` for
 * Main→Renderer push messages.
 *
 * Requirements: 13.1, 13.2, 13.3, 13.4, 13.5, 13.6
 */

import { ipcMain, dialog, BrowserWindow, app } from 'electron';
import { ZodError } from 'zod';
import path from 'path';
import fs from 'fs/promises';

import { IPCChannel } from '../shared/channels';
import {
  // Incoming schemas (Renderer → Main)
  VaultOpenSchema,
  VaultCloseSchema,
  FileGetSchema,
  TaskToggleSchema,
  ContextQuerySchema,
  ContextReindexSchema,
  VectorStatusSchema,
  ActivityLogSchema,
  SettingsGetSchema,
  SettingsSetSchema,
  VaultCreateSchema,
  FolderCreateSchema,
  NoteCreateSchema,
  NoteSaveSchema,
  NoteRenameSchema,
  NoteDeleteSchema,
  NoteGetRawSchema,
  NoteExportHtmlSchema,
  TemplatesListSchema,
  // Outgoing schemas (Main → Renderer)
  VaultScanResultSchema,
  FileGetResultSchema,
  TaskToggleResultSchema,
  ContextSearchResultSchema,
  ContextReindexResultSchema,
  VectorStatusResultSchema,
  NoteLoadedSchema,
  NoteUpdatedSchema,
  NoteDeletedSchema,
  NotesLoadedSchema,
  TemplatesListResultSchema,
  IndexBuildSchema,
  AssetReadSchema,
  SearchQuerySchema,
  SearchResponseSchema,
} from '../shared/schemas';

import { search } from '../shared/search-query';

import { loadSettings, saveSettings } from './settings';
import { substituteVariables } from './templates';

import type { StateManager } from './state';
import type { VectorManager } from './vector';
import type { VaultWatcher, WatcherConfig } from './watcher';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Build a structured activity:log payload and broadcast it to all renderer
 * windows. Used internally for validation warnings and handler errors.
 */
function emitActivityLog(
  level: 'info' | 'warn' | 'error',
  message: string,
): void {
  const payload = ActivityLogSchema.safeParse({
    level,
    message,
    timestamp: Date.now(),
  });

  if (!payload.success) return; // shouldn't happen with literal inputs

  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      win.webContents.send(IPCChannel.ACTIVITY_LOG, payload.data);
    }
  }
}

/**
 * Format a Zod validation error into a short readable string suitable for
 * an activity:log message.
 */
function formatZodError(err: ZodError): string {
  return err.issues.map((e) => `${e.path.join('.')}: ${e.message}`).join('; ');
}

// ---------------------------------------------------------------------------
// buildWatcherConfig
// ---------------------------------------------------------------------------

/**
 * Build a consolidated watcher configuration with vector embedding wired into
 * the add/change/delete callbacks.
 *
 * This function replaces the three previously-duplicated watcher callback sites
 * in `ipc.ts` (vault:open) and `index.ts` (restoreVault, NABU_TEST_VAULT).
 *
 * Vector embedding behaviour:
 * - onFileChanged: re-parses the file, pushes the updated AST to the renderer,
 *   then embeds the changed content (only for external edits — the watcher's
 *   internal `handleChange` already skips when `Pending_Write_Lock` is set).
 * - onFileAdded: pushes the updated file list, then reads and embeds the new
 *   file (guarded with `Pending_Write_Lock`).
 * - onFileDeleted: removes the file's vector from the Vectra index.
 *
 * Requirements: 1.1, 1.3, 1.9
 */
export function buildWatcherConfig(
  stateManager: StateManager,
  vectorManager: VectorManager,
  vaultPath: string,
  vaultMeta: { files: import('../shared/types').FileEntry[] },
): WatcherConfig {
  return {
    vaultPath,
    ignored: /^\.|\.nabu/,
    awaitWriteFinish: { stabilityThreshold: 50 },

    onFileChanged: async (filePath: string, isExternal: boolean) => {
      // Re-parse and push update to the renderer
      stateManager.invalidateAST(filePath);
      try {
        const ast = await stateManager.getAST(filePath);
        sendToRenderer(IPCChannel.NOTE_UPDATED, { path: filePath, ast, isExternal });

        // Embed the changed file. The watcher's handleChange already skips
        // when Pending_Write_Lock is set (requirement 1.9), but we guard here
        // as a belt-and-suspenders measure.
        if (!stateManager.hasPendingWrite(filePath)) {
          try {
            const content = await fs.readFile(filePath, 'utf-8');
            vectorManager.embedFile(filePath, content);
          } catch (embedErr) {
            emitActivityLog(
              'error',
              `[IPC] Failed to read file for embedding "${filePath}": ${String(embedErr)}`,
            );
          }
        }
      } catch (err) {
        emitActivityLog(
          'error',
          `[IPC] Failed to re-parse "${filePath}": ${String(err)}`,
        );
      }
    },

    onFileAdded: async (filePath: string) => {
      // Push the updated file list to the renderer
      sendToRenderer(IPCChannel.NOTES_LOADED, { vaultPath, files: vaultMeta.files });

      // Embed the new file (guard with Pending_Write_Lock — app-created files
      // set the lock before writing)
      if (!stateManager.hasPendingWrite(filePath)) {
        try {
          const content = await fs.readFile(filePath, 'utf-8');
          vectorManager.embedFile(filePath, content);
        } catch (embedErr) {
          emitActivityLog(
            'error',
            `[IPC] Failed to read new file for embedding "${filePath}": ${String(embedErr)}`,
          );
        }
      }
    },

    onFileDeleted: (filePath: string) => {
      // Remove from the vector index (async, non-blocking)
      vectorManager.removeFile(filePath).catch((err) => {
        emitActivityLog(
          'error',
          `[IPC] Failed to remove vector for "${filePath}": ${String(err)}`,
        );
      });
      // Notify the renderer
      sendToRenderer(IPCChannel.NOTE_DELETED, { path: filePath });
    },

    onError: (error: Error) => {
      emitActivityLog('error', `[IPC] Watcher error: ${error.message}`);
    },
  };
}

// ---------------------------------------------------------------------------
// sendToRenderer
// ---------------------------------------------------------------------------

/**
 * Schema map for outgoing Main→Renderer channels.
 * Used by `sendToRenderer` to validate payloads before dispatch.
 */
const outgoingSchemas: Partial<Record<IPCChannel, { safeParse: (data: unknown) => { success: boolean; data?: unknown; error?: ZodError } }>> = {
  [IPCChannel.NOTE_LOADED]:     NoteLoadedSchema,
  [IPCChannel.NOTE_UPDATED]:    NoteUpdatedSchema,
  [IPCChannel.NOTE_DELETED]:    NoteDeletedSchema,
  [IPCChannel.NOTES_LOADED]:    NotesLoadedSchema,
  [IPCChannel.CONTEXT_SEARCH]:  ContextSearchResultSchema,
  [IPCChannel.ACTIVITY_LOG]:    ActivityLogSchema,
  [IPCChannel.INDEX_BUILD]:     IndexBuildSchema,
};

/**
 * Send a validated payload from the main process to all renderer windows on
 * the given channel.
 *
 * - Validates the payload against the channel's Zod schema before sending.
 * - On validation failure: logs a warning to activity:log, does not send.
 * - Channels not present in `outgoingSchemas` are ignored silently (Req 13.5).
 *
 * Requirements: 13.4, 13.5
 */
export function sendToRenderer(channel: IPCChannel, payload: unknown): void {
  const schema = outgoingSchemas[channel];

  // Silently ignore undeclared outgoing channels (Req 13.5)
  if (!schema) return;

  const result = schema.safeParse(payload);

  if (!result.success) {
    const reason = result.error ? formatZodError(result.error as ZodError) : 'unknown';
    const msg = `[IPC] sendToRenderer validation failed on channel "${channel}": ${reason}`;
    console.warn(msg);
    emitActivityLog('warn', msg);
    return;
  }

  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      win.webContents.send(channel, result.data);
    }
  }
}

// ---------------------------------------------------------------------------
// copyDefaultTemplates
// ---------------------------------------------------------------------------

/**
 * Copy default template `.md` files from the bundled `default-templates`
 * resource directory into `<vaultPath>/_templates/` on first open.
 *
 * - Only copies if `_templates/` does not already exist (first-open guard).
 * - Resolves the source directory from `process.resourcesPath` when packaged,
 *   or from the local `resources/` folder in development.
 * - Failures are non-fatal: a warning is emitted to activity:log and the
 *   vault open / create flow continues normally.
 *
 * Requirements: 9.3
 */
async function copyDefaultTemplates(vaultPath: string): Promise<void> {
  const templatesDir = path.join(vaultPath, '_templates');

  // Only copy on first open — skip if _templates already exists
  try {
    await fs.access(templatesDir);
    return; // directory exists; nothing to do
  } catch {
    // Directory does not exist — proceed with copy
  }

  // Resolve source directory based on whether the app is packaged
  const srcDir = app.isPackaged
    ? path.join(process.resourcesPath, 'default-templates')
    : path.join(__dirname, '..', '..', '..', 'resources', 'default-templates');

  // Create the _templates directory
  await fs.mkdir(templatesDir, { recursive: true });

  // Read all .md files from the source dir and copy each to _templates/
  const dirents = await fs.readdir(srcDir, { withFileTypes: true });
  await Promise.all(
    dirents
      .filter((d) => d.isFile() && d.name.endsWith('.md'))
      .map((d) => fs.copyFile(path.join(srcDir, d.name), path.join(templatesDir, d.name))),
  );
}

// ---------------------------------------------------------------------------
// registerIPCHandlers
// ---------------------------------------------------------------------------

/**
 * Register all IPC `ipcMain.handle()` channels for Renderer→Main invocations.
 *
 * Each handler:
 * 1. Parses the raw payload through the appropriate Zod schema.
 * 2. Executes the handler logic.
 * 3. Returns a validated response.
 *
 * Validation failures and handler errors are caught, logged to activity:log,
 * and a structured error response is returned so the renderer is never left
 * awaiting a rejected promise without context.
 *
 * Requirements: 13.1, 13.2, 13.3, 13.6
 */
export function registerIPCHandlers(
  stateManager: StateManager,
  vectorManager: VectorManager,
  watcher: VaultWatcher,
): void {

  // Remove any previously registered handlers to avoid "handler already
  // registered" errors on hot-reload or second-window initialization.
  const channels = [
    IPCChannel.VAULT_OPEN,
    IPCChannel.VAULT_SCAN,
    IPCChannel.VAULT_CLOSE,
    IPCChannel.FILE_GET,
    IPCChannel.FILE_WATCH,
    IPCChannel.TASK_TOGGLE,
    IPCChannel.NOTE_TOGGLE,
    IPCChannel.CONTEXT_QUERY,
    IPCChannel.ACTIVITY_LOG,
    IPCChannel.SETTINGS_GET,
    IPCChannel.SETTINGS_SET,
    IPCChannel.VAULT_CREATE,
    IPCChannel.FOLDER_CREATE,
    IPCChannel.NOTE_CREATE,
    IPCChannel.NOTE_SAVE,
    IPCChannel.NOTE_RENAME,
    IPCChannel.NOTE_DELETE,
    IPCChannel.NOTE_GET_RAW,
    IPCChannel.NOTE_EXPORT_HTML,
    IPCChannel.TEMPLATES_LIST,
    IPCChannel.ASSET_READ,
    IPCChannel.CONTEXT_REINDEX,
    IPCChannel.VECTOR_STATUS,
    IPCChannel.SEARCH_QUERY,
    'vault:get-current' as IPCChannel,
  ];
  for (const ch of channels) {
    ipcMain.removeHandler(ch);
  }

  // -------------------------------------------------------------------------
  // vault:get-current — renderer pulls current vault state on mount
  // -------------------------------------------------------------------------
  ipcMain.removeHandler('vault:get-current');
  ipcMain.handle('vault:get-current', async (_event) => {
    try {
      const vault = stateManager.getCurrentVault();
      if (!vault) return null;
      return VaultScanResultSchema.parse(vault);
    } catch (err) {
      console.error('[IPC] vault:get-current error:', err);
      return null;
    }
  });

  // -------------------------------------------------------------------------
  // vault:open — open a vault by path, or prompt with native folder picker
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VAULT_OPEN, async (_event, rawPayload) => {
    let parsedPath: string | undefined;

    // Validate incoming payload (path is optional)
    const validation = VaultOpenSchema.safeParse(rawPayload ?? {});
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] vault:open validation failed: ${reason}`);
      return { error: reason };
    }

    parsedPath = validation.data.path;

    // If no path provided, show native folder picker (Req 13.3)
    if (!parsedPath) {
      const focusedWindow = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0];
      const result = await dialog.showOpenDialog(focusedWindow, {
        properties: ['openDirectory'],
        title: 'Open Vault',
        buttonLabel: 'Open',
      });

      if (result.canceled || result.filePaths.length === 0) {
        return { canceled: true };
      }

      parsedPath = result.filePaths[0];
    }

    try {
      const vaultMeta = await stateManager.openVault(parsedPath);

      // Copy default templates on first open (non-fatal)
      try {
        await copyDefaultTemplates(parsedPath);
      } catch (copyErr) {
        emitActivityLog('warn', `[IPC] vault:open — failed to copy default templates: ${String(copyErr)}`);
      }

      // Start the file watcher (uses shared config with vector embedding)
      watcher.start(buildWatcherConfig(stateManager, vectorManager, parsedPath, vaultMeta));

      const response = VaultScanResultSchema.parse(vaultMeta);

      // Trigger index build (task 9 implements buildIndexes; guard with try/catch)
      try {
        const indexResult = await (stateManager as any).buildIndexes?.();
        if (indexResult) {
          sendToRenderer(IPCChannel.INDEX_BUILD, indexResult);
        }
      } catch {
        // buildIndexes not yet available — silently ignore
      }

      return response;
    } catch (err) {
      const msg = `[IPC] vault:open handler error: ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return { error: String(err) };
    }
  });

  // -------------------------------------------------------------------------
  // vault:scan — re-scan the current vault and return updated metadata
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VAULT_SCAN, async (_event, _rawPayload) => {
    try {
      const currentVault = stateManager.getCurrentVault();
      if (!currentVault) {
        return { error: 'No vault is currently open' };
      }

      const vaultMeta = await stateManager.openVault(currentVault.path);
      const response = VaultScanResultSchema.parse(vaultMeta);

      // Trigger index build (task 9 implements buildIndexes; guard with try/catch)
      try {
        const indexResult = await (stateManager as any).buildIndexes?.();
        if (indexResult) {
          sendToRenderer(IPCChannel.INDEX_BUILD, indexResult);
        }
      } catch {
        // buildIndexes not yet available — silently ignore
      }

      return response;
    } catch (err) {
      const msg = `[IPC] vault:scan handler error: ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return { error: String(err) };
    }
  });

  // -------------------------------------------------------------------------
  // vault:close — stop the watcher and release vault state
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VAULT_CLOSE, async (_event, rawPayload) => {
    const validation = VaultCloseSchema.safeParse(rawPayload ?? {});
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] vault:close validation failed: ${reason}`);
      return { error: reason };
    }

    try {
      watcher.stop();
      return { success: true };
    } catch (err) {
      const msg = `[IPC] vault:close handler error: ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return { error: String(err) };
    }
  });

  // -------------------------------------------------------------------------
  // file:get — return the parsed AST for a given file path
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.FILE_GET, async (_event, rawPayload) => {
    const validation = FileGetSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] file:get validation failed: ${reason}`);
      return { error: reason };
    }

    const { path: filePath } = validation.data;

    try {
      const ast = await stateManager.getAST(filePath);
      const response = FileGetResultSchema.parse({ path: filePath, ast });
      return response;
    } catch (err) {
      const msg = `[IPC] file:get handler error for "${filePath}": ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return {
        path: filePath,
        ast: null,
        error: {
          line: 0,
          column: 0,
          message: String(err),
        },
      };
    }
  });

  // -------------------------------------------------------------------------
  // file:watch — acknowledge a watch request for a specific file
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.FILE_WATCH, async (_event, rawPayload) => {
    const validation = FileGetSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] file:watch validation failed: ${reason}`);
      return { error: reason };
    }

    // The VaultWatcher already watches the entire vault directory, so
    // individual file watch requests are acknowledged without additional action.
    return { success: true, path: validation.data.path };
  });

  // -------------------------------------------------------------------------
  // task:toggle — toggle a checkbox at the given line index
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.TASK_TOGGLE, async (_event, rawPayload) => {
    const validation = TaskToggleSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] task:toggle validation failed: ${reason}`);
      return TaskToggleResultSchema.parse({ success: false, error: reason });
    }

    const { path: filePath, lineIndex } = validation.data;

    try {
      await stateManager.toggleTask(filePath, lineIndex);
      return TaskToggleResultSchema.parse({ success: true });
    } catch (err) {
      const msg = `[IPC] task:toggle handler error for "${filePath}" line ${lineIndex}: ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return TaskToggleResultSchema.parse({ success: false, error: String(err) });
    }
  });

  // -------------------------------------------------------------------------
  // note:toggle — toggle a note-level item (same mechanism as task:toggle)
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_TOGGLE, async (_event, rawPayload) => {
    const validation = TaskToggleSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] note:toggle validation failed: ${reason}`);
      return TaskToggleResultSchema.parse({ success: false, error: reason });
    }

    const { path: filePath, lineIndex } = validation.data;

    try {
      await stateManager.toggleTask(filePath, lineIndex);
      return TaskToggleResultSchema.parse({ success: true });
    } catch (err) {
      const msg = `[IPC] note:toggle handler error for "${filePath}" line ${lineIndex}: ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return TaskToggleResultSchema.parse({ success: false, error: String(err) });
    }
  });

  // -------------------------------------------------------------------------
  // context:query — perform a semantic similarity search
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.CONTEXT_QUERY, async (_event, rawPayload) => {
    const validation = ContextQuerySchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] context:query validation failed: ${reason}`);
      return { error: reason };
    }

    const { text, excludePath } = validation.data;

    // Check vector index status before searching. If disabled or empty, return
    // an honest `disabled` flag so the renderer surfaces a clear message
    // instead of silently showing no results (Requirement 1.7).
    try {
      const status = await vectorManager.getStatus();
      if (status.disabled) {
        return { results: [], disabled: true, reason: status.reason ?? 'Embedding model not loaded' };
      }
      if (status.items === 0) {
        return { results: [], disabled: true, reason: 'Vector index is empty — save some notes to populate it' };
      }
    } catch (err) {
      emitActivityLog('warn', `[IPC] context:query status check failed: ${String(err)}`);
      // Fall through to search — let it fail normally if there's a real problem
    }

    try {
      const rawResults = await vectorManager.search(text, 5, excludePath);
      return ContextSearchResultSchema.parse({ results: rawResults });
    } catch (err) {
      const msg = `[IPC] context:query handler error: ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return { results: [], error: String(err) };
    }
  });

  // -------------------------------------------------------------------------
  // context:reindex — trigger full re-embed of all vault files
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.CONTEXT_REINDEX, async (_event, rawPayload) => {
    const validation = ContextReindexSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] context:reindex validation failed: ${reason}`);
      return { error: reason };
    }

    const { vaultPath } = validation.data;
    const vault = stateManager.getCurrentVault();
    if (!vault) {
      return { error: 'No vault is open' };
    }
    // Verify vault path matches the open vault
    if (vault.path !== vaultPath) {
      emitActivityLog('warn', `[IPC] context:reindex vault path mismatch: "${vaultPath}" !== "${vault.path}"`);
      return { error: 'Vault path does not match currently open vault' };
    }

    try {
      const processed = await vectorManager.reindexAll(vault.files);
      return ContextReindexResultSchema.parse({ processed });
    } catch (err) {
      const msg = `[IPC] context:reindex handler error: ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return { error: String(err) };
    }
  });

  // -------------------------------------------------------------------------
  // vector:status — return the current vector index status
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VECTOR_STATUS, async (_event, rawPayload) => {
    // Validate payload (empty schema — just ensures correctness)
    const validation = VectorStatusSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] vector:status validation failed: ${reason}`);
      return { disabled: true, reason };
    }

    try {
      const status = await vectorManager.getStatus();
      return VectorStatusResultSchema.parse(status);
    } catch (err) {
      const msg = `[IPC] vector:status handler error: ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return { disabled: true, reason: String(err), items: 0 };
    }
  });

  // -------------------------------------------------------------------------
  // search:query — execute a text search against the extended search index
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.SEARCH_QUERY, async (_event, rawPayload) => {
    const validation = SearchQuerySchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] search:query validation failed: ${reason}`);
      return { results: [] };
    }

    const { query } = validation.data;
    const vault = stateManager.getCurrentVault();
    if (!vault) {
      return { results: [] };
    }

    try {
      const results = search(
        query,
        vault.files,
        vault.path,
        stateManager.getExtendedIndex(),
        (p) => stateManager.getASTSync(p),
      );
      return SearchResponseSchema.parse({ results });
    } catch (err) {
      const msg = `[IPC] search:query handler error: ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return { results: [] };
    }
  });

  // -------------------------------------------------------------------------
  // activity:log — receive log entries from the renderer
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.ACTIVITY_LOG, async (_event, rawPayload) => {
    const validation = ActivityLogSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      // Log to console only — avoid recursive loop back to renderer
      console.warn(`[IPC] activity:log validation failed: ${reason}`);
      return { error: reason };
    }

    const { level, message } = validation.data;
    console[level](`[Renderer] ${message}`);
    return { success: true };
  });

  // -------------------------------------------------------------------------
  // settings:get — retrieve a single settings value by key
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.SETTINGS_GET, async (_event, rawPayload) => {
    const validation = SettingsGetSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] settings:get validation failed: ${reason}`);
      return { success: false, error: reason };
    }

    const { key } = validation.data;

    try {
      const settings = await loadSettings();
      const value = (settings as unknown as Record<string, unknown>)[key];
      return { value };
    } catch (err) {
      const msg = `[IPC] settings:get handler error for key "${key}": ${String(err)}`;
      console.error(msg);
      emitActivityLog('warn', msg);
      return { success: false, error: String(err) };
    }
  });

  // -------------------------------------------------------------------------
  // settings:set — update a single settings value by key
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.SETTINGS_SET, async (_event, rawPayload) => {
    const validation = SettingsSetSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] settings:set validation failed: ${reason}`);
      return { success: false, error: reason };
    }

    const { key, value } = validation.data;

    try {
      const settings = await loadSettings();
      const updated = { ...settings, [key]: value };
      await saveSettings(updated);
      return { success: true };
    } catch (err) {
      const msg = `[IPC] settings:set handler error for key "${key}": ${String(err)}`;
      console.error(msg);
      emitActivityLog('warn', msg);
      return { success: false, error: String(err) };
    }
  });

  // -------------------------------------------------------------------------
  // vault:create — create a new vault directory and open it
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VAULT_CREATE, async (_event, rawPayload) => {
    const validation = VaultCreateSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] vault:create validation failed: ${reason}`);
      return { error: reason };
    }

    const { parentPath, name } = validation.data;
    const newPath = path.join(parentPath, name);

    try {
      // Create the vault directory
      await fs.mkdir(newPath, { recursive: true });

      // Write a Welcome.md file as the initial note
      const welcomePath = path.join(newPath, 'Welcome.md');
      const welcomeContent = `# Welcome to ${name}\n\nThis is your new vault. Start writing!\n`;
      stateManager.setPendingWrite(welcomePath);
      try {
        await fs.writeFile(welcomePath, welcomeContent, 'utf-8');
      } finally {
        stateManager.clearPendingWrite(welcomePath);
      }

      // Open the newly created vault
      const vaultMeta = await stateManager.openVault(newPath);
      const result = VaultScanResultSchema.parse(vaultMeta);

      // Trigger index build (task 9 implements buildIndexes; guard with try/catch)
      try {
        const indexResult = await (stateManager as any).buildIndexes?.();
        if (indexResult) {
          sendToRenderer(IPCChannel.INDEX_BUILD, indexResult);
        }
      } catch {
        // buildIndexes not yet available — silently ignore
      }

      return result;
    } catch (err) {
      const msg = `[IPC] vault:create handler error: ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return { error: String(err) };
    }
  });

  // -------------------------------------------------------------------------
  // folder:create — create a new folder inside the vault
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.FOLDER_CREATE, async (_event, rawPayload) => {
    const validation = FolderCreateSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] folder:create validation failed: ${reason}`);
      return { success: false, error: reason };
    }

    const { path: folderPath } = validation.data;

    try {
      await fs.mkdir(folderPath, { recursive: true });
      return { success: true };
    } catch (err) {
      const msg = `[IPC] folder:create handler error for "${folderPath}": ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return { success: false, error: String(err) };
    }
  });

  // -------------------------------------------------------------------------
  // note:create — create a new note, optionally from a template
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_CREATE, async (_event, rawPayload) => {
    const validation = NoteCreateSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] note:create validation failed: ${reason}`);
      return { error: reason };
    }

    const { vaultPath, name, templateContent } = validation.data;

    // Strip .md suffix if present, then re-append for the actual file path
    const normalisedName = name.replace(/\.md$/i, '');
    const filePath = path.join(vaultPath, normalisedName + '.md');

    // Check for existing file
    try {
      await fs.access(filePath);
      // File exists — return error
      return { success: false, error: 'A note with that name already exists' };
    } catch {
      // File does not exist — proceed
    }

    try {
      // Prepare content with template variable substitution
      const now = new Date();
      const dateStr = now.toISOString().slice(0, 10);
      const timeStr = now.toTimeString().slice(0, 5);

      const rawContent = templateContent ?? `# ${normalisedName}\n`;
      const content = substituteVariables(rawContent, {
        title: normalisedName,
        date: dateStr,
        time: timeStr,
      });

      // Write file with pending write lock
      stateManager.setPendingWrite(filePath);
      try {
        await fs.writeFile(filePath, content, 'utf-8');
      } finally {
        stateManager.clearPendingWrite(filePath);
      }

      // Get AST for the new file and return
      const ast = await stateManager.getAST(filePath);
      const response = FileGetResultSchema.parse({ path: filePath, ast });
      return response;
    } catch (err) {
      const msg = `[IPC] note:create handler error for "${filePath}": ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return {
        path: filePath,
        ast: null,
        error: { line: 0, column: 0, message: String(err) },
      };
    }
  });

  // -------------------------------------------------------------------------
  // note:save — write updated content to an existing note
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_SAVE, async (_event, rawPayload) => {
    const validation = NoteSaveSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] note:save validation failed: ${reason}`);
      return { success: false, error: reason };
    }

    const { path: filePath, content } = validation.data;

    try {
      stateManager.setPendingWrite(filePath);
      await fs.writeFile(filePath, content, 'utf-8');
      stateManager.invalidateAST(filePath);
      stateManager.clearPendingWrite(filePath);

      // Incremental index update (task 9 implements updateIndexesForFile; guard with try/catch)
      try {
        const indexResult = await (stateManager as any).updateIndexesForFile?.(filePath);
        if (indexResult) {
          sendToRenderer(IPCChannel.INDEX_BUILD, indexResult);
        }
      } catch {
        // updateIndexesForFile not yet available — silently ignore
      }

      // Enqueue an embedding for the saved file (Requirement 1.2).
      // VectorManager.embedFile skips empty-content notes internally (Requirement 1.8)
      // and respects the embeddingsDisabled flag, so calling it unconditionally is safe.
      vectorManager.embedFile(filePath, content);

      return { success: true };
    } catch (err) {
      // Ensure lock is released even on error
      stateManager.clearPendingWrite(filePath);
      const msg = `[IPC] note:save handler error for "${filePath}": ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return { success: false, error: String(err) };
    }
  });

  // -------------------------------------------------------------------------
  // note:rename — rename a note file (no PendingWriteLock — watcher handles events)
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_RENAME, async (_event, rawPayload) => {
    const validation = NoteRenameSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] note:rename validation failed: ${reason}`);
      return { success: false, error: reason };
    }

    const { oldPath, newPath: rawNewPath } = validation.data;

    // Normalise: append .md if not already present
    const normalisedNewPath = rawNewPath.endsWith('.md') ? rawNewPath : rawNewPath + '.md';

    try {
      await fs.rename(oldPath, normalisedNewPath);
      return { success: true };
    } catch (err) {
      const msg = `[IPC] note:rename handler error "${oldPath}" → "${normalisedNewPath}": ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return { success: false, error: String(err) };
    }
  });

  // -------------------------------------------------------------------------
  // note:delete — delete a note file (no PendingWriteLock — watcher handleUnlink
  //               never checks the lock, so it has no effect)
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_DELETE, async (_event, rawPayload) => {
    const validation = NoteDeleteSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] note:delete validation failed: ${reason}`);
      return { success: false, error: reason };
    }

    const { path: filePath } = validation.data;

    try {
      await fs.rm(filePath);

      // Full index rebuild after deletion (deleted file must be purged from all index entries)
      try {
        const indexResult = await (stateManager as any).buildIndexes?.();
        if (indexResult) {
          sendToRenderer(IPCChannel.INDEX_BUILD, indexResult);
        }
      } catch {
        // buildIndexes not yet available — silently ignore
      }

      return { success: true };
    } catch (err) {
      const msg = `[IPC] note:delete handler error for "${filePath}": ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return { success: false, error: String(err) };
    }
  });

  // -------------------------------------------------------------------------
  // note:get-raw — return the raw markdown string for a note
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_GET_RAW, async (_event, rawPayload) => {
    const validation = NoteGetRawSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] note:get-raw validation failed: ${reason}`);
      return { path: '', error: reason };
    }

    const { path: filePath } = validation.data;

    try {
      const content = await fs.readFile(filePath, 'utf-8');
      return { path: filePath, content };
    } catch (err) {
      const msg = `[IPC] note:get-raw handler error for "${filePath}": ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return { path: filePath, error: String(err) };
    }
  });

  // -------------------------------------------------------------------------
  // asset:read — read a file as a base64 data URI for sandboxed iframes
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.ASSET_READ, async (_event, rawPayload) => {
    const validation = AssetReadSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] asset:read validation failed: ${reason}`);
      return { path: '', error: reason };
    }

    const { path: filePath } = validation.data;

    try {
      // Read the file as a Buffer so it works for both text and binary
      const buffer = await fs.readFile(filePath);
      const ext = path.extname(filePath).toLowerCase();
      const mimeMap: Record<string, string> = {
        '.png': 'image/png',
        '.jpg': 'image/jpeg',
        '.jpeg': 'image/jpeg',
        '.gif': 'image/gif',
        '.svg': 'image/svg+xml',
        '.webp': 'image/webp',
        '.ico': 'image/x-icon',
        '.css': 'text/css',
        '.js': 'application/javascript',
        '.json': 'application/json',
        '.woff': 'font/woff',
        '.woff2': 'font/woff2',
        '.ttf': 'font/ttf',
      };
      const mime = mimeMap[ext] ?? 'application/octet-stream';
      const dataUri = `data:${mime};base64,${buffer.toString('base64')}`;
      return { path: filePath, dataUri };
    } catch (err) {
      const msg = `[IPC] asset:read handler error for "${filePath}": ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return { path: filePath, error: String(err) };
    }
  });

  // -------------------------------------------------------------------------
  // note:export-html — export a note as an HTML file via save dialog
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_EXPORT_HTML, async (_event, rawPayload) => {
    const validation = NoteExportHtmlSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] note:export-html validation failed: ${reason}`);
      return { success: false, error: reason };
    }

    const { path: notePath, html } = validation.data;

    try {
      const focusedWindow = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0];
      const dialogResult = await dialog.showSaveDialog(focusedWindow, {
        defaultPath: notePath,
        filters: [{ name: 'HTML', extensions: ['html'] }],
      });

      if (dialogResult.canceled || !dialogResult.filePath) {
        return { success: false };
      }

      const savedPath = dialogResult.filePath;
      stateManager.setPendingWrite(savedPath);
      try {
        await fs.writeFile(savedPath, html, 'utf-8');
      } finally {
        stateManager.clearPendingWrite(savedPath);
      }

      return { success: true, savedPath };
    } catch (err) {
      const msg = `[IPC] note:export-html handler error for "${notePath}": ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return { success: false, error: String(err) };
    }
  });

  // -------------------------------------------------------------------------
  // templates:list — list all templates in the vault's _templates directory
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.TEMPLATES_LIST, async (_event, rawPayload) => {
    const validation = TemplatesListSchema.safeParse(rawPayload);
    if (!validation.success) {
      const reason = formatZodError(validation.error);
      emitActivityLog('warn', `[IPC] templates:list validation failed: ${reason}`);
      return { templates: [] };
    }

    const { vaultPath } = validation.data;
    const templatesDir = path.join(vaultPath, '_templates');

    // Check if _templates directory exists
    try {
      await fs.access(templatesDir);
    } catch {
      // Directory does not exist — return empty list
      return { templates: [] };
    }

    try {
      const dirents = await fs.readdir(templatesDir, { withFileTypes: true });
      const mdFiles = dirents.filter((d) => d.isFile() && d.name.endsWith('.md'));

      const templates = await Promise.all(
        mdFiles.map(async (dirent) => {
          const templatePath = path.join(templatesDir, dirent.name);
          const content = await fs.readFile(templatePath, 'utf-8');
          const name = path.basename(dirent.name, '.md');
          return { name, path: templatePath, content };
        }),
      );

      return TemplatesListResultSchema.parse({ templates });
    } catch (err) {
      const msg = `[IPC] templates:list handler error for vault "${vaultPath}": ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return { templates: [] };
    }
  });
}
