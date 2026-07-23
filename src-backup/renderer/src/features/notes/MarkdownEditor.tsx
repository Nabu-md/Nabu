/**
 * MarkdownEditor.tsx
 *
 * CodeMirror 6 wrapper component for markdown editing.
 * Supports Live Preview mode alongside the standard edit mode.
 *
 * Requirements: 23.1, 23.2, 0b
 */

import React, { useCallback, useMemo } from 'react'
import CodeMirror from '@uiw/react-codemirror'
import { markdown } from '@codemirror/lang-markdown'
import { EditorView } from '@codemirror/view'
import { Extension } from '@codemirror/state'
import { FindReplaceBar } from './FindReplaceBar'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface MarkdownEditorProps {
  /** The markdown content to edit */
  value: string
  /** Callback when content changes */
  onChange: (value: string) => void
  /** Whether the editor is read-only (for Live Preview) */
  readOnly?: boolean
  /** Placeholder text when empty */
  placeholder?: string
  /** Show find/replace bar */
  showFindReplace?: boolean
  /** Callback to toggle find/replace visibility */
  onToggleFindReplace?: (visible: boolean) => void
}

// ---------------------------------------------------------------------------
// Theme - Nabu dark theme using CSS variables
// ---------------------------------------------------------------------------

const nabuDarkTheme = EditorView.theme({
  '&': {
    backgroundColor: 'var(--nabu-bg, #0a0a0a)',
    color: 'var(--nabu-text, #e5e5e5)',
    fontFamily: 'var(--nabu-font-mono, monospace)',
    fontSize: '14px'
  },
  '.cm-content': {
    caretColor: 'var(--nabu-accent, #60a5fa)'
  },
  '.cm-cursor': {
    borderLeft: 'var(--nabu-accent, #60a5fa)'
  },
  '&.cm-focused .cm-cursor': {
    borderLeft: 'var(--nabu-accent, #60a5fa)'
  },
  '.cm-selectionBackground': {
    backgroundColor: 'rgba(96, 165, 250, 0.2)'
  },
  '.cm-activeLine': {
    backgroundColor: 'rgba(255, 255, 255, 0.05)'
  },
  '.cm-gutters': {
    backgroundColor: 'var(--nabu-sidebar-bg, #121212)',
    border: 'none'
  },
  '.cm-gutter': {
    color: 'var(--nabu-text-secondary, #a3a3a3)'
  }
})

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export const MarkdownEditor: React.FC<MarkdownEditorProps> = ({
  value,
  onChange,
  readOnly = false,
  showFindReplace = false,
  onToggleFindReplace
}) => {
  // Create extensions once
  const extensions = useMemo<Extension[]>(() => {
    return [
      markdown(),
      nabuDarkTheme,
      EditorView.lineWrapping,
      readOnly ? EditorView.editable.of(false) : []
    ]
  }, [readOnly])

  const handleChange = useCallback(
    (val: string) => {
      onChange(val)
    },
    [onChange]
  )

  // Handle find/replace operations
  const handleReplace = useCallback((find: string, replace: string, replaceAll = false) => {
    if (replaceAll) {
      const escaped = find.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
      const regex = new RegExp(escaped, 'gi')
      onChange(value.replace(regex, replace))
    } else {
      // For single replace, we just replace the first occurrence
      const escaped = find.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
      const regex = new RegExp(escaped, 'i')
      onChange(value.replace(regex, replace))
    }
  }, [value, onChange])

  const handleCloseFindReplace = useCallback(() => {
    onToggleFindReplace?.(false)
  }, [onToggleFindReplace])

  return (
    <div className="markdown-editor flex flex-col h-full">
      {showFindReplace && (
        <FindReplaceBar
          value={value}
          onReplace={handleReplace}
          onClose={handleCloseFindReplace}
        />
      )}
      <div className="flex-1">
        <CodeMirror
          value={value}
          height="100%"
          extensions={extensions}
          onChange={handleChange}
          editable={!readOnly}
          basicSetup={{
            lineNumbers: false,
            highlightActiveLine: true
          }}
        />
      </div>
    </div>
  )
}
