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
  NoteLoadedSchema,
  NoteUpdatedSchema,
  NoteDeletedSchema,
  NotesLoadedSchema,
  TemplatesListResultSchema,
  IndexBuildSchema,
} from '../shared/schemas';

import { loadSettings, saveSettings } from './settings';
import { substituteVariables } from './templates';

import type { StateManager } from './state';
import type { VectorManager } from './vector';
import type { VaultWatcher } from './watcher';

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

      // Start the file watcher for the newly opened vault
      watcher.start({
        vaultPath: parsedPath,
        ignored: /^\.|\.onyx/,
        awaitWriteFinish: { stabilityThreshold: 50 },
        onFileChanged: (filePath, isExternal) => {
          stateManager.invalidateAST(filePath);
          stateManager.getAST(filePath)
            .then((ast) => {
              sendToRenderer(IPCChannel.NOTE_UPDATED, {
                path: filePath,
                ast,
                isExternal,
              });
            })
            .catch((err) => {
              emitActivityLog('error', `[IPC] Failed to re-parse "${filePath}": ${String(err)}`);
            });
        },
        onFileAdded: (_filePath) => {
          sendToRenderer(IPCChannel.NOTES_LOADED, { vaultPath: parsedPath, files: vaultMeta.files });
        },
        onFileDeleted: (filePath) => {
          sendToRenderer(IPCChannel.NOTE_DELETED, { path: filePath });
        },
        onError: (error) => {
          emitActivityLog('error', `[IPC] Watcher error: ${error.message}`);
        },
      });

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

    try {
      const rawResults = await vectorManager.search(text, 5, excludePath);
      const response = ContextSearchResultSchema.parse({ results: rawResults });
      return response;
    } catch (err) {
      const msg = `[IPC] context:query handler error: ${String(err)}`;
      console.error(msg);
      emitActivityLog('error', msg);
      return { results: [], error: String(err) };
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
