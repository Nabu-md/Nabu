/**
 * SlashCommands.tsx
 *
 * Inline autocomplete menu triggered by typing `/` at line start.
 * Inserts markdown syntax for common constructs.
 *
 * Requirements: 30.1, 30.2, 30.3, 30.4, 30.5, 30.6, 30.7
 */

import React, { useState, useEffect, useRef } from 'react'

interface SlashCommand {
  id: string
  label: string
  keywords: string
  insertText: string
}

const SLASH_COMMANDS: SlashCommand[] = [
  { id: 'heading', label: 'Heading 1', keywords: 'h1 heading', insertText: '# ' },
  { id: 'heading2', label: 'Heading 2', keywords: 'h2 heading', insertText: '## ' },
  { id: 'heading3', label: 'Heading 3', keywords: 'h3 heading', insertText: '### ' },
  { id: 'bullet-list', label: 'Bullet List', keywords: 'list ul', insertText: '- ' },
  { id: 'numbered-list', label: 'Numbered List', keywords: 'list ol', insertText: '1. ' },
  { id: 'task-list', label: 'Task List', keywords: 'task todo checkbox', insertText: '- [ ] ' },
  { id: 'callout', label: 'Callout', keywords: 'callout note', insertText: '> [!note] ' },
  { id: 'code-block', label: 'Code Block', keywords: 'code fence', insertText: '```\n\n```' },
  { id: 'math-block', label: 'Math Block', keywords: 'math katex', insertText: '$$\n\n$$' },
  { id: 'table', label: 'Table', keywords: 'table grid', insertText: '| Header 1 | Header 2 |\n| --- | --- |\n| Cell 1 | Cell 2 |\n' },
  { id: 'horizontal-rule', label: 'Horizontal Rule', keywords: 'hr rule divider', insertText: '---\n' },
  { id: 'embed', label: 'Embed', keywords: 'embed transclude', insertText: '![[note]]' },
]

interface SlashCommandsProps {
  onInsert: (text: string) => void
  onClose: () => void
}

export function SlashCommands({ onInsert, onClose }: SlashCommandsProps): React.JSX.Element {
  const [query, setQuery] = useState('')
  const [selectedIndex, setSelectedIndex] = useState(0)

  const filtered = SLASH_COMMANDS.filter(cmd =>
    cmd.label.toLowerCase().includes(query.toLowerCase()) ||
    cmd.keywords.toLowerCase().includes(query.toLowerCase()),
  )

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setSelectedIndex(i => Math.min(i + 1, filtered.length - 1))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setSelectedIndex(i => Math.max(i - 1, 0))
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

  return (
    <div
      className="slash-commands absolute z-50 bg-nabu-bg border border-nabu-border rounded shadow-lg max-h-64 overflow-y-auto"
      onKeyDown={handleKeyDown}
    >
      <input
        type="text"
        value={query}
        onChange={e => setQuery(e.target.value)}
        placeholder="Search commands..."
        className="w-full px-2 py-1 text-sm bg-transparent border-b border-nabu-border outline-none"
        autoFocus
      />
      {filtered.map((cmd, i) => (
        <button
          key={cmd.id}
          type="button"
          onClick={() => onInsert(cmd.insertText)}
          className={`w-full px-2 py-1 text-left text-sm hover:bg-nabu-bg-mute ${
            i === selectedIndex ? 'bg-nabu-accent/20' : ''
          }`}
        >
          {cmd.label}
        </button>
      ))}
      {filtered.length === 0 && (
        <div className="px-2 py-1 text-sm text-nabu-text-muted">No commands found</div>
      )}
    </div>
  )
}