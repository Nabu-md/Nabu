/**
 * settings.ts — Settings feature IPC module.
 *
 * Owns settings:get, settings:set, settings:getFeatureToggles,
 * settings:setFeatureToggle, activity:log, and the open:settings push channel
 * (registered in index.ts menu wiring, not here).
 *
 * This is a pure structural extraction from the previous monolithic
 * `src/main/ipc.ts`. Handler behavior is unchanged.
 */

import { ipcMain } from 'electron'

import { IPCChannel } from '@shared/channels'
import {
  ActivityLogSchema,
  SettingsGetSchema,
  SettingsSetSchema,
  SetFeatureToggleSchema,
  FeatureTogglesResultSchema,
  SetFeatureToggleResultSchema
} from '@shared/schemas'

import { loadSettings, saveSettings } from '../services/settings'

import type { IPCContext } from './context'
import {
  emitActivityLog,
  formatZodError,
  getWidgetToggleCallback,
  normalizeError,
  errorToString
} from './shared'

/**
 * Register all settings-feature IPC handlers.
 */
export function registerSettingsIPC(_ctx: IPCContext): void {
  // -------------------------------------------------------------------------
  // activity:log — receive log entries from the renderer
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.ACTIVITY_LOG, async (_event, rawPayload) => {
    const validation = ActivityLogSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      console.warn(`[IPC] activity:log validation failed: ${reason}`)
      return { error: reason }
    }

    const { level, message } = validation.data
    console[level](`[Renderer] ${message}`)
    return { success: true }
  })

  // -------------------------------------------------------------------------
  // settings:get — retrieve a single settings value by key
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.SETTINGS_GET, async (_event, rawPayload) => {
    const validation = SettingsGetSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] settings:get validation failed: ${reason}`)
      return { success: false, error: reason }
    }

    const { key } = validation.data

    try {
      const settings = await loadSettings()
      const value = (settings as unknown as Record<string, unknown>)[key]
      return { value }
    } catch (err) {
      const normalized = normalizeError(err, { key })
      const msg = `[IPC] settings:get handler error for key "${key}": ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('warn', msg)
      return { success: false, error: errorToString(normalized) }
    }
  })

  // -------------------------------------------------------------------------
  // settings:set — update a single settings value by key
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.SETTINGS_SET, async (_event, rawPayload) => {
    const validation = SettingsSetSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] settings:set validation failed: ${reason}`)
      return { success: false, error: reason }
    }

    const { key, value } = validation.data

    try {
      const settings = await loadSettings()
      const updated = { ...settings, [key]: value }
      await saveSettings(updated)
      return { success: true }
    } catch (err) {
      const normalized = normalizeError(err, { key })
      const msg = `[IPC] settings:set handler error for key "${key}": ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('warn', msg)
      return { success: false, error: errorToString(normalized) }
    }
  })

  // -------------------------------------------------------------------------
  // settings:getFeatureToggles — get all feature toggles for the Settings UI
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.SETTINGS_GET_FEATURE_TOGGLES, async (_event) => {
    try {
      const { getFeatureToggles, getDefaultState } = await import('@shared/feature-toggles')
      const toggles = getFeatureToggles()
      const result = toggles.map((t) => ({
        ...t,
        enabled: getDefaultState(t.id)
      }))
      return FeatureTogglesResultSchema.parse({ toggles: result })
    } catch (err) {
      const normalized = normalizeError(err)
      const msg = `[IPC] settings:getFeatureToggles error: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { toggles: [] }
    }
  })

  // -------------------------------------------------------------------------
  // settings:setFeatureToggle — toggle a feature on/off
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.SETTINGS_SET_FEATURE_TOGGLE, async (_event, rawPayload) => {
    const validation = SetFeatureToggleSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] settings:setFeatureToggle validation failed: ${reason}`)
      return SetFeatureToggleResultSchema.parse({ success: false, error: reason })
    }

    const { id, enabled } = validation.data

    try {
      const { setFeatureEnabled } = await import('@shared/feature-toggles')
      setFeatureEnabled(id, enabled)

      // Notify the widget manager when clipboard-widget toggles
      if (id === 'clipboard-widget') {
        const cb = getWidgetToggleCallback()
        if (cb) cb(enabled)
      }

      return SetFeatureToggleResultSchema.parse({ success: true })
    } catch (err) {
      const normalized = normalizeError(err, { id })
      const msg = `[IPC] settings:setFeatureToggle error for "${id}": ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return SetFeatureToggleResultSchema.parse({
        success: false,
        error: errorToString(normalized)
      })
    }
  })
}
