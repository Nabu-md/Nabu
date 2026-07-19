/**
 * notes.ts — Notes feature IPC module.
 *
 * Owns note lifecycle channels (create/save/rename/delete/get-raw/export-html/
 * daily/random/compose/unique/toggle), task:toggle, templates:list, view-state
 * fold channels, and properties read/write.
 *
 * This is a pure structural extraction from the previous monolithic
 * `src/main/ipc.ts`. Handler behavior is unchanged.
 */

import { ipcMain, dialog, BrowserWindow } from 'electron'
import path from 'path'
import fs from 'fs/promises'

import { IPCChannel } from '@shared/channels'
import {
  TaskToggleSchema,
  TaskToggleResultSchema,
  NoteCreateSchema,
  NoteSaveSchema,
  NoteRenameSchema,
  NoteDeleteSchema,
  NoteGetRawSchema,
  NoteExportHtmlSchema,
  NoteDailySchema,
  NoteDailyResultSchema,
  NoteRandomSchema,
  NoteRandomResultSchema,
  TemplatesListSchema,
  TemplatesListResultSchema,
  ViewStateGetFoldSchema,
  ViewStateSetFoldSchema,
  PropertiesReadSchema,
  PropertiesReadResultSchema,
  PropertiesWriteSchema,
  PropertiesWriteResultSchema,
  NoteComposeSchema,
  NoteComposeResultSchema,
  NoteUniqueSchema,
  NoteUniqueResultSchema,
  FileGetResultSchema
} from '@shared/schemas'

import { loadSettings } from '../services/settings'
import { substituteVariables } from '../services/templates'
import { mergeNotes } from '../services/composer'
import { generateUniqueNoteName } from '../services/unique-note'
import { loadViewState, setFoldState } from '../services/view-state'

import type { IPCContext } from './context'
import {
  emitActivityLog,
  formatZodError,
  getSessionForVault,
  injectAutoProperty,
  extractFrontmatter,
  replaceFrontmatterRaw,
  sendToRenderer,
  normalizeError,
  errorToString
} from './shared'

/**
 * Register all notes-feature IPC handlers.
 */
export function registerNotesIPC(ctx: IPCContext): void {
  const { stateManager, vectorManager } = ctx

  // -------------------------------------------------------------------------
  // task:toggle — toggle a checkbox at the given line index
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.TASK_TOGGLE, async (_event, rawPayload) => {
    const validation = TaskToggleSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] task:toggle validation failed: ${reason}`)
      return TaskToggleResultSchema.parse({ success: false, error: reason })
    }

    const { path: filePath, lineIndex, vaultId } = validation.data

    try {
      const { stateManager: sm } = getSessionForVault(vaultId)
      await sm.toggleTask(filePath, lineIndex)
      return TaskToggleResultSchema.parse({ success: true })
    } catch (err) {
      const normalized = normalizeError(err, { path: filePath, lineIndex })
      const msg = `[IPC] task:toggle handler error for "${filePath}" line ${lineIndex}: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return TaskToggleResultSchema.parse({ success: false, error: errorToString(normalized) })
    }
  })

  // -------------------------------------------------------------------------
  // note:toggle — toggle a note-level item (same mechanism as task:toggle)
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_TOGGLE, async (_event, rawPayload) => {
    const validation = TaskToggleSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:toggle validation failed: ${reason}`)
      return TaskToggleResultSchema.parse({ success: false, error: reason })
    }

    const { path: filePath, lineIndex } = validation.data

    try {
      await stateManager.toggleTask(filePath, lineIndex)
      return TaskToggleResultSchema.parse({ success: true })
    } catch (err) {
      const normalized = normalizeError(err, { path: filePath, lineIndex })
      const msg = `[IPC] note:toggle handler error for "${filePath}" line ${lineIndex}: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return TaskToggleResultSchema.parse({ success: false, error: errorToString(normalized) })
    }
  })

  // -------------------------------------------------------------------------
  // note:create — create a new note, optionally from a template
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_CREATE, async (_event, rawPayload) => {
    const validation = NoteCreateSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:create validation failed: ${reason}`)
      return { error: reason }
    }

    const { vaultPath, name, templateContent } = validation.data

    const normalisedName = name.replace(/\.md$/i, '')
    const filePath = path.join(vaultPath, normalisedName + '.md')

    try {
      await fs.access(filePath)
      return { success: false, error: 'A note with that name already exists' }
    } catch {
      // File does not exist — proceed
    }

    try {
      const now = new Date()
      const dateStr = now.toISOString().slice(0, 10)
      const timeStr = now.toTimeString().slice(0, 5)

      const rawContent = templateContent ?? `# ${normalisedName}\n`
      let content = substituteVariables(rawContent, {
        title: normalisedName,
        date: dateStr,
        time: timeStr
      })

      const settings = await loadSettings()
      if (settings.autoProperties) {
        content = injectAutoProperty(content, 'created', now.toISOString(), true)
      }

      stateManager.setPendingWrite(filePath)
      try {
        await fs.writeFile(filePath, content, 'utf-8')
      } finally {
        stateManager.clearPendingWrite(filePath)
      }

      const ast = await stateManager.getAST(filePath)
      const response = FileGetResultSchema.parse({ path: filePath, ast })
      return response
    } catch (err) {
      const normalized = normalizeError(err, { path: filePath })
      const msg = `[IPC] note:create handler error for "${filePath}": ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return {
        path: filePath,
        ast: null,
        error: { line: 0, column: 0, message: errorToString(normalized) }
      }
    }
  })

  // -------------------------------------------------------------------------
  // note:save — write updated content to an existing note
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_SAVE, async (_event, rawPayload) => {
    const validation = NoteSaveSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:save validation failed: ${reason}`)
      return { success: false, error: reason }
    }

    const { path: filePath, content } = validation.data

    try {
      const settings = await loadSettings()
      const finalContent = settings.autoProperties
        ? injectAutoProperty(content, 'modified', new Date().toISOString(), false)
        : content

      stateManager.setPendingWrite(filePath)
      await fs.writeFile(filePath, finalContent, 'utf-8')
      stateManager.invalidateAST(filePath)
      stateManager.clearPendingWrite(filePath)

      try {
        const indexResult = await (stateManager as any).updateIndexesForFile?.(filePath)
        if (indexResult) {
          sendToRenderer(IPCChannel.INDEX_BUILD, indexResult)
        }
      } catch {
        // updateIndexesForFile not yet available — silently ignore
      }

      vectorManager.embedFile(filePath, content)

      return { success: true }
    } catch (err) {
      stateManager.clearPendingWrite(filePath)
      const normalized = normalizeError(err, { path: filePath })
      const msg = `[IPC] note:save handler error for "${filePath}": ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { success: false, error: errorToString(normalized) }
    }
  })

  // -------------------------------------------------------------------------
  // note:rename — rename a note file
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_RENAME, async (_event, rawPayload) => {
    const validation = NoteRenameSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:rename validation failed: ${reason}`)
      return { success: false, error: reason }
    }

    const { oldPath, newPath: rawNewPath } = validation.data

    const normalisedNewPath = rawNewPath.endsWith('.md') ? rawNewPath : rawNewPath + '.md'

    try {
      await fs.rename(oldPath, normalisedNewPath)
      return { success: true }
    } catch (err) {
      const normalized = normalizeError(err, { oldPath, newPath: normalisedNewPath })
      const msg = `[IPC] note:rename handler error "${oldPath}" → "${normalisedNewPath}": ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { success: false, error: errorToString(normalized) }
    }
  })

  // -------------------------------------------------------------------------
  // note:delete — delete a note file
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_DELETE, async (_event, rawPayload) => {
    const validation = NoteDeleteSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:delete validation failed: ${reason}`)
      return { success: false, error: reason }
    }

    const { path: filePath } = validation.data

    try {
      await fs.rm(filePath)

      try {
        const indexResult = await (stateManager as any).buildIndexes?.()
        if (indexResult) {
          sendToRenderer(IPCChannel.INDEX_BUILD, indexResult)
        }
      } catch {
        // buildIndexes not yet available — silently ignore
      }

      return { success: true }
    } catch (err) {
      const normalized = normalizeError(err, { path: filePath })
      const msg = `[IPC] note:delete handler error for "${filePath}": ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { success: false, error: errorToString(normalized) }
    }
  })

  // -------------------------------------------------------------------------
  // note:get-raw — return the raw markdown string for a note
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_GET_RAW, async (_event, rawPayload) => {
    const validation = NoteGetRawSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:get-raw validation failed: ${reason}`)
      return { path: '', error: reason }
    }

    const { path: filePath } = validation.data

    try {
      const content = await fs.readFile(filePath, 'utf-8')
      return { path: filePath, content }
    } catch (err) {
      const normalized = normalizeError(err, { path: filePath })
      const msg = `[IPC] note:get-raw handler error for "${filePath}": ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { path: filePath, error: errorToString(normalized) }
    }
  })

  // -------------------------------------------------------------------------
  // asset:read — read a file as a base64 data URI for sandboxed iframes
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.ASSET_READ, async (_event, rawPayload) => {
    const validation = (await import('@shared/schemas')).AssetReadSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] asset:read validation failed: ${reason}`)
      return { path: '', error: reason }
    }

    const { path: filePath } = validation.data

    try {
      const buffer = await fs.readFile(filePath)
      const ext = path.extname(filePath).toLowerCase()
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
        '.ttf': 'font/ttf'
      }
      const mime = mimeMap[ext] ?? 'application/octet-stream'
      const dataUri = `data:${mime};base64,${buffer.toString('base64')}`
      return { path: filePath, dataUri }
    } catch (err) {
      const normalized = normalizeError(err, { path: filePath })
      const msg = `[IPC] asset:read handler error for "${filePath}": ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { path: filePath, error: errorToString(normalized) }
    }
  })

  // -------------------------------------------------------------------------
  // note:export-html — export a note as an HTML file via save dialog
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_EXPORT_HTML, async (_event, rawPayload) => {
    const validation = NoteExportHtmlSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:export-html validation failed: ${reason}`)
      return { success: false, error: reason }
    }

    const { path: notePath, html } = validation.data

    try {
      const focusedWindow = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0]
      const dialogResult = await dialog.showSaveDialog(focusedWindow, {
        defaultPath: notePath,
        filters: [{ name: 'HTML', extensions: ['html'] }]
      })

      if (dialogResult.canceled || !dialogResult.filePath) {
        return { success: false }
      }

      const savedPath = dialogResult.filePath
      stateManager.setPendingWrite(savedPath)
      try {
        await fs.writeFile(savedPath, html, 'utf-8')
      } finally {
        stateManager.clearPendingWrite(savedPath)
      }

      return { success: true, savedPath }
    } catch (err) {
      const normalized = normalizeError(err, { path: notePath })
      const msg = `[IPC] note:export-html handler error for "${notePath}": ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { success: false, error: errorToString(normalized) }
    }
  })

  // -------------------------------------------------------------------------
  // note:daily — open or create today's daily note
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_DAILY, async (_event, rawPayload) => {
    const validation = NoteDailySchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:daily validation failed: ${reason}`)
      return { path: '', ast: null, created: false, error: reason }
    }

    const { vaultPath } = validation.data

    try {
      const settings = await loadSettings()
      const now = new Date()

      const dateFormat = settings.dailyNoteDateFormat || 'YYYY-MM-DD'
      const dateStr = dateFormat
        .replace('YYYY', String(now.getFullYear()))
        .replace('MM', String(now.getMonth() + 1).padStart(2, '0'))
        .replace('DD', String(now.getDate()).padStart(2, '0'))

      const folder = settings.dailyNoteFolder || 'Daily'
      const dirPath = path.join(vaultPath, folder)
      const filePath = path.join(dirPath, `${dateStr}.md`)

      let created = false
      let content: string
      try {
        await fs.access(filePath)
        content = await fs.readFile(filePath, 'utf-8')
      } catch {
        created = true

        await fs.mkdir(dirPath, { recursive: true })

        const templateName = settings.dailyNoteTemplate || ''
        if (templateName) {
          const templatesDir = path.join(vaultPath, '_templates')
          const templatePath = path.join(templatesDir, `${templateName}.md`)
          try {
            const templateContent = await fs.readFile(templatePath, 'utf-8')
            const dateFormatted = now.toISOString().slice(0, 10)
            const timeFormatted = now.toTimeString().slice(0, 5)
            content = substituteVariables(templateContent, {
              title: dateStr,
              date: dateFormatted,
              time: timeFormatted
            })
          } catch {
            content = `# ${dateStr}\n\n`
          }
        } else {
          content = `# ${dateStr}\n\n`
        }

        const dnSettings = await loadSettings()
        if (dnSettings.autoProperties) {
          content = injectAutoProperty(content, 'created', now.toISOString(), true)
        }

        stateManager.setPendingWrite(filePath)
        try {
          await fs.writeFile(filePath, content, 'utf-8')
        } finally {
          stateManager.clearPendingWrite(filePath)
        }
      }

      const ast = await stateManager.getAST(filePath)
      return NoteDailyResultSchema.parse({
        path: filePath,
        ast,
        created
      })
    } catch (err) {
      const normalized = normalizeError(err, { vaultPath })
      const msg = `[IPC] note:daily handler error: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { path: '', ast: null, created: false, error: errorToString(normalized) }
    }
  })

  // -------------------------------------------------------------------------
  // note:random — open a random note from the vault
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_RANDOM, async (_event, rawPayload) => {
    const validation = NoteRandomSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:random validation failed: ${reason}`)
      return { error: reason }
    }
    const { vaultPath, tagFilter } = validation.data
    try {
      const vault = stateManager.getCurrentVault()
      if (!vault || vault.path !== vaultPath) {
        return { error: 'Vault not open' }
      }
      const files = vault.files ?? []
      let candidates = files
      if (tagFilter && tagFilter.length > 0) {
        const tagPaths = stateManager.getExtendedIndex()?.tagIndex?.get(tagFilter)
        if (tagPaths) {
          candidates = files.filter((f) => tagPaths.has(f.path))
        } else {
          candidates = []
        }
      }
      if (candidates.length === 0) {
        return { error: 'No notes found in vault' }
      }
      const randomFile = candidates[Math.floor(Math.random() * candidates.length)]
      const result = await stateManager.getAST(randomFile.path)
      return NoteRandomResultSchema.parse({
        path: randomFile.path,
        ast: result
      })
    } catch (err) {
      const normalized = normalizeError(err, { vaultPath, tagFilter })
      const msg = `[IPC] note:random error: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { error: errorToString(normalized) }
    }
  })

  // -------------------------------------------------------------------------
  // templates:list — list all templates in the vault's _templates directory
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.TEMPLATES_LIST, async (_event, rawPayload) => {
    const validation = TemplatesListSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] templates:list validation failed: ${reason}`)
      return { templates: [] }
    }

    const { vaultPath } = validation.data
    const templatesDir = path.join(vaultPath, '_templates')

    try {
      await fs.access(templatesDir)
    } catch {
      return { templates: [] }
    }

    try {
      const dirents = await fs.readdir(templatesDir, { withFileTypes: true })
      const mdFiles = dirents.filter((d) => d.isFile() && d.name.endsWith('.md'))

      const templates = await Promise.all(
        mdFiles.map(async (dirent) => {
          const templatePath = path.join(templatesDir, dirent.name)
          const content = await fs.readFile(templatePath, 'utf-8')
          const name = path.basename(dirent.name, '.md')
          return { name, path: templatePath, content }
        })
      )

      return TemplatesListResultSchema.parse({ templates })
    } catch (err) {
      const normalized = normalizeError(err, { vaultPath })
      const msg = `[IPC] templates:list handler error for vault "${vaultPath}": ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { templates: [] }
    }
  })

  // -------------------------------------------------------------------------
  // view-state:get-fold — get fold state for a heading
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VIEW_STATE_GET_FOLD, async (_event, rawPayload) => {
    const validation = ViewStateGetFoldSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] view-state:get-fold validation failed: ${reason}`)
      return true // Default to open
    }

    const { vaultPath, notePath, headingId } = validation.data

    try {
      const state = await loadViewState(vaultPath, notePath)
      return state.foldStates[headingId] ?? true
    } catch (err) {
      const normalized = normalizeError(err, { vaultPath, notePath, headingId })
      const msg = `[IPC] view-state:get-fold handler error: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return true // Default to open on error
    }
  })

  // -------------------------------------------------------------------------
  // view-state:set-fold — set fold state for a heading
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VIEW_STATE_SET_FOLD, async (_event, rawPayload) => {
    const validation = ViewStateSetFoldSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] view-state:set-fold validation failed: ${reason}`)
      return
    }

    const { vaultPath, notePath, headingId, isOpen } = validation.data

    try {
      await setFoldState(vaultPath, notePath, headingId, isOpen)
    } catch (err) {
      const normalized = normalizeError(err, { vaultPath, notePath, headingId })
      const msg = `[IPC] view-state:set-fold handler error: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
    }
  })

  // -------------------------------------------------------------------------
  // properties:read — read YAML frontmatter properties from a file
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.PROPERTIES_READ, async (_event, rawPayload) => {
    const validation = PropertiesReadSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] properties:read validation failed: ${reason}`)
      return { path: '', properties: {}, yaml: '' }
    }

    const { path: filePath } = validation.data

    try {
      const content = await fs.readFile(filePath, 'utf-8')
      const { yaml, parsed } = extractFrontmatter(content)
      return PropertiesReadResultSchema.parse({
        path: filePath,
        properties: parsed,
        yaml
      })
    } catch (err) {
      const normalized = normalizeError(err, { path: filePath })
      const msg = `[IPC] properties:read handler error for "${filePath}": ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { path: filePath, properties: {}, yaml: '' }
    }
  })

  // -------------------------------------------------------------------------
  // properties:write — rewrite YAML frontmatter properties for a file
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.PROPERTIES_WRITE, async (_event, rawPayload) => {
    const validation = PropertiesWriteSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] properties:write validation failed: ${reason}`)
      return PropertiesWriteResultSchema.parse({ success: false, error: reason })
    }

    const { path: filePath, yaml: newYaml } = validation.data

    try {
      const yaml = await import('yaml')
      yaml.parse(newYaml)
    } catch (err) {
      const reason = `Invalid YAML: ${err instanceof Error ? err.message : String(err)}`
      emitActivityLog('warn', `[IPC] properties:write rejected: ${reason}`)
      return PropertiesWriteResultSchema.parse({ success: false, error: reason })
    }

    try {
      const content = await fs.readFile(filePath, 'utf-8')
      const newContent = replaceFrontmatterRaw(content, newYaml)

      stateManager.setPendingWrite(filePath)
      await fs.writeFile(filePath, newContent, 'utf-8')
      stateManager.invalidateAST(filePath)
      stateManager.clearPendingWrite(filePath)

      return PropertiesWriteResultSchema.parse({ success: true })
    } catch (err) {
      stateManager.clearPendingWrite(filePath)
      const normalized = normalizeError(err, { path: filePath })
      const msg = `[IPC] properties:write error for "${filePath}": ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return PropertiesWriteResultSchema.parse({ success: false, error: errorToString(normalized) })
    }
  })

  // -------------------------------------------------------------------------
  // note:compose — merge multiple notes into one
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_COMPOSE, async (_event, rawPayload) => {
    const validation = NoteComposeSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:compose validation failed: ${reason}`)
      return { previewMarkdown: '', warning: reason }
    }

    const { vaultPath, sourcePaths } = validation.data

    try {
      const vault = stateManager.getCurrentVault()
      if (!vault || vault.path !== vaultPath) {
        return { previewMarkdown: '', warning: 'Vault not open' }
      }

      const result = await mergeNotes(sourcePaths, vaultPath, vault.files)
      return NoteComposeResultSchema.parse(result)
    } catch (err) {
      const normalized = normalizeError(err, { vaultPath, sourcePaths })
      const msg = `[IPC] note:compose handler error: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { previewMarkdown: '', warning: errorToString(normalized) }
    }
  })

  // -------------------------------------------------------------------------
  // note:unique — create a note with a unique timestamp name
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_UNIQUE, async (_event, rawPayload) => {
    const validation = NoteUniqueSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:unique validation failed: ${reason}`)
      return { path: '', error: reason }
    }

    const { vaultPath } = validation.data

    try {
      const now = new Date()
      const uniqueName = generateUniqueNoteName('YYYYMMDDHHmmss', now)
      const filePath = path.join(vaultPath, `${uniqueName}.md`)

      try {
        await fs.access(filePath)
        return { path: '', error: 'Note with that name already exists' }
      } catch {
        // File doesn't exist, proceed
      }

      const content = `# ${uniqueName}\n\n`
      stateManager.setPendingWrite(filePath)
      try {
        await fs.writeFile(filePath, content, 'utf-8')
      } finally {
        stateManager.clearPendingWrite(filePath)
      }

      const ast = await stateManager.getAST(filePath)
      return NoteUniqueResultSchema.parse({ path: filePath, ast })
    } catch (err) {
      const normalized = normalizeError(err, { vaultPath })
      const msg = `[IPC] note:unique handler error: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { path: '', error: errorToString(normalized) }
    }
  })
}
