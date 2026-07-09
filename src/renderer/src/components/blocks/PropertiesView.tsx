/**
 * PropertiesView.tsx
 *
 * Renders YAML frontmatter as a two-column editable table, with a toggle
 * to switch between table view and raw-YAML textarea editor.
 *
 * Requirements: 12.1, 12.2, 12.3, 12.5, 12.6, 12.7
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { parse, stringify, Document, YAMLMap, YAMLSeq, Scalar } from 'yaml'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface PropertiesViewProps {
  /** The raw YAML string from the AST yaml node (or null/empty when none). */
  yamlValue: string | null
  /** Called when the user modifies properties, with the new YAML string. */
  onSave: (yamlString: string) => void
}

interface PropertyEntry {
  key: string
  value: unknown
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Convert a YAML value to its display string representation. */
function valueToString(val: unknown): string {
  if (val === null || val === undefined) return ''
  if (typeof val === 'boolean') return val ? 'true' : 'false'
  if (Array.isArray(val)) return val.map(String).join(', ')
  return String(val)
}

/** Parse a user-entered string back to a typed YAML value based on the original type. */
function parseTypedValue(input: string, originalType: 'string' | 'number' | 'boolean' | 'array'): unknown {
  const trimmed = input.trim()
  if (originalType === 'number') {
    const n = Number(trimmed)
    return Number.isNaN(n) ? trimmed : n
  }
  if (originalType === 'boolean') {
    if (trimmed.toLowerCase() === 'true') return true
    if (trimmed.toLowerCase() === 'false') return false
    return trimmed
  }
  if (originalType === 'array') {
    return trimmed.split(',').map((s) => s.trim()).filter(Boolean)
  }
  return trimmed // string
}

/** Detect the type category of a YAML value. */
function detectType(val: unknown): 'string' | 'number' | 'boolean' | 'array' | 'null' {
  if (val === null || val === undefined) return 'null'
  if (typeof val === 'boolean') return 'boolean'
  if (typeof val === 'number') return 'number'
  if (Array.isArray(val)) return 'array'
  return 'string'
}

// ---------------------------------------------------------------------------
// Input sub-component
// ---------------------------------------------------------------------------

interface ValueInputProps {
  value: unknown
  onChange: (newValue: unknown) => void
  onBlur: () => void
  autoFocus?: boolean
}

function ValueInput({ value, onChange, onBlur, autoFocus }: ValueInputProps): React.JSX.Element {
  const type = detectType(value)
  const strVal = valueToString(value)
  const [local, setLocal] = useState(strVal)

  // Reset local state when value changes externally
  useEffect(() => {
    setLocal(valueToString(value))
  }, [value])

  const handleKeyDown = (e: React.KeyboardEvent): void => {
    if (e.key === 'Enter') {
      onChange(parseTypedValue(local, type === 'null' ? 'string' : type))
      onBlur()
    }
    if (e.key === 'Escape') {
      setLocal(strVal) // revert
      onBlur()
    }
  }

  if (type === 'boolean') {
    return (
      <button
        type="button"
        onClick={() => onChange(!value)}
        onBlur={onBlur}
        autoFocus={autoFocus}
        className={`px-2 py-0.5 text-xs rounded border transition-colors ${
          value
            ? 'bg-emerald-900/30 border-emerald-700/40 text-emerald-400'
            : 'bg-white/5 border-white/10 text-white/40'
        }`}
      >
        {value ? 'true' : 'false'}
      </button>
    )
  }

  return (
    <input
      type={type === 'number' ? 'number' : 'text'}
      value={local}
      onChange={(e) => setLocal(e.target.value)}
      onBlur={() => {
        onChange(parseTypedValue(local, type === 'null' ? 'string' : type))
        onBlur()
      }}
      onKeyDown={handleKeyDown}
      autoFocus={autoFocus}
      className="w-full bg-transparent border-b border-white/20 text-sm text-white/90 outline-none focus:border-blue-400/60 transition-colors px-1 py-0.5"
    />
  )
}

// ---------------------------------------------------------------------------
// PropertiesView
// ---------------------------------------------------------------------------

export function PropertiesView({ yamlValue, onSave }: PropertiesViewProps): React.JSX.Element {
  const [entries, setEntries] = useState<PropertyEntry[]>([])
  const [editingKey, setEditingKey] = useState<string | null>(null)
  const [newKeyInput, setNewKeyInput] = useState('')
  const [showNewKeyInput, setShowNewKeyInput] = useState(false)
  const [collapsed, setCollapsed] = useState(false)
  const [rawMode, setRawMode] = useState(false)
  const [rawYamlText, setRawYamlText] = useState('')

  // Parse YAML value into entries whenever yamlValue prop changes
  useEffect(() => {
    if (!yamlValue || !yamlValue.trim()) {
      setEntries([])
      return
    }
    try {
      const parsed = parse(yamlValue)
      if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
        setEntries([])
        return
      }
      const ordered: PropertyEntry[] = []
      for (const [key, value] of Object.entries(parsed)) {
        ordered.push({ key, value })
      }
      setEntries(ordered)
    } catch {
      // If YAML is invalid, show nothing — raw-YAML edit is available
      setEntries([])
    }
  }, [yamlValue])

  /** Serialise the current entries back to a YAML string. */
  const serializeEntries = useCallback((items: PropertyEntry[]): string => {
    if (items.length === 0) return ''
    const obj: Record<string, unknown> = {}
    for (const { key, value } of items) {
      if (key.trim()) obj[key.trim()] = value
    }
    return stringify(obj)
  }, [])

  /** The raw YAML derived from current entries (for the raw editor). */
  const entriesRawYaml = useMemo(() => serializeEntries(entries), [entries, serializeEntries])

  /** Save current state and notify parent. */
  const saveEntries = useCallback(
    (items: PropertyEntry[]) => {
      const yaml = serializeEntries(items)
      onSave(yaml)
    },
    [onSave, serializeEntries],
  )

  /** Save raw YAML text. */
  const saveRawYaml = useCallback(() => {
    // Validate YAML before saving
    try {
      parse(rawYamlText)
    } catch {
      // Invalid — don't save, show no feedback for now
      return
    }
    onSave(rawYamlText)
    // Re-parse back into entries for table view
    try {
      const parsed = parse(rawYamlText)
      if (typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)) {
        const ordered: PropertyEntry[] = []
        for (const [key, value] of Object.entries(parsed)) {
          ordered.push({ key, value })
        }
        setEntries(ordered)
      }
    } catch {
      // ignore parse errors on save
    }
    setRawMode(false)
  }, [rawYamlText, onSave])

  /** Switch to raw mode, loading the current YAML text. */
  const enterRawMode = useCallback(() => {
    setRawYamlText(entriesRawYaml || yamlValue || '')
    setRawMode(true)
  }, [entriesRawYaml, yamlValue])

  /** Switch to table mode (no save — discard raw edits). */
  const exitRawMode = useCallback(() => {
    setRawMode(false)
  }, [])

  /** Update a single entry's value. */
  const handleValueChange = useCallback(
    (key: string, newValue: unknown) => {
      const updated = entries.map((e) =>
        e.key === key ? { ...e, value: newValue } : e,
      )
      setEntries(updated)
      saveEntries(updated)
    },
    [entries, saveEntries],
  )

  /** Update a single entry's key. */
  const handleKeyChange = useCallback(
    (oldKey: string, newKey: string) => {
      if (!newKey.trim() || newKey === oldKey) return
      const updated = entries.map((e) =>
        e.key === oldKey ? { ...e, key: newKey.trim() } : e,
      )
      setEntries(updated)
      setEditingKey(null)
      saveEntries(updated)
    },
    [entries, saveEntries],
  )

  /** Remove an entry. */
  const handleRemove = useCallback(
    (key: string) => {
      const updated = entries.filter((e) => e.key !== key)
      setEntries(updated)
      saveEntries(updated)
    },
    [entries, saveEntries],
  )

  /** Add a new entry. */
  const handleAdd = useCallback(() => {
    const key = newKeyInput.trim()
    if (!key) return
    if (entries.some((e) => e.key === key)) return // duplicate key
    const updated = [...entries, { key, value: '' }]
    setEntries(updated)
    setNewKeyInput('')
    setShowNewKeyInput(false)
    setEditingKey(key) // start editing the value
    saveEntries(updated)
  }, [newKeyInput, entries, saveEntries])

  // ---- Raw-YAML editor mode ----
  if (rawMode) {
    return (
      <section
        className="properties-view mt-2 mb-4 border border-white/10 rounded-lg overflow-hidden"
        aria-label="Properties — raw YAML editor"
      >
        <div className="flex items-center justify-between px-4 py-2 bg-white/[0.02] border-b border-white/5">
          <span className="text-xs font-semibold text-white/40 uppercase tracking-wide">
            Raw YAML
          </span>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={saveRawYaml}
              className="text-xs px-2 py-0.5 rounded bg-blue-500/20 text-blue-400 hover:bg-blue-500/30 transition-colors"
            >
              Save
            </button>
            <button
              type="button"
              onClick={exitRawMode}
              className="text-xs text-white/30 hover:text-white/50 transition-colors"
            >
              Back to table
            </button>
          </div>
        </div>
        <textarea
          value={rawYamlText}
          onChange={(e) => setRawYamlText(e.target.value)}
          className="w-full min-h-[120px] bg-transparent text-sm font-mono text-white/80 p-3 outline-none resize-y focus:bg-white/[0.01] transition-colors"
          placeholder="key: value"
          spellCheck={false}
          aria-label="Raw YAML editor"
        />
      </section>
    )
  }

  // ---- Empty state ----
  if (entries.length === 0 && !showNewKeyInput) {
    return (
      <section
        className="properties-view mt-2 mb-4 border border-dashed border-white/10 rounded-lg px-4 py-3"
        aria-label="Properties"
      >
        <div className="flex items-center justify-between mb-2">
          <span className="text-xs font-semibold text-white/40 uppercase tracking-wide">
            Properties
          </span>
          <button
            type="button"
            onClick={enterRawMode}
            className="text-xs text-white/30 hover:text-white/50 transition-colors"
            title="Edit raw YAML"
          >
            {'{ }'}
          </button>
        </div>
        <p className="text-xs text-white/30 mb-2">No properties yet.</p>
        <button
          type="button"
          onClick={() => setShowNewKeyInput(true)}
          className="text-xs text-blue-400/70 hover:text-blue-400 transition-colors"
        >
          + Add properties
        </button>
      </section>
    )
  }

  // ---- Table view ----
  return (
    <section
      className="properties-view mt-2 mb-4 border border-white/10 rounded-lg overflow-hidden"
      aria-label="Properties"
    >
      {/* Header bar */}
      <div className="flex items-center justify-between px-4 py-2 bg-white/[0.02] border-b border-white/5">
        <span className="text-xs font-semibold text-white/40 uppercase tracking-wide">
          Properties
        </span>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={enterRawMode}
            className="text-xs text-white/30 hover:text-white/50 transition-colors"
            title="Edit raw YAML"
          >
            {'{ }'}
          </button>
          <button
            type="button"
            onClick={() => setCollapsed(!collapsed)}
            className="text-xs text-white/30 hover:text-white/50 transition-colors"
            aria-label={collapsed ? 'Expand properties' : 'Collapse properties'}
          >
            {collapsed ? '▲' : '▼'}
          </button>
        </div>
      </div>

      {!collapsed && (
        <>
          {/* Table header */}
          <div className="grid grid-cols-[1fr_2fr_auto] gap-1 px-4 py-1.5 text-[10px] font-semibold text-white/30 uppercase tracking-wide border-b border-white/5">
            <span>Property</span>
            <span>Value</span>
            <span />
          </div>

          {/* Rows */}
          {entries.map((entry) => (
            <div
              key={entry.key}
              className="group grid grid-cols-[1fr_2fr_auto] gap-1 px-4 py-1.5 border-b border-white/5 last:border-b-0 hover:bg-white/[0.02] transition-colors"
            >
              {/* Key cell */}
              <div className="flex items-center">
                {editingKey === entry.key ? (
                  <input
                    type="text"
                    defaultValue={entry.key}
                    autoFocus
                    onBlur={(e) => {
                      const newKey = e.target.value.trim()
                      if (newKey && newKey !== entry.key) {
                        handleKeyChange(entry.key, newKey)
                      } else {
                        setEditingKey(null)
                      }
                    }}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') {
                        const target = e.target as HTMLInputElement
                        const newKey = target.value.trim()
                        if (newKey && newKey !== entry.key) {
                          handleKeyChange(entry.key, newKey)
                        } else {
                          setEditingKey(null)
                        }
                      }
                      if (e.key === 'Escape') {
                        setEditingKey(null)
                      }
                    }}
                    className="w-full bg-transparent border-b border-blue-400/40 text-xs font-mono text-white/80 outline-none"
                  />
                ) : (
                  <button
                    type="button"
                    onClick={() => setEditingKey(entry.key)}
                    className="text-xs font-mono text-white/70 hover:text-white/90 transition-colors text-left"
                    title="Rename property"
                  >
                    {entry.key}
                  </button>
                )}
              </div>

              {/* Value cell */}
              <div className="flex items-center min-h-[24px]">
                <ValueInput
                  value={entry.value}
                  onChange={(newVal) => handleValueChange(entry.key, newVal)}
                  onBlur={() => setEditingKey(null)}
                />
              </div>

              {/* Remove button */}
              <div className="flex items-center justify-end opacity-0 group-hover:opacity-100 transition-opacity">
                <button
                  type="button"
                  onClick={() => handleRemove(entry.key)}
                  className="text-xs text-white/20 hover:text-red-400 transition-colors px-1"
                  title={`Remove "${entry.key}"`}
                  aria-label={`Remove property ${entry.key}`}
                >
                  ✕
                </button>
              </div>
            </div>
          ))}

          {/* Add new property */}
          <div className="px-4 py-2 border-t border-white/5">
            {showNewKeyInput ? (
              <div className="flex items-center gap-2">
                <input
                  type="text"
                  value={newKeyInput}
                  onChange={(e) => setNewKeyInput(e.target.value)}
                  onBlur={() => {
                    if (!newKeyInput.trim()) setShowNewKeyInput(false)
                  }}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') handleAdd()
                    if (e.key === 'Escape') {
                      setShowNewKeyInput(false)
                      setNewKeyInput('')
                    }
                  }}
                  placeholder="Property name…"
                  autoFocus
                  className="flex-1 bg-transparent border-b border-blue-400/40 text-sm text-white/80 outline-none placeholder:text-white/20"
                />
                <button
                  type="button"
                  onClick={handleAdd}
                  disabled={!newKeyInput.trim()}
                  className="text-xs text-blue-400/70 hover:text-blue-400 transition-colors disabled:text-white/20 disabled:cursor-not-allowed"
                >
                  Add
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setShowNewKeyInput(false)
                    setNewKeyInput('')
                  }}
                  className="text-xs text-white/30 hover:text-white/50 transition-colors"
                >
                  Cancel
                </button>
              </div>
            ) : (
              <button
                type="button"
                onClick={() => setShowNewKeyInput(true)}
                className="text-xs text-blue-400/70 hover:text-blue-400 transition-colors"
              >
                + Add property
              </button>
            )}
          </div>
        </>
      )}
    </section>
  )
}
