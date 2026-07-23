import React from 'react'
import clsx from 'clsx'
import { useWidgetActivity, type ActivityEntry } from './widgetService'

// ---------------------------------------------------------------------------
// External badge
// ---------------------------------------------------------------------------

function ExternalBadge(): React.JSX.Element {
  return (
    <span
      className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-semibold uppercase tracking-wide bg-blue-500/20 text-blue-400 shrink-0"
      aria-label="External edit"
    >
      External
    </span>
  )
}

// ---------------------------------------------------------------------------
// Single timeline entry
// ---------------------------------------------------------------------------

interface EntryRowProps {
  entry: ActivityEntry
}

function EntryRow({ entry }: EntryRowProps): React.JSX.Element {
  const localTime = new Date(entry.timestamp).toLocaleTimeString()
  // Show only the filename portion for brevity; full path in title for accessibility
  const safePath = entry.filePath ?? ''
  const fileName = safePath.split(/[\\/]/).pop() ?? safePath

  return (
    <div
      className={clsx(
        'entry flex items-center gap-2 px-3 py-1.5 rounded hover:bg-white/[0.04] transition-colors text-xs',
        entry.isExternal && 'external'
      )}
      title={entry.filePath}
    >
      {/* Timestamp */}
      <time
        dateTime={new Date(entry.timestamp).toISOString()}
        className="text-white/40 shrink-0 tabular-nums"
      >
        {localTime}
      </time>

      {/* File path */}
      <span className="text-white/70 truncate flex-1 min-w-0" title={entry.filePath}>
        {fileName}
      </span>

      {/* External badge — only for external edits */}
      {entry.isExternal && <ExternalBadge />}
    </div>
  )
}

// ---------------------------------------------------------------------------
// ActivityTimeline
// ---------------------------------------------------------------------------

export function ActivityTimeline(): React.JSX.Element {
  const activityLog = useWidgetActivity()

  return (
    <aside
      className="activity-timeline flex flex-col w-full bg-[var(--ev-c-black-soft)] border-t border-white/10"
      aria-label="Activity timeline"
    >
      {/* Header */}
      <header className="flex items-center px-3 py-1.5 border-b border-white/10 shrink-0">
        <span className="text-[10px] font-semibold text-white/40 uppercase tracking-wide select-none">
          Activity
        </span>
        {activityLog.length > 0 && (
          <span className="ml-2 text-[10px] text-white/30 select-none tabular-nums">
            {activityLog.length}
          </span>
        )}
      </header>

      {/* Entries */}
      <div
        className="overflow-y-auto"
        style={{ maxHeight: '120px' }}
        aria-live="polite"
        aria-label="Recent file change events"
        role="log"
      >
        {activityLog.length === 0 ? (
          <p className="px-3 py-2 text-xs text-white/25 select-none">No recent activity</p>
        ) : (
          activityLog.map((entry) => (
            <EntryRow key={`${entry.filePath}-${entry.timestamp}`} entry={entry} />
          ))
        )}
      </div>
    </aside>
  )
}
