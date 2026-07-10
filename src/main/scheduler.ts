/**
 * scheduler.ts
 *
 * Local reminder scheduler for Nabu (Phase 3).
 * Parses task list items for ⏰ time reminders and schedules native OS notifications.
 *
 * Requirements: Phase 3 (Recurring Tasks & Reminders)
 */

import { Notification, app } from 'electron'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ScheduledReminder {
  filePath: string
  lineIndex: number
  taskText: string
  date?: string
  time?: string
  timeoutId?: NodeJS.Timeout | null
}

// ---------------------------------------------------------------------------
// Globals
// ---------------------------------------------------------------------------

const reminders = new Map<string, ScheduledReminder>()
const DATE_REGEX = /📅\s*(\d{4}-\d{2}-\d{2})/
const TIME_REGEX = /⏰\s*(\d{2}:\d{2})/

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Parse reminder info from task text.
 */
function parseReminder(text: string): { date?: string; time?: string } | null {
  const dateMatch = text.match(DATE_REGEX)
  const timeMatch = text.match(TIME_REGEX)
  if (!dateMatch && !timeMatch) return null
  return {
    date: dateMatch?.[1],
    time: timeMatch?.[1]
  }
}

/**
 * Calculate milliseconds until a specific date/time.
 */
function msUntil(dateStr: string, timeStr?: string): number {
  const today = new Date()
  const [year, month, day] = dateStr.split('-').map(Number)
  const target = new Date(year, month - 1, day)

  if (timeStr) {
    const [hours, minutes] = timeStr.split(':').map(Number)
    target.setHours(hours, minutes, 0, 0)
  } else {
    target.setHours(9, 0, 0, 0) // Default to 9 AM if no time specified
  }

  return target.getTime() - today.getTime()
}

/**
 * Show a native OS notification for a reminder.
 */
function showReminder(filePath: string, taskText: string): void {
  const noteName = filePath.split('/').pop()?.replace('.md', '') ?? 'Note'
  const notif = new Notification({
    title: '🔔 Reminder',
    body: `"${taskText}" in ${noteName}`,
    icon: undefined // Use app icon
  })

  notif.show()

  // On macOS, clicking the notification opens the note
  notif.on('click', () => {
    // Send IPC to open the note - this would be implemented via app focus
    app.focus({ steal: true })
  })
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Register a reminder from a task list item.
 */
export function registerReminder(filePath: string, lineIndex: number, taskText: string): void {
  const reminder = parseReminder(taskText)
  if (!reminder?.date && !reminder?.time) return

  // Cancel existing reminder if present
  cancelReminder(filePath, lineIndex)

  const targetMs = msUntil(reminder.date ?? '', reminder.time)
  if (targetMs <= 0) return // Already passed

  const timeoutId = setTimeout(() => {
    showReminder(filePath, taskText)
    reminders.delete(`${filePath}:${lineIndex}`)
  }, targetMs)

  reminders.set(`${filePath}:${lineIndex}`, {
    filePath,
    lineIndex,
    taskText,
    date: reminder.date,
    time: reminder.time,
    timeoutId
  })
}

/**
 * Cancel a specific reminder.
 */
export function cancelReminder(filePath: string, lineIndex: number): void {
  const key = `${filePath}:${lineIndex}`
  const reminder = reminders.get(key)
  if (reminder?.timeoutId) {
    clearTimeout(reminder.timeoutId)
  }
  reminders.delete(key)
}

/**
 * Clear all scheduled reminders.
 */
export function clearAllReminders(): void {
  for (const reminder of reminders.values()) {
    if (reminder.timeoutId) {
      clearTimeout(reminder.timeoutId)
    }
  }
  reminders.clear()
}

/**
 * Get all active reminders (for debugging).
 */
export function getReminders(): ScheduledReminder[] {
  return Array.from(reminders.values()).map(r => ({ ...r, timeoutId: undefined }))
}

/**
 * Parse all task lists in the vault and register reminders.
 * Called when vault loads.
 */
export async function scanAndRegisterReminders(
  scanTasksCallback: () => Array<{ filePath: string; lineIndex: number; taskText: string }>
): Promise<void> {
  const tasks = scanTasksCallback()
  for (const task of tasks) {
    registerReminder(task.filePath, task.lineIndex, task.taskText)
  }
}