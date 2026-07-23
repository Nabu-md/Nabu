/**
 * SlashCommands.tsx
 *
 * Inline autocomplete menu triggered by typing `/` at line start.
 * Inserts markdown syntax for common constructs.
 * Visual command palette with categories and previews (Phase 1).
 *
 * Requirements: 30.1, 30.2, 30.3, 30.4, 30.5, 30.6, 30.7, 1a, 1b, 1c, 1d
 */

import React, { useState, useEffect, useMemo } from 'react'

// ---------------------------------------------------------------------------
// Date/Time helpers for /date and /time commands
// ---------------------------------------------------------------------------

function formatDate(date: Date): string {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

function formatTime(date: Date): string {
  const hours = String(date.getHours()).padStart(2, '0')
  const minutes = String(date.getMinutes()).padStart(2, '0')
  return `${hours}:${minutes}`
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface SlashCommand {
  id: string
  label: string
  keywords: string
  insertText: string
  icon?: string
  category: string
}

// Category order for display
const CATEGORY_ORDER: SlashCommand['category'][] = [
  'Headings',
  'Lists',
  'Blocks',
  'Inline',
  'Templates'
]

// ---------------------------------------------------------------------------
// Command definitions
// ---------------------------------------------------------------------------

const SLASH_COMMANDS: SlashCommand[] = [
  // Headings
  { id: 'heading', label: 'Heading 1', keywords: 'h1 heading', insertText: '# ', icon: '📝', category: 'Headings' },
  { id: 'heading2', label: 'Heading 2', keywords: 'h2 heading', insertText: '## ', icon: '📝', category: 'Headings' },
  { id: 'heading3', label: 'Heading 3', keywords: 'h3 heading', insertText: '### ', icon: '📝', category: 'Headings' },
  // Lists
  { id: 'bullet-list', label: 'Bullet List', keywords: 'list ul', insertText: '- ', icon: '📋', category: 'Lists' },
  { id: 'numbered-list', label: 'Numbered List', keywords: 'list ol', insertText: '1. ', icon: '📋', category: 'Lists' },
  { id: 'task-list', label: 'Task List', keywords: 'task todo checkbox', insertText: '- [ ] ', icon: '☑️', category: 'Lists' },
  // Blocks
  { id: 'callout', label: 'Callout', keywords: 'callout note', insertText: '> [!note] ', icon: '📓', category: 'Blocks' },
  { id: 'code-block', label: 'Code Block', keywords: 'code fence', insertText: '```\n\n```', icon: '💻', category: 'Blocks' },
  { id: 'math-block', label: 'Math Block', keywords: 'math katex', insertText: '$$\n\n$$', icon: '∑', category: 'Blocks' },
  {
    id: 'table',
    label: 'Table',
    keywords: 'table grid',
    insertText: '| Header 1 | Header 2 |\n| --- | --- |\n| Cell 1 | Cell 2 |\n',
    icon: '📊',
    category: 'Blocks'
  },
  {
    id: 'toggle',
    label: 'Toggle',
    keywords: 'toggle details fold',
    insertText: '> [!toggle] \n\n> ',
    icon: '🔽',
    category: 'Blocks'
  },
  {
    id: 'kanban',
    label: 'Kanban Board',
    keywords: 'kanban board tasks',
    insertText: '%%kanban%%\nstatuses: [Backlog, In Progress, Done]',
    icon: '📋',
    category: 'Blocks'
  },
  // Inline
  {
    id: 'date',
    label: 'Date',
    keywords: 'date today',
    insertText: formatDate(new Date()),
    icon: '📅',
    category: 'Inline'
  },
  {
    id: 'time',
    label: 'Time',
    keywords: 'time now',
    insertText: formatTime(new Date()),
    icon: '⏰',
    category: 'Inline'
  },
  {
    id: 'reminder',
    label: 'Reminder',
    keywords: 'reminder date time',
    insertText: `📅 ${formatDate(new Date())} ⏰ `,
    icon: '⏳',
    category: 'Inline'
  },
  { id: 'embed', label: 'Embed', keywords: 'embed transclude', insertText: '![[note]]', icon: '🔗', category: 'Inline' },
  { id: 'divider', label: 'Divider', keywords: 'hr rule divider', insertText: '---\n', icon: '➖', category: 'Inline' },
  // Templates
  { id: 'template', label: 'Template', keywords: 'template insert', insertText: '{{title}}', icon: '📄', category: 'Templates' }
]

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

interface SlashCommandsProps {
  onInsert: (text: string) => void
  onClose: () => void
}

export function SlashCommands({ onInsert, onClose }: SlashCommandsProps): React.JSX.Element {
  const [query, setQuery] = useState('')
  const [selectedIndex, setSelectedIndex] = useState(0)
  const [hoveredCommand, setHoveredCommand] = useState<SlashCommand | null>(null)

  const filtered = SLASH_COMMANDS.filter(
    (cmd) =>
      cmd.label.toLowerCase().includes(query.toLowerCase()) ||
      cmd.keywords.toLowerCase().includes(query.toLowerCase())
  )

  // Group filtered commands by category
  const groupedCommands = useMemo(() => {
    const groups: Record<string, SlashCommand[]> = {}
    for (const cmd of filtered) {
      const cat = cmd.category || 'Other'
      if (!groups[cat]) groups[cat] = []
      groups[cat].push(cmd)
    }
    return groups
  }, [filtered])

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setSelectedIndex((i) => Math.min(i + 1, filtered.length - 1))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setSelectedIndex((i) => Math.max(i - 1, 0))
    } else if (e.key === 'Enter') {
      e.preventDefault()
      if (filtered[selectedIndex]) {
        onInsert(filtered[selectedIndex].insertText)
      }
    } else if (e.key === 'Escape') {
      e.preventDefault()
      onClose()
    }
  }

  useEffect(() => {
    setSelectedIndex(0)
  }, [query])

  // Render preview for hovered/selected command
  const renderPreview = (cmd: SlashCommand | null) => {
    if (!cmd) return null

    // Get the command at selected index
    const command = filtered[selectedIndex] || cmd

    if (command.category === 'Headings') {
      const depth = command.id === 'heading' ? 1 : command.id === 'heading2' ? 2 : 3
      const Tag = `h${depth}` as keyof React.JSX.IntrinsicElements
      const classMap: Record<number, string> = {
        1: 'text-xl font-bold text-white/90',
        2: 'text-lg font-semibold text-white/85',
        3: 'text-base font-semibold text-white/80'
      }
      return (
        <div className="p-2">
          <Tag className={classMap[depth]}>Heading {depth} preview</Tag>
        </div>
      )
    }

    if (command.id === 'table') {
      return (
        <div className="p-2 overflow-x-auto">
          <table className="text-xs border-collapse min-w-full">
            <thead>
              <tr>
                <th className="border border-white/20 px-2 py-1 bg-white/5">Header 1</th>
                <th className="border border-white/20 px-1 py-1 bg-white/5">Header 2</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td className="border border-white/10 px-2 py-1">Cell 1</td>
                <td className="border border-white/10 px-1 py-1">Cell 2</td>
              </tr>
            </tbody>
          </table>
        </div>
      )
    }

    if (command.id === 'code-block') {
      return (
        <div className="p-2 font-mono text-xs">
          <pre className="bg-white/5 p-1 rounded">{"code block\n"}</pre>
        </div>
      )
    }

    if (command.id === 'callout') {
      return (
        <div className="p-2 text-xs">
          <div className="border-l-2 border-blue-500 pl-2 bg-blue-950/20 py-1">
            ℹ️ Note: Callout content preview
          </div>
        </div>
      )
    }

    if (command.id === 'bullet-list' || command.id === 'numbered-list' || command.id === 'task-list') {
      const isOrdered = command.id === 'numbered-list'
      return (
        <div className="p-2 text-xs">
          <ul className={isOrdered ? 'list-decimal pl-4' : 'list-disc pl-4'}>
            <li>Item 1</li>
            <li>Item 2</li>
          </ul>
        </div>
      )
    }

    // Default preview
    return (
      <div className="p-2 text-xs text-white/60">
        <code>{command.insertText.substring(0, 40)}</code>
      </div>
    )
  }

  return (
    <div
      className="slash-commands flex absolute z-50 bg-nabu-bg border border-nabu-border rounded-lg shadow-lg max-h-64 overflow-hidden animate-in fade-in slide-in-from-bottom-2 duration-150"
      onKeyDown={handleKeyDown}
      role="dialog"
      aria-label="Slash commands"
    >
      {/* Left: Command list */}
      <div className="w-56 max-h-64 overflow-y-auto">
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search commands..."
          className="w-full px-3 py-2 text-sm bg-transparent border-b border-nabu-border outline-none sticky top-0"
          autoFocus
        />
        {query === '' ? (
          // Show grouped view when not searching
          CATEGORY_ORDER.map((category) => {
            const commands = groupedCommands[category]
            if (!commands || commands.length === 0) return null
            return (
              <div key={category}>
                <div className="px-2 py-1 text-xs font-semibold text-nabu-text-muted uppercase tracking-wider">
                  {category}
                </div>
                {commands.map((cmd) => {
                  const globalIdx = filtered.indexOf(cmd)
                  return (
                    <button
                      key={cmd.id}
                      type="button"
                      onMouseEnter={() => setHoveredCommand(cmd)}
                      onClick={() => onInsert(cmd.insertText)}
                      className={`w-full flex items-center gap-2 px-3 py-1.5 text-left text-sm hover:bg-nabu-bg-mute transition-colors ${
                        globalIdx === selectedIndex ? 'bg-nabu-accent/20' : ''
                      }`}
                    >
                      <span className="text-base w-5">{cmd.icon}</span>
                      <span className="flex-1">{cmd.label}</span>
                    </button>
                  )
                })}
              </div>
            )
          })
        ) : (
          // Show flat list when searching
          filtered.map((cmd, i) => (
            <button
              key={cmd.id}
              type="button"
              onMouseEnter={() => setHoveredCommand(cmd)}
              onClick={() => onInsert(cmd.insertText)}
              className={`w-full flex items-center gap-2 px-3 py-1.5 text-left text-sm hover:bg-nabu-bg-mute transition-colors ${
                i === selectedIndex ? 'bg-nabu-accent/20' : ''
              }`}
            >
              <span className="text-base w-5">{cmd.icon}</span>
              <span className="flex-1">{cmd.label}</span>
            </button>
          ))
        )}
        {filtered.length === 0 && query !== '' && (
          <div className="px-3 py-2 text-sm text-nabu-text-muted">No commands found</div>
        )}
      </div>

      {/* Right: Preview panel */}
      <div className="w-48 border-l border-nabu-border bg-nabu-bg-mute hidden sm:block max-h-64 overflow-y-auto">
        <div className="px-2 py-1 text-xs font-semibold text-nabu-text-muted border-b border-nabu-border">
          Preview
        </div>
        {renderPreview(hoveredCommand || filtered[selectedIndex] || null)}
      </div>
    </div>
  )
}