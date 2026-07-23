import React, {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useRef,
  useState
} from 'react'
import { useAppContext } from '../../shared/store'
import { FileEntry, Template } from '@shared/types'
import { PlusIcon, FolderPlusIcon } from '../../shared/components/icons'
import {
  renameFile as cmdRenameFile,
  deleteFile as cmdDeleteFile,
  createFolder as cmdCreateFolder,
  createNote as cmdCreateNote,
  openTreeFile as cmdOpenTreeFile
} from './vaultCommands'

// ---------------------------------------------------------------------------
// TreeNode
// ---------------------------------------------------------------------------

interface TreeNode {
  name: string
  path: string
  type: 'file' | 'folder'
  children?: TreeNode[]
}

// ---------------------------------------------------------------------------
// Build tree from flat FileEntry array
// ---------------------------------------------------------------------------

/**
 * Returns true if a path segment or any ancestor segment should be excluded.
 * Excludes _-prefixed paths (e.g. _templates/), .nabu/, dot-prefixed paths,
 * and non-.md files.
 */
function shouldExclude(filePath: string, vaultRoot: string): boolean {
  // Get path relative to vault root
  const relative = filePath.startsWith(vaultRoot)
    ? filePath.slice(vaultRoot.length).replace(/^[\\/]/, '')
    : filePath

  const segments = relative.split(/[\\/]/)

  // Check each segment for underscore-prefix or dot-prefix
  for (const seg of segments) {
    if (seg.startsWith('_')) return true
    if (seg.startsWith('.')) return true
  }

  // Must end with .md
  const last = segments[segments.length - 1]
  if (!last.endsWith('.md')) return true

  return false
}

function buildTree(files: FileEntry[], vaultRoot: string): TreeNode[] {
  const root: TreeNode = { name: '', path: '', type: 'folder', children: [] }

  for (const file of files) {
    if (shouldExclude(file.path, vaultRoot)) continue

    const relative = file.path.startsWith(vaultRoot)
      ? file.path.slice(vaultRoot.length).replace(/^[\\/]/, '')
      : file.path

    const segments = relative.split(/[\\/]/)
    let current = root

    for (let i = 0; i < segments.length; i++) {
      const seg = segments[i]
      const isLast = i === segments.length - 1

      if (isLast) {
        // file node
        current.children = current.children ?? []
        current.children.push({
          name: seg,
          path: file.path,
          type: 'file'
        })
      } else {
        // folder node — find or create
        current.children = current.children ?? []
        let folder = current.children.find((c) => c.type === 'folder' && c.name === seg)
        if (!folder) {
          // reconstruct folder path
          const folderPath = vaultRoot + '/' + segments.slice(0, i + 1).join('/')
          folder = { name: seg, path: folderPath, type: 'folder', children: [] }
          current.children.push(folder)
        }
        current = folder
      }
    }
  }

  sortTree(root)
  return root.children ?? []
}

/** Sort: folders before files, alphabetical (case-insensitive) within groups */
function sortTree(node: TreeNode): void {
  if (!node.children) return
  node.children.sort((a, b) => {
    if (a.type !== b.type) return a.type === 'folder' ? -1 : 1
    return a.name.toLowerCase().localeCompare(b.name.toLowerCase())
  })
  for (const child of node.children) sortTree(child)
}

// ---------------------------------------------------------------------------
// Flatten tree (for filtered view)
// ---------------------------------------------------------------------------

function flattenTree(nodes: TreeNode[]): TreeNode[] {
  const result: TreeNode[] = []
  for (const node of nodes) {
    result.push(node)
    if (node.children) result.push(...flattenTree(node.children))
  }
  return result
}

// ---------------------------------------------------------------------------
// FileTree public handle (for forwardRef)
// ---------------------------------------------------------------------------

export interface FileTreeHandle {
  focusSearch(): void
}

// ---------------------------------------------------------------------------
// FileTree props
// ---------------------------------------------------------------------------

export interface FileTreeProps {
  tagFilteredPaths?: Set<string> | null
}

// ---------------------------------------------------------------------------
// TreeNodeRow — single row in the tree
// ---------------------------------------------------------------------------

interface TreeNodeRowProps {
  node: TreeNode
  depth: number
  isExpanded: boolean
  isActive: boolean
  isPulsing: boolean
  onToggle: (path: string) => void
  onFileClick: (node: TreeNode) => void
  onContextMenu?: (e: React.MouseEvent, node: TreeNode) => void
}

function TreeNodeRow({
  node,
  depth,
  isExpanded,
  isActive,
  isPulsing,
  onToggle,
  onFileClick,
  onContextMenu
}: TreeNodeRowProps): React.JSX.Element {
  const indent = depth * 12 // px per level

  if (node.type === 'folder') {
    return (
      <div
        role="button"
        tabIndex={0}
        aria-expanded={isExpanded}
        className="flex items-center gap-1 px-2 py-0.5 cursor-pointer select-none text-sm
                   text-nabu-text-muted hover:text-nabu-text hover:bg-nabu-bg-mute
                   rounded transition-colors"
        style={{ paddingLeft: `${8 + indent}px` }}
        onClick={() => onToggle(node.path)}
        onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && onToggle(node.path)}
      >
        <span
          aria-hidden="true"
          className="shrink-0 w-3 text-nabu-text-faint transition-transform duration-150"
          style={{ transform: isExpanded ? 'rotate(90deg)' : 'rotate(0deg)' }}
        >
          ▶
        </span>
        <span className="truncate">{node.name}</span>
      </div>
    )
  }

  // file node
  return (
    <div
      role="button"
      tabIndex={0}
      aria-current={isActive ? 'page' : undefined}
      className={[
        'flex items-center gap-1 px-2 py-0.5 cursor-pointer select-none text-sm rounded transition-colors',
        isActive
          ? 'bg-nabu-accent/20 text-nabu-accent font-medium'
          : 'text-nabu-text hover:bg-nabu-bg-mute hover:text-nabu-text',
        isPulsing ? 'external-edit' : ''
      ]
        .filter(Boolean)
        .join(' ')}
      style={{ paddingLeft: `${8 + indent}px` }}
      onClick={() => onFileClick(node)}
      onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && onFileClick(node)}
      onContextMenu={onContextMenu ? (e) => onContextMenu(e, node) : undefined}
    >
      <span aria-hidden="true" className="shrink-0 w-3 text-nabu-text-faint text-xs">
        ·
      </span>
      <span className="truncate">{node.name}</span>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Recursive tree renderer
// ---------------------------------------------------------------------------

interface TreeRendererProps {
  nodes: TreeNode[]
  depth: number
  expandedFolders: Set<string>
  activeFile: string | null
  pulsingPaths: Set<string>
  onToggle: (path: string) => void
  onFileClick: (node: TreeNode) => void
  onContextMenu: (e: React.MouseEvent, node: TreeNode) => void
}

function TreeRenderer({
  nodes,
  depth,
  expandedFolders,
  activeFile,
  pulsingPaths,
  onToggle,
  onFileClick,
  onContextMenu
}: TreeRendererProps): React.JSX.Element {
  return (
    <>
      {nodes.map((node) => {
        const isExpanded = node.type === 'folder' && expandedFolders.has(node.path)
        return (
          <React.Fragment key={node.path}>
            <TreeNodeRow
              node={node}
              depth={depth}
              isExpanded={isExpanded}
              isActive={node.type === 'file' && activeFile === node.path}
              isPulsing={pulsingPaths.has(node.path)}
              onToggle={onToggle}
              onFileClick={onFileClick}
              onContextMenu={node.type === 'file' ? onContextMenu : undefined}
            />
            {node.type === 'folder' && isExpanded && node.children && (
              <TreeRenderer
                nodes={node.children}
                depth={depth + 1}
                expandedFolders={expandedFolders}
                activeFile={activeFile}
                pulsingPaths={pulsingPaths}
                onToggle={onToggle}
                onFileClick={onFileClick}
                onContextMenu={onContextMenu}
              />
            )}
          </React.Fragment>
        )
      })}
    </>
  )
}

// ---------------------------------------------------------------------------
// FileTree component
// ---------------------------------------------------------------------------

export const FileTree = forwardRef<FileTreeHandle, FileTreeProps>(function FileTree(
  { tagFilteredPaths },
  ref
) {
  const { state, dispatch } = useAppContext()
  const searchRef = useRef<HTMLInputElement>(null)
  const folderInputRef = useRef<HTMLInputElement>(null)
  const [query, setQuery] = useState('')
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(new Set())
  const [pulsingPaths, setPulsingPaths] = useState<Set<string>>(new Set())

  // Folder creation dialog state
  const [showFolderDialog, setShowFolderDialog] = useState(false)
  const [folderName, setFolderName] = useState('')
  const [folderError, setFolderError] = useState<string | null>(null)
  const [folderLoading, setFolderLoading] = useState(false)

  // Note creation dialog state
  const [showNoteDialog, setShowNoteDialog] = useState(false)
  const [templates, setTemplates] = useState<Template[]>([])
  const [selectedTemplate, setSelectedTemplate] = useState<Template | null>(null)
  const [noteName, setNoteName] = useState('')
  const [noteError, setNoteError] = useState<string | null>(null)
  const [noteLoading, setNoteLoading] = useState(false)
  const noteNameInputRef = useRef<HTMLInputElement>(null)

  // Context menu state
  const [contextMenuTarget, setContextMenuTarget] = useState<{ path: string; name: string } | null>(
    null
  )
  const [contextMenuPos, setContextMenuPos] = useState<{ x: number; y: number }>({ x: 0, y: 0 })
  const [showRenameInput, setShowRenameInput] = useState(false)
  const [renameValue, setRenameValue] = useState('')
  const [renameError, setRenameError] = useState<string | null>(null)
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false)
  const [menuLoading, setMenuLoading] = useState(false)
  const contextMenuRef = useRef<HTMLDivElement>(null)
  const renameInputRef = useRef<HTMLInputElement>(null)

  // Expose focusSearch handle
  useImperativeHandle(ref, () => ({
    focusSearch() {
      const el = searchRef.current
      if (!el) return
      // Use click() to transfer both DOM and OS-level keyboard focus,
      // then select all so the first keystroke replaces existing text.
      console.log('[FileTree] focusSearch called, el:', !!el)
      el.click()
      el.select()
    }
  }))

  // Focus folder name input when dialog opens
  useEffect(() => {
    if (showFolderDialog) {
      setTimeout(() => folderInputRef.current?.focus(), 0)
    }
  }, [showFolderDialog])

  // Dismiss folder dialog on Escape
  useEffect(() => {
    if (!showFolderDialog) return
    const handleKeyDown = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') {
        setShowFolderDialog(false)
        setFolderName('')
        setFolderError(null)
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [showFolderDialog])

  // Focus note name input when note dialog opens
  useEffect(() => {
    if (showNoteDialog) {
      setTimeout(() => noteNameInputRef.current?.focus(), 0)
    }
  }, [showNoteDialog])

  // Dismiss note dialog on Escape
  useEffect(() => {
    if (!showNoteDialog) return
    const handleKeyDown = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') {
        setShowNoteDialog(false)
        setNoteName('')
        setNoteError(null)
        setSelectedTemplate(null)
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [showNoteDialog])

  // Context menu: open
  const handleContextMenu = useCallback((e: React.MouseEvent, node: TreeNode) => {
    e.preventDefault()
    setContextMenuTarget({ path: node.path, name: node.name })
    setContextMenuPos({ x: e.clientX, y: e.clientY })
    setShowRenameInput(false)
    setRenameValue(node.name)
    setRenameError(null)
    setShowDeleteConfirm(false)
    setMenuLoading(false)
  }, [])

  // Context menu: close on outside mousedown
  useEffect(() => {
    if (!contextMenuTarget) return
    const handleMouseDown = (e: MouseEvent): void => {
      if (contextMenuRef.current && !contextMenuRef.current.contains(e.target as Node)) {
        setContextMenuTarget(null)
        setShowRenameInput(false)
        setRenameError(null)
        setShowDeleteConfirm(false)
      }
    }
    document.addEventListener('mousedown', handleMouseDown)
    return () => document.removeEventListener('mousedown', handleMouseDown)
  }, [contextMenuTarget])

  // Context menu: close on Escape key
  useEffect(() => {
    if (!contextMenuTarget) return
    const handleKeyDown = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') {
        setContextMenuTarget(null)
        setShowRenameInput(false)
        setRenameError(null)
        setShowDeleteConfirm(false)
      }
    }
    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [contextMenuTarget])

  // Focus rename input when shown
  useEffect(() => {
    if (showRenameInput) {
      setTimeout(() => renameInputRef.current?.focus(), 0)
    }
  }, [showRenameInput])

  // Rename handler (delegates orchestration to vaultCommands)
  const handleRename = useCallback(async () => {
    if (!contextMenuTarget) return
    setMenuLoading(true)
    setRenameError(null)
    const result = await cmdRenameFile(contextMenuTarget.path, renameValue, state.currentFile, dispatch)
    if (!result.success) {
      setRenameError(result.error ?? 'Failed to rename.')
      setMenuLoading(false)
      return
    }
    setContextMenuTarget(null)
    setShowRenameInput(false)
    setRenameError(null)
    setMenuLoading(false)
  }, [contextMenuTarget, renameValue, state.currentFile, dispatch])

  // Delete handler (delegates orchestration to vaultCommands)
  const handleDelete = useCallback(async () => {
    if (!contextMenuTarget) return
    setMenuLoading(true)
    setRenameError(null)
    const result = await cmdDeleteFile(contextMenuTarget.path)
    if (!result.success) {
      setRenameError(result.error ?? 'Failed to delete.')
      setMenuLoading(false)
      return
    }
    setContextMenuTarget(null)
    setShowDeleteConfirm(false)
    setMenuLoading(false)
  }, [contextMenuTarget])

  // Create folder handler (delegates orchestration to vaultCommands)
  const handleCreateFolder = useCallback(async () => {
    if (!state.vault) return
    setFolderLoading(true)
    setFolderError(null)
    const result = await cmdCreateFolder(state.vault.path, folderName, dispatch)
    if (!result.success) {
      setFolderError(result.error ?? 'Failed to create folder.')
      setFolderLoading(false)
      return
    }
    // Close dialog and reset state
    setShowFolderDialog(false)
    setFolderName('')
    setFolderError(null)
    setFolderLoading(false)
  }, [state.vault, folderName, dispatch])

  // Create note handler (delegates orchestration to vaultCommands)
  const handleCreateNote = useCallback(async () => {
    if (!state.vault) return
    setNoteLoading(true)
    setNoteError(null)
    const result = await cmdCreateNote(state.vault.path, noteName, selectedTemplate, dispatch)
    if (!result.success) {
      setNoteError(result.error ?? 'Failed to create note.')
      setNoteLoading(false)
      return
    }
    // Close dialog and reset state
    setShowNoteDialog(false)
    setNoteName('')
    setNoteError(null)
    setSelectedTemplate(null)
    setNoteLoading(false)
  }, [state.vault, noteName, selectedTemplate, dispatch])
  const tree = React.useMemo(() => {
    if (!state.vault) return []
    // When tag filtering is active, only pass files whose paths are in tagFilteredPaths
    const files =
      tagFilteredPaths != null
        ? state.vault.files.filter((f) => tagFilteredPaths.has(f.path))
        : state.vault.files
    return buildTree(files, state.vault.path)
  }, [state.vault, tagFilteredPaths])

  // Listen for external edits to trigger pulse animation
  useEffect(() => {
    const off = window.ipc.on.noteUpdated(({ path, isExternal }) => {
      if (!isExternal) return
      setPulsingPaths((prev) => new Set(prev).add(path))
      setTimeout(() => {
        setPulsingPaths((prev) => {
          const next = new Set(prev)
          next.delete(path)
          return next
        })
      }, 600)
    })
    return off
  }, [])

  const handleToggle = useCallback((folderPath: string) => {
    setExpandedFolders((prev) => {
      const next = new Set(prev)
      if (next.has(folderPath)) next.delete(folderPath)
      else next.add(folderPath)
      return next
    })
  }, [])

  const handleFileClick = useCallback(
    async (node: TreeNode) => {
      try {
        await cmdOpenTreeFile(node.path, dispatch)
      } catch (err) {
        console.error('[FileTree] Failed to load file:', node.path, err)
      }
    },
    [dispatch]
  )

  // Filtered view: full-text search when index is available, else name-based filter
  const filteredNodes = React.useMemo(() => {
    if (!query.trim()) return null

    // Full-text search when index is available
    if (state.fullTextIndex.size > 0) {
      const words = query.toLowerCase().split(/\s+/).filter(Boolean)
      const matchCounts = new Map<string, number>()
      for (const word of words) {
        const paths = state.fullTextIndex.get(word)
        if (paths) {
          for (const p of paths) {
            matchCounts.set(p, (matchCounts.get(p) ?? 0) + 1)
          }
        }
      }
      // Sort by match count descending, then return matching TreeNodes
      const allFiles = flattenTree(tree).filter((n) => n.type === 'file')
      return Array.from(matchCounts.entries())
        .sort((a, b) => b[1] - a[1])
        .map(([p]) => allFiles.find((n) => n.path === p))
        .filter((n): n is TreeNode => n !== undefined)
    }

    // Fallback: name-based filter
    const q = query.toLowerCase()
    return flattenTree(tree).filter((n) => n.type === 'file' && n.name.toLowerCase().includes(q))
  }, [tree, query, state.fullTextIndex])

  return (
    <div className="file-tree flex flex-col h-full" aria-label="File tree">
      {/* Create buttons */}
      <div className="px-2 pt-2 flex gap-1 shrink-0">
        <button
          aria-label="Create folder"
          disabled={!state.vault}
          onClick={() => {
            setFolderName('')
            setFolderError(null)
            setShowFolderDialog(true)
          }}
          className="flex flex-1 items-center justify-center rounded
                     bg-nabu-bg-mute border border-nabu-border text-nabu-text-muted
                     hover:text-nabu-text hover:border-nabu-accent transition-colors
                     disabled:opacity-40 disabled:cursor-not-allowed"
          style={{ height: 28 }}
          type="button"
          title="Create folder"
        >
          <FolderPlusIcon size={14} />
        </button>
        <button
          aria-label="Create note"
          disabled={!state.vault}
          onClick={async () => {
            if (!state.vault) return
            setNoteLoading(true)
            try {
              const { templates: tpls } = await window.ipc.templates.list(state.vault.path)
              setTemplates(tpls)
            } catch {
              setTemplates([])
            }
            setNoteName('')
            setNoteError(null)
            setSelectedTemplate(null)
            setShowNoteDialog(true)
            setNoteLoading(false)
          }}
          className="flex flex-1 items-center justify-center rounded
                     bg-nabu-bg-mute border border-nabu-border text-nabu-text-muted
                     hover:text-nabu-text hover:border-nabu-accent transition-colors
                     disabled:opacity-40 disabled:cursor-not-allowed"
          style={{ height: 28 }}
          type="button"
          title="Create note"
        >
          <PlusIcon size={14} />
        </button>
      </div>

      {/* Inline folder creation dialog */}
      {showFolderDialog && (
        <div
          role="dialog"
          aria-modal="true"
          aria-label="Create folder"
          className="mx-2 mt-1 p-2 rounded border border-nabu-border bg-nabu-bg-mute shrink-0"
        >
          <input
            ref={folderInputRef}
            type="text"
            placeholder="Folder name…"
            value={folderName}
            onChange={(e) => {
              setFolderName(e.target.value)
              setFolderError(null)
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleCreateFolder()
            }}
            disabled={folderLoading}
            className="w-full px-2 py-1 text-xs rounded bg-nabu-bg border border-nabu-border
                       text-nabu-text placeholder:text-nabu-text-faint
                       focus:outline-none focus:border-nabu-accent transition-colors
                       disabled:opacity-50"
            aria-label="New folder name"
          />
          {folderError && (
            <p className="mt-1 text-xs text-red-400" role="alert">
              {folderError}
            </p>
          )}
          {folderLoading && <p className="mt-1 text-xs text-nabu-text-faint">Creating…</p>}
          {!folderLoading && !folderError && folderName.trim() === '' && (
            <p className="mt-1 text-xs text-nabu-text-faint">Enter a folder name.</p>
          )}
          <div className="flex gap-1 mt-2">
            <button
              onClick={handleCreateFolder}
              disabled={folderLoading || folderName.trim() === ''}
              className="flex-1 px-2 py-1 text-xs rounded bg-nabu-accent text-white
                         hover:opacity-90 transition-opacity
                         disabled:opacity-40 disabled:cursor-not-allowed"
            >
              Confirm
            </button>
            <button
              onClick={() => {
                setShowFolderDialog(false)
                setFolderName('')
                setFolderError(null)
              }}
              disabled={folderLoading}
              className="flex-1 px-2 py-1 text-xs rounded border border-nabu-border
                         text-nabu-text-muted hover:text-nabu-text transition-colors
                         disabled:opacity-40 disabled:cursor-not-allowed"
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {/* Note creation dialog */}
      {showNoteDialog && (
        <div
          role="dialog"
          aria-modal="true"
          aria-label="Create note"
          className="mx-2 mt-1 p-2 rounded border border-nabu-border bg-nabu-bg-mute shrink-0"
        >
          {/* Template list */}
          <div className="mb-2 max-h-36 overflow-y-auto">
            <p className="text-xs text-nabu-text-faint mb-1">Template</p>
            {/* Empty note option */}
            <div
              role="button"
              tabIndex={0}
              onClick={() => setSelectedTemplate(null)}
              onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && setSelectedTemplate(null)}
              className={[
                'flex items-center gap-2 px-2 py-0.5 rounded cursor-pointer text-xs transition-colors',
                selectedTemplate === null
                  ? 'bg-nabu-accent/20 text-nabu-accent'
                  : 'text-nabu-text hover:bg-nabu-bg'
              ].join(' ')}
            >
              <span
                aria-hidden="true"
                className={[
                  'w-3 h-3 rounded-full border shrink-0',
                  selectedTemplate === null
                    ? 'border-nabu-accent bg-nabu-accent'
                    : 'border-nabu-border'
                ].join(' ')}
              />
              (Empty note)
            </div>
            {/* Template options */}
            {templates.map((tpl) => (
              <div
                key={tpl.path}
                role="button"
                tabIndex={0}
                onClick={() => setSelectedTemplate(tpl)}
                onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && setSelectedTemplate(tpl)}
                className={[
                  'flex items-center gap-2 px-2 py-0.5 rounded cursor-pointer text-xs transition-colors',
                  selectedTemplate?.path === tpl.path
                    ? 'bg-nabu-accent/20 text-nabu-accent'
                    : 'text-nabu-text hover:bg-nabu-bg'
                ].join(' ')}
              >
                <span
                  aria-hidden="true"
                  className={[
                    'w-3 h-3 rounded-full border shrink-0',
                    selectedTemplate?.path === tpl.path
                      ? 'border-nabu-accent bg-nabu-accent'
                      : 'border-nabu-border'
                  ].join(' ')}
                />
                {tpl.name}
              </div>
            ))}
          </div>

          {/* Note name input */}
          <input
            ref={noteNameInputRef}
            type="text"
            placeholder="Note name…"
            value={noteName}
            onChange={(e) => {
              setNoteName(e.target.value)
              setNoteError(null)
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleCreateNote()
            }}
            disabled={noteLoading}
            className="w-full px-2 py-1 text-xs rounded bg-nabu-bg border border-nabu-border
                       text-nabu-text placeholder:text-nabu-text-faint
                       focus:outline-none focus:border-nabu-accent transition-colors
                       disabled:opacity-50"
            aria-label="Note name"
          />
          {noteError && (
            <p className="mt-1 text-xs text-red-400" role="alert">
              {noteError}
            </p>
          )}
          {noteLoading && <p className="mt-1 text-xs text-nabu-text-faint">Creating…</p>}

          {/* Action buttons */}
          <div className="flex gap-1 mt-2">
            <button
              onClick={handleCreateNote}
              disabled={noteLoading || noteName.trim() === ''}
              className="flex-1 px-2 py-1 text-xs rounded bg-nabu-accent text-white
                         hover:opacity-90 transition-opacity
                         disabled:opacity-40 disabled:cursor-not-allowed"
            >
              Confirm
            </button>
            <button
              onClick={() => {
                setShowNoteDialog(false)
                setNoteName('')
                setNoteError(null)
                setSelectedTemplate(null)
              }}
              disabled={noteLoading}
              className="flex-1 px-2 py-1 text-xs rounded border border-nabu-border
                         text-nabu-text-muted hover:text-nabu-text transition-colors
                         disabled:opacity-40 disabled:cursor-not-allowed"
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {/* Search / Filter input */}
      <div className="px-2 pt-2 pb-1 shrink-0">
        <input
          ref={searchRef}
          type="text"
          placeholder="Filter files…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          className="w-full px-2 py-1 text-xs rounded bg-nabu-bg-mute border border-nabu-border
                     text-nabu-text placeholder:text-nabu-text-faint
                     focus:outline-none focus:border-nabu-accent transition-colors"
          aria-label="Search notes"
        />
      </div>

      {/* Tree or filtered flat list */}
      <div
        className="flex-1 overflow-y-auto py-1"
        role={filteredNodes !== null ? 'listbox' : 'tree'}
        aria-label="Vault files"
      >
        {state.vault === null ? (
          <p className="px-3 py-2 text-xs text-nabu-text-faint">No vault open</p>
        ) : filteredNodes !== null ? (
          // Flat filtered list
          filteredNodes.length === 0 ? (
            <p className="px-3 py-2 text-xs text-nabu-text-faint">No files match</p>
          ) : (
            filteredNodes.map((node) => (
              <div role="option" key={node.path}>
                <TreeNodeRow
                  node={node}
                  depth={0}
                  isExpanded={false}
                  isActive={state.currentFile === node.path}
                  isPulsing={pulsingPaths.has(node.path)}
                  onToggle={handleToggle}
                  onFileClick={handleFileClick}
                  onContextMenu={handleContextMenu}
                />
              </div>
            ))
          )
        ) : (
          // Full tree view
          <TreeRenderer
            nodes={tree}
            depth={0}
            expandedFolders={expandedFolders}
            activeFile={state.currentFile}
            pulsingPaths={pulsingPaths}
            onToggle={handleToggle}
            onFileClick={handleFileClick}
            onContextMenu={handleContextMenu}
          />
        )}
      </div>

      {/* Context menu */}
      {contextMenuTarget && (
        <div
          ref={contextMenuRef}
          id="file-tree-context-menu"
          role="menu"
          style={{ position: 'fixed', left: contextMenuPos.x, top: contextMenuPos.y, zIndex: 50 }}
          className="bg-nabu-bg-mute border border-nabu-border rounded shadow-lg py-1 min-w-[140px]"
        >
          {!showRenameInput && !showDeleteConfirm && (
            <>
              <button
                role="menuitem"
                aria-label="Rename file"
                className="w-full text-left px-3 py-1 text-sm text-nabu-text hover:bg-nabu-bg transition-colors"
                onClick={() => setShowRenameInput(true)}
              >
                Rename
              </button>
              <button
                role="menuitem"
                aria-label="Delete file"
                className="w-full text-left px-3 py-1 text-sm text-red-400 hover:bg-nabu-bg transition-colors"
                onClick={() => setShowDeleteConfirm(true)}
              >
                Delete
              </button>
            </>
          )}
          {showRenameInput && (
            <div className="px-2 py-1">
              <input
                ref={renameInputRef}
                type="text"
                value={renameValue}
                onChange={(e) => {
                  setRenameValue(e.target.value)
                  setRenameError(null)
                }}
                onKeyDown={(e) => e.key === 'Enter' && handleRename()}
                disabled={menuLoading}
                className="w-full px-2 py-0.5 text-xs rounded bg-nabu-bg border border-nabu-border text-nabu-text focus:outline-none focus:border-nabu-accent"
                aria-label="New file name"
                autoFocus
              />
              {renameError && <p className="text-xs text-red-400 mt-0.5">{renameError}</p>}
              <div className="flex gap-1 mt-1">
                <button
                  onClick={handleRename}
                  disabled={menuLoading}
                  className="flex-1 px-2 py-0.5 text-xs rounded bg-nabu-accent text-white disabled:opacity-40"
                >
                  Rename
                </button>
                <button
                  onClick={() => setShowRenameInput(false)}
                  disabled={menuLoading}
                  className="flex-1 px-2 py-0.5 text-xs rounded border border-nabu-border text-nabu-text-muted"
                >
                  Cancel
                </button>
              </div>
            </div>
          )}
          {showDeleteConfirm && (
            <div role="dialog" aria-modal="true" className="px-2 py-1">
              <p className="text-xs text-nabu-text mb-1">
                Delete &ldquo;{contextMenuTarget.name}&rdquo;?
              </p>
              {renameError && <p className="text-xs text-red-400 mb-1">{renameError}</p>}
              <div className="flex gap-1">
                <button
                  onClick={handleDelete}
                  disabled={menuLoading}
                  className="flex-1 px-2 py-0.5 text-xs rounded bg-red-600 text-white disabled:opacity-40"
                >
                  Delete
                </button>
                <button
                  onClick={() => setShowDeleteConfirm(false)}
                  disabled={menuLoading}
                  className="flex-1 px-2 py-0.5 text-xs rounded border border-nabu-border text-nabu-text-muted"
                >
                  Cancel
                </button>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  )
})
