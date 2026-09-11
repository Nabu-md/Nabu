import { Fragment, memo, useCallback, useEffect, useMemo, useRef, useState, type DragEvent as ReactDragEvent, type KeyboardEvent as ReactKeyboardEvent, type MouseEvent as ReactMouseEvent } from 'react'
import { CaretDown, CaretRight, File, Plus } from '@phosphor-icons/react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { FolderCreationParent, FolderNode, SidebarSelection, VaultEntry } from '../types'
import { FolderContextMenu } from './folder-tree/FolderContextMenu'
import { FolderNameInput } from './folder-tree/FolderNameInput'
import { folderNodeKey } from './folder-tree/folderTreeUtils'
import { useFolderContextMenu } from './folder-tree/useFolderContextMenu'
import { useFolderTreeDisclosure } from './folder-tree/useFolderTreeDisclosure'
import { SidebarGroupHeader } from './sidebar/SidebarGroupHeader'
import { translate, type AppLocale } from '../lib/i18n'
import type { FolderFileActions } from '../hooks/useFileActions'
import { useNoteListContextMenu as useNoteContextMenu } from './NoteContextMenu'
import { readDraggedNotePath } from '../utils/noteDragDrop'
import { useTreeExplorerData, entriesForFolderNode, mergeEntryFolders, type EntriesByFolder } from './folder-tree/treeExplorerModel'
import { FolderNoteRow } from './folder-tree/FolderNoteRow'
import { describeFolderNote, type FolderNoteDescription } from '../utils/folderNotes'
import { trackEvent } from '../lib/telemetry'
import type { SortConfig } from '../utils/noteListHelpers'

interface FolderTreeProps {
  folders: FolderNode[]
  selection: SidebarSelection
  onSelect: (selection: SidebarSelection) => void
  entries: VaultEntry[]
  onSelectNote?: (entry: VaultEntry) => void
  onCreateFolder?: (name: string, parent?: FolderCreationParent) => Promise<boolean> | boolean
  onRenameFolder?: (folderPath: string, nextName: string) => Promise<boolean> | boolean
  onDeleteFolder?: (folderPath: string) => void
  folderFileActions?: FolderFileActions
  renamingFolderPath?: string | null
  onStartRenameFolder?: (folderPath: string) => void
  onCancelRenameFolder?: () => void
  onCanDropNote?: (notePath: string, folderPath: string) => boolean
  onMoveNoteToFolder?: (notePath: string, folderPath: string) => Promise<unknown> | unknown
  collapsed?: boolean
  locale?: AppLocale
  onToggle?: () => void
  vaultRootPath?: string
  search?: string
  listSort?: SortConfig | null
  typeEntryMap?: Record<string, VaultEntry>
  onEnterNeighborhood?: (entry: VaultEntry) => void
  onOpenInNewWindow?: (entry: VaultEntry) => void
  onRenameFilename?: (path: string, newFilenameStem: string) => void
  onArchivePaths?: (paths: string[]) => void
  onDeletePaths?: (paths: string[]) => void
  onExportPdf?: (entry: VaultEntry) => void
  onToggleFavorite?: (path: string) => void
  onToggleOrganized?: (path: string) => void
  onRevealFile?: (path: string) => void
  onCopyFilePath?: (path: string) => void
  canCopyGitUrl?: (entry: VaultEntry) => boolean
  onCopyGitUrl?: (entry: VaultEntry) => void
}

interface FolderTreeBodyProps
  extends Pick<
  FolderTreeProps,
  | 'locale'
  | 'onCancelRenameFolder'
  | 'onDeleteFolder'
  | 'onRenameFolder'
  | 'onSelect'
  | 'onStartRenameFolder'
  | 'onCanDropNote'
  | 'onMoveNoteToFolder'
  | 'renamingFolderPath'
  | 'selection'
  > {
  displayedExpanded: Record<string, boolean>
  displayedFolders: FolderNode[]
  isCreating: boolean
  onCancelCreateFolder: () => void
  onCreateFolderSubmit: (value: string) => Promise<boolean>
  creationParent?: FolderCreationParent
  rootPath?: string
  sectionCollapsed: boolean
  toggleFolder: (path: string) => void
  onOpenMenu: (node: FolderNode, event: ReactMouseEvent<HTMLElement>) => void
  entriesByFolder: EntriesByFolder
  typeEntryMap: Record<string, VaultEntry>
  fileHandlers: FileRowHandlers
  keyboardNav: TreeKeyboardNav
  folderNoteDescriptions: Map<string, FolderNoteDescription>
  folderNotesExpanded: Record<string, boolean>
  onToggleFolderNote: (path: string) => void
}

interface FileRowHandlers {
  onSelectNote?: (entry: VaultEntry) => void
  onEnterNeighborhood?: (entry: VaultEntry) => void
  onOpenInNewWindow?: (entry: VaultEntry) => void
  onRenameFilename?: (path: string, newFilenameStem: string) => void
  onArchivePaths?: (paths: string[]) => void
  onDeletePaths?: (paths: string[]) => void
  onExportPdf?: (entry: VaultEntry) => void
  onToggleFavorite?: (path: string) => void
  onToggleOrganized?: (path: string) => void
  onRevealFile?: (path: string) => void
  onCopyFilePath?: (path: string) => void
  canCopyGitUrl?: (entry: VaultEntry) => boolean
  onCopyGitUrl?: (entry: VaultEntry) => void
}

interface TreeKeyboardNav {
  registerRow: (key: string, element: HTMLElement | null) => void
  focusSiblingRow: (key: string, offset: number) => void
}

const TREE_INDENT_PX = 12

/** Collect expansion keys for UI-synthesized folder nodes so they render expanded by default. */
function collectSyntheticKeys(nodes: FolderNode[]): string[] {
  const keys: string[] = []
  const walk = (list: FolderNode[]) => {
    for (const node of list) {
      if (node.synthetic) keys.push(folderNodeKey(node))
      walk(node.children)
    }
  }
  walk(nodes)
  return keys
}

function vaultRootLabel(vaultRootPath: string, locale: AppLocale): string {
  const trimmed = vaultRootPath.trim().replace(/[\\/]+$/g, '')
  return trimmed.split(/[\\/]/).filter(Boolean).pop() || translate(locale, 'status.vault.default')
}

function buildRootNode(folders: FolderNode[], vaultRootPath: string | undefined, locale: AppLocale): FolderNode | null {
  if (!vaultRootPath?.trim()) return null
  return {
    name: vaultRootLabel(vaultRootPath, locale),
    path: '',
    rootPath: vaultRootPath,
    children: folders,
  }
}

function useDisplayedFolders(
  folders: FolderNode[],
  expanded: Record<string, boolean>,
  vaultRootPath: string | undefined,
  locale: AppLocale,
) {
  return useMemo(() => {
    const syntheticKeys = collectSyntheticKeys(folders)
    const syntheticExpanded = Object.fromEntries(syntheticKeys.map((key) => [key, true]))
    const hasExplicitRoot = folders.some((folder) => folder.path === '' && folder.rootPath)
    if (hasExplicitRoot) {
      const expandedRoots = Object.fromEntries(
        folders
          .filter((folder) => folder.path === '' && folder.rootPath)
          .map((folder) => [folderNodeKey(folder), true]),
      )
      return {
        displayedExpanded: { ...syntheticExpanded, ...expandedRoots, ...expanded },
        displayedFolders: folders,
      }
    }
    const rootNode = buildRootNode(folders, vaultRootPath, locale)
    return {
      displayedExpanded: rootNode
        ? { [folderNodeKey(rootNode)]: true, ...syntheticExpanded, ...expanded }
        : { ...syntheticExpanded, ...expanded },
      displayedFolders: rootNode ? [rootNode] : folders,
    }
  }, [expanded, folders, locale, vaultRootPath])
}

function folderCreationParent(path: string, rootPath?: string): FolderCreationParent {
  return rootPath ? { path, rootPath } : { path }
}

function creationParentForSelection(selection: SidebarSelection): FolderCreationParent | undefined {
  if (selection.kind !== 'folder') return undefined
  return folderCreationParent(selection.path, selection.rootPath)
}

function useCreateFolderSubmit({
  closeCreateForm,
  creationParent,
  expandFolder,
  onCreateFolder,
  selection,
}: {
  closeCreateForm: () => void
  creationParent?: FolderCreationParent
  expandFolder: (key: string) => void
  onCreateFolder?: (name: string, parent?: FolderCreationParent) => Promise<boolean> | boolean
  selection: SidebarSelection
}) {
  return useCallback(
    async (value: string) => {
    const nextName = value.trim()
    if (!nextName || !onCreateFolder) {
      closeCreateForm()
      return true
    }

    const parent = creationParent ?? creationParentForSelection(selection)
    const created = await onCreateFolder(nextName, parent)
    if (!created) return created

    closeCreateForm()
    if (parent?.path) expandFolder(folderNodeKey(parent))
    return created
    },
    [closeCreateForm, creationParent, expandFolder, onCreateFolder, selection],
  )
}

function useTreeKeyboardNav(): TreeKeyboardNav {
  const rowElementsRef = useRef<Map<string, HTMLElement>>(new Map())

  const registerRow = useCallback((key: string, element: HTMLElement | null) => {
    if (element) rowElementsRef.current.set(key, element)
    else rowElementsRef.current.delete(key)
  }, [])

  const focusSiblingRow = useCallback((key: string, offset: number) => {
    const element = rowElementsRef.current.get(key)
    if (!element) return
    // Visual order is derived from the DOM at keystroke time: rows register
    // and unregister in their own mount effects, so keeping a parallel key
    // array in sync would require render-phase mutation.
    const rows = [...rowElementsRef.current.values()].sort((a, b) => {
      const position = a.compareDocumentPosition(b)
      if (position & Node.DOCUMENT_POSITION_FOLLOWING) return -1
      if (position & Node.DOCUMENT_POSITION_PRECEDING) return 1
      return 0
    })
    const next = rows[rows.indexOf(element) + offset]
    next?.focus()
  }, [])

  return { registerRow, focusSiblingRow }
}

function FileTreeRow({
  entry,
  depth,
  isSelected,
  handlers,
  keyboardNav,
  rowKey,
}: {
  entry: VaultEntry
  depth: number
  isSelected: boolean
  handlers: WithNoteContextMenu
  keyboardNav: TreeKeyboardNav & { syncRowOrder?: (keys: string[]) => void }
  rowKey: string
}) {
  const rowRef = useRef<HTMLDivElement | null>(null)
  useEffect(() => {
    keyboardNav.registerRow(rowKey, rowRef.current)
    return () => keyboardNav.registerRow(rowKey, null)
  }, [keyboardNav, rowKey])

  const handleKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'ArrowDown') {
      event.preventDefault()
      keyboardNav.focusSiblingRow(rowKey, 1)
      return
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault()
      keyboardNav.focusSiblingRow(rowKey, -1)
      return
    }
    if (event.key === 'Enter') {
      event.preventDefault()
      handlers.onSelectNote?.(entry)
    }
    if (event.key === 'F2' && handlers.onRenameFilename) {
      event.preventDefault()
      // Renaming happens via the context dialog; open it through the menu flow.
      const renameTrigger = new MouseEvent('contextmenu', { bubbles: true, clientX: 40, clientY: 40 })
      rowRef.current?.dispatchEvent(renameTrigger)
    }
  }

  return (
    <div
      ref={rowRef}
      className={cn(
        'flex items-center rounded pr-2 text-[13px] hover:bg-accent',
        isSelected && 'bg-[var(--accent-blue-light)] font-medium text-primary',
      )}
      style={{ paddingLeft: 8 + depth * TREE_INDENT_PX, paddingTop: 2, paddingBottom: 2 }}
      data-testid={`tree-file-row:${entry.path}`}
      tabIndex={0}
      onKeyDown={handleKeyDown}
    >
      <button
        type="button"
        className="flex min-w-0 flex-1 items-center gap-2 rounded px-1 py-1 text-left"
        onClick={(event) => {
          if (event.detail > 1) return
          // Meta/Ctrl+click mirrors the old NoteList shortcut for entering
          // Neighborhood mode on a note.
          if ((event.metaKey || event.ctrlKey) && handlers.onEnterNeighborhood) {
            handlers.onEnterNeighborhood(entry)
            return
          }
          handlers.onSelectNote?.(entry)
        }}
        onDoubleClick={() => handlers.onOpenInNewWindow?.(entry)}
        onContextMenu={(event) => {
          event.preventDefault()
          handlers.onNoteContextMenu?.(entry, event)
        }}
      >
        <FileTypeIcon entry={entry} />
        <span className="truncate">{entry.title}</span>
      </button>
    </div>
  )
}

function FileTypeIcon({ entry }: { entry: VaultEntry }) {
  void entry
  return <File size={14} className="shrink-0 text-muted-foreground" />
}

interface WithNoteContextMenu extends FileRowHandlers {
  onNoteContextMenu?: (entry: VaultEntry, event: ReactMouseEvent) => void
}

function FolderFiles({
  node,
  depth,
  rootPath,
  entriesByFolder,
  selection,
  handlers,
  keyboardNav,
  folderNoteDescriptions,
  folderNotesExpanded,
  onToggleFolderNote,
}: {
  node: FolderNode
  depth: number
  rootPath?: string
  entriesByFolder: EntriesByFolder
  selection: SidebarSelection
  typeEntryMap: Record<string, VaultEntry>
  handlers: WithNoteContextMenu
  keyboardNav: TreeKeyboardNav
  folderNoteDescriptions: Map<string, FolderNoteDescription>
  folderNotesExpanded: Record<string, boolean>
  onToggleFolderNote: (path: string) => void
}) {
  const files = entriesForFolderNode(entriesByFolder, node, rootPath)
  if (files.length === 0) return null
  return (
    <>
      {files.map((entry) => {
        const folderNote = folderNoteDescriptions.get(entry.path)
        if (folderNote) {
          const isExpanded = folderNotesExpanded[entry.path] ?? false
          return (
            <Fragment key={entry.path}>
              <FolderNoteRow
                entry={entry}
                depth={depth + 1}
                childrenCount={folderNote.children.length}
                isExpanded={isExpanded}
                isSelected={selection.kind === 'entity' && selection.entry.path === entry.path}
                onToggle={() => onToggleFolderNote(entry.path)}
                onOpen={handlers.onSelectNote ? () => handlers.onSelectNote?.(entry) : undefined}
                onContextMenu={handlers.onNoteContextMenu}
                registerRow={keyboardNav.registerRow}
                rowKey={`folder-note:${entry.path}`}
                focusSiblingRow={keyboardNav.focusSiblingRow}
              />
              {isExpanded &&
                folderNote.children.map((child) => (
                  <FileTreeRow
                    key={`folder-note-child:${entry.path}:${child.path}`}
                    entry={child}
                    depth={depth + 2}
                    isSelected={selection.kind === 'entity' && selection.entry.path === child.path}
                    handlers={handlers}
                    keyboardNav={keyboardNav}
                    rowKey={`folder-note-child:${entry.path}:${child.path}`}
                  />
                ))}
            </Fragment>
          )
        }
        return (
          <FileTreeRow
            key={entry.path}
            entry={entry}
            depth={depth + 1}
            isSelected={selection.kind === 'entity' && selection.entry.path === entry.path}
            handlers={handlers}
            keyboardNav={keyboardNav}
            rowKey={`file:${entry.path}`}
          />
        )
      })}
    </>
  )
}

function selectionMatchesFolder(selection: SidebarSelection, node: FolderNode, defaultRootPath?: string): boolean {
  if (selection.kind !== 'folder' || selection.path !== node.path) return false
  const nodeRootPath = node.rootPath ?? defaultRootPath
  if (!nodeRootPath) return !selection.rootPath
  if (selection.rootPath) return selection.rootPath === nodeRootPath
  return nodeRootPath === defaultRootPath
}

function creationParentMatchesNode(
  creationParent: FolderCreationParent | undefined,
  node: FolderNode,
  defaultRootPath?: string,
): boolean {
  if (!creationParent || creationParent.path !== node.path) return false
  const nodeRootPath = node.rootPath ?? defaultRootPath
  const creationRootPath = creationParent.rootPath ?? defaultRootPath
  return nodeRootPath === creationRootPath
}

function selectionForFolder(path: string, rootPath?: string): SidebarSelection {
  return rootPath ? { kind: 'folder', path, rootPath } : { kind: 'folder', path }
}

export const FolderTree = memo(function FolderTree(options: FolderTreeProps) {
  const {
    folders,
    selection,
    onSelect,
    entries = [],
    onSelectNote,
    onCreateFolder,
    onRenameFolder,
    onDeleteFolder,
    folderFileActions,
    renamingFolderPath,
    onStartRenameFolder,
    onCancelRenameFolder,
    onCanDropNote,
    onMoveNoteToFolder,
    collapsed: externalCollapsed,
    locale = 'en',
    onToggle,
    vaultRootPath,
    search = '',
    listSort = null,
    typeEntryMap = {},
    onEnterNeighborhood,
    onOpenInNewWindow,
    onRenameFilename,
    onArchivePaths,
    onDeletePaths,
    onExportPdf,
    onToggleFavorite,
    onToggleOrganized,
    onRevealFile,
    onCopyFilePath,
    canCopyGitUrl,
    onCopyGitUrl,
  } = options
  const [creationParent, setCreationParent] = useState<FolderCreationParent | undefined>(undefined)
  const {
        closeCreateForm,
        expanded,
        expandFolder,
        handleToggleSection,
        isCreating,
        openCreateForm,
        sectionCollapsed,
        toggleFolder,
      } = useFolderTreeDisclosure({
        collapsed: externalCollapsed,
        onToggle,
        renamingFolderPath,
        selection,
      })
      const openCreateFormForParent = useCallback(
        (folderPath: string, rootPath?: string) => {
        setCreationParent(folderCreationParent(folderPath, rootPath))
        openCreateForm()
        },
        [openCreateForm],
      )
      const {
        closeContextMenu,
        contextMenu,
        handleCopyPathFromMenu,
        handleCreateNoteFromMenu,
        handleCreateFolderFromMenu,
        handleDeleteFromMenu,
        handleOpenMenu,
        handleRevealFromMenu,
        handleRenameFromMenu,
        menuRef,
      } = useFolderContextMenu({
        onDeleteFolder,
        folderFileActions,
        onCreateFolder: onCreateFolder ? openCreateFormForParent : undefined,
        onStartRenameFolder,
      })

      const handleCloseCreateForm = useCallback(() => {
        closeCreateForm()
        setCreationParent(undefined)
      }, [closeCreateForm])

      const handleCreateFolderSubmit = useCreateFolderSubmit({
        closeCreateForm: handleCloseCreateForm,
        creationParent,
        expandFolder,
        onCreateFolder,
        selection,
      })

      const handleCreateFolderClick = useCallback(() => {
        closeContextMenu()
        setCreationParent(undefined)
        openCreateForm()
      }, [closeContextMenu, openCreateForm])

      const { displayedExpanded, displayedFolders } = useDisplayedFolders(
        useMemo(() => mergeEntryFolders(folders, entries, vaultRootPath), [folders, entries, vaultRootPath]),
        expanded,
        vaultRootPath,
        locale,
      )

      const entriesByFolder = useTreeExplorerData(entries, listSort, search, vaultRootPath)

      // Folder notes (`type: Folder`): describe linked children once per entries change.
      const folderNoteDescriptions = useMemo(() => {
        const descriptions = new Map<string, FolderNoteDescription>()
        for (const entry of entries) {
          const description = describeFolderNote({ entry, entries })
          if (description && description.children.length > 0) descriptions.set(entry.path, description)
        }
        return descriptions
      }, [entries])

      const [folderNotesExpanded, setFolderNotesExpanded] = useState<Record<string, boolean>>({})
      const onToggleFolderNote = useCallback((path: string) => {
        setFolderNotesExpanded((current) => {
          const next = !current[path]
          if (next) trackEvent('folder_note_expanded', { child_count: folderNoteDescriptions.get(path)?.children.length })
          return { ...current, [path]: next }
        })
      }, [folderNoteDescriptions])

      // Keep the folder holding the active note expanded so the file stays visible.
      const activeEntityPath = selection.kind === 'entity' ? selection.entry.path : null
      useEffect(() => {
        if (!activeEntityPath) return
        const normalized = activeEntityPath.replaceAll('\\', '/')
        const lastSlash = normalized.lastIndexOf('/')
        if (lastSlash <= 0) return
        expandFolder(normalized.slice(0, lastSlash))
      }, [activeEntityPath, expandFolder])

      const fileHandlers: WithNoteContextMenu = {
        onSelectNote,
        onEnterNeighborhood,
        onOpenInNewWindow,
        onRenameFilename,
        onArchivePaths,
        onDeletePaths,
        onExportPdf,
        onToggleFavorite,
        onToggleOrganized,
        onRevealFile,
        onCopyFilePath,
        canCopyGitUrl,
        onCopyGitUrl,
      }
      const { handleNoteContextMenu, contextMenuNode } = useNoteFileContextMenu(fileHandlers, locale)
      const noteRowHandlers: WithNoteContextMenu = { ...fileHandlers, onNoteContextMenu: handleNoteContextMenu }

      const keyboardNav = useTreeKeyboardNav()

      const explorerEmpty = displayedFolders.length === 0 && !isCreating && entriesByFolder.size === 0
      if (explorerEmpty) return null

      return (
        <div className="border-b border-border" style={{ padding: '0 6px' }}>
          <SidebarGroupHeader
            label={translate(locale, 'sidebar.group.folders')}
            collapsed={sectionCollapsed}
            onToggle={handleToggleSection}
          >
            {onCreateFolder && <CreateFolderButton locale={locale} onCreate={handleCreateFolderClick} />}
          </SidebarGroupHeader>
          <FolderTreeBody
            displayedExpanded={displayedExpanded}
            displayedFolders={displayedFolders}
            isCreating={isCreating}
            locale={locale}
            creationParent={creationParent}
            onCancelCreateFolder={handleCloseCreateForm}
            onCancelRenameFolder={onCancelRenameFolder}
            onCreateFolderSubmit={handleCreateFolderSubmit}
            onDeleteFolder={onDeleteFolder}
            onOpenMenu={handleOpenMenu}
            onRenameFolder={onRenameFolder}
            onSelect={onSelect}
            onStartRenameFolder={onStartRenameFolder}
            renamingFolderPath={renamingFolderPath}
            onCanDropNote={onCanDropNote}
            onMoveNoteToFolder={onMoveNoteToFolder}
            rootPath={vaultRootPath}
            sectionCollapsed={sectionCollapsed}
            selection={selection}
            toggleFolder={toggleFolder}
            entriesByFolder={entriesByFolder}
            typeEntryMap={typeEntryMap}
            fileHandlers={noteRowHandlers}
            keyboardNav={keyboardNav}
            folderNoteDescriptions={folderNoteDescriptions}
            folderNotesExpanded={folderNotesExpanded}
            onToggleFolderNote={onToggleFolderNote}
          />
          <FolderContextMenu
            menu={contextMenu}
            menuRef={menuRef}
            onDelete={handleDeleteFromMenu}
            onReveal={handleRevealFromMenu}
            onCopyPath={handleCopyPathFromMenu}
            onCreateFolder={handleCreateFolderFromMenu}
            onCreateNote={handleCreateNoteFromMenu}
            onRename={handleRenameFromMenu}
            locale={locale}
          />
          {contextMenuNode}
        </div>
      )
    })

function useNoteFileContextMenu(handlers: WithNoteContextMenu, locale: AppLocale) {
  return useNoteContextMenu({
    locale,
    onEnterNeighborhood: handlers.onEnterNeighborhood,
    onOpenInNewWindow: handlers.onOpenInNewWindow,
    onRenameFilename: handlers.onRenameFilename,
    onArchivePaths: handlers.onArchivePaths,
    onDeletePaths: handlers.onDeletePaths,
    onExportPdf: handlers.onExportPdf,
    onToggleFavorite: handlers.onToggleFavorite,
    onToggleOrganized: handlers.onToggleOrganized,
    onRevealFile: handlers.onRevealFile,
    onCopyFilePath: handlers.onCopyFilePath,
    canCopyGitUrl: handlers.canCopyGitUrl,
    onCopyGitUrl: handlers.onCopyGitUrl,
  })
}

function FolderTreeBody(options: FolderTreeBodyProps) {
  const {
    displayedExpanded,
    displayedFolders,
    isCreating,
    locale = 'en',
    creationParent,
    onCancelCreateFolder,
    onCancelRenameFolder,
    onCreateFolderSubmit,
    onDeleteFolder,
    onOpenMenu,
    onRenameFolder,
    onSelect,
    onStartRenameFolder,
    onCanDropNote,
    onMoveNoteToFolder,
    renamingFolderPath,
    rootPath,
    sectionCollapsed,
    selection,
    toggleFolder,
    entriesByFolder,
    typeEntryMap,
    fileHandlers,
    keyboardNav,
    folderNoteDescriptions,
    folderNotesExpanded,
    onToggleFolderNote,
  } = options
  if (sectionCollapsed) return null

  return (
    <div className="flex flex-col gap-0.5 pb-2" data-testid="folder-tree-explorer">
      {displayedFolders.map((node) => (
        <ExplorerFolderRow
          key={folderNodeKey(node)}
          depth={0}
          expanded={displayedExpanded}
          node={node}
          creationParent={creationParent}
          isCreating={isCreating}
          onCancelCreateFolder={onCancelCreateFolder}
          onCreateFolderSubmit={onCreateFolderSubmit}
          onDeleteFolder={onDeleteFolder}
          onOpenMenu={onOpenMenu}
          onRenameFolder={onRenameFolder}
          onSelect={onSelect}
          onStartRenameFolder={onStartRenameFolder}
          onCanDropNote={onCanDropNote}
          onMoveNoteToFolder={onMoveNoteToFolder}
          onToggle={toggleFolder}
          onCancelRenameFolder={onCancelRenameFolder}
          locale={locale}
          renamingFolderPath={renamingFolderPath}
          rootPath={rootPath}
          selection={selection}
          entriesByFolder={entriesByFolder}
          typeEntryMap={typeEntryMap}
          fileHandlers={fileHandlers}
          keyboardNav={keyboardNav}
          folderNoteDescriptions={folderNoteDescriptions}
          folderNotesExpanded={folderNotesExpanded}
          onToggleFolderNote={onToggleFolderNote}
        />
      ))}
      {isCreating && !creationParent && (
        <div style={{ paddingLeft: 8 }}>
          <FolderNameInput
            ariaLabel={translate(locale, 'sidebar.folder.newName')}
            initialValue=""
            placeholder={translate(locale, 'sidebar.folder.name')}
            submitOnBlur={true}
            testId="new-folder-input"
            onCancel={onCancelCreateFolder}
            onSubmit={onCreateFolderSubmit}
          />
        </div>
      )}
    </div>
  )
}

function ExplorerFolderRow(options: {
  depth: number
  expanded: Record<string, boolean>
  node: FolderNode
  creationParent?: FolderCreationParent
  isCreating: boolean
  onCancelCreateFolder?: () => void
  onCreateFolderSubmit?: (value: string) => Promise<boolean>
  onDeleteFolder?: (folderPath: string) => void
  onOpenMenu: (node: FolderNode, event: ReactMouseEvent<HTMLElement>) => void
  onRenameFolder?: (folderPath: string, nextName: string) => Promise<boolean> | boolean
  onSelect: (selection: SidebarSelection) => void
  onStartRenameFolder?: (folderPath: string) => void
  onToggle: (path: string) => void
  onCancelRenameFolder?: () => void
  onCanDropNote?: (notePath: string, folderPath: string) => boolean
  onMoveNoteToFolder?: (notePath: string, folderPath: string) => Promise<unknown> | unknown
  locale?: AppLocale
  renamingFolderPath?: string | null
  rootPath?: string
  selection: SidebarSelection
  entriesByFolder: EntriesByFolder
  typeEntryMap: Record<string, VaultEntry>
  fileHandlers: WithNoteContextMenu
  keyboardNav: TreeKeyboardNav
  folderNoteDescriptions: Map<string, FolderNoteDescription>
  folderNotesExpanded: Record<string, boolean>
  onToggleFolderNote: (path: string) => void
}) {
  const {
    depth,
    expanded,
    node,
    creationParent,
    isCreating,
    onCancelCreateFolder,
    onCreateFolderSubmit,
    onDeleteFolder,
    onOpenMenu,
    onRenameFolder,
    onSelect,
    onStartRenameFolder,
    onToggle,
    onCancelRenameFolder,
    onCanDropNote,
    onMoveNoteToFolder,
    locale = 'en',
    renamingFolderPath,
    rootPath,
    selection,
    entriesByFolder,
    typeEntryMap,
    fileHandlers,
    keyboardNav,
    folderNoteDescriptions,
    folderNotesExpanded,
    onToggleFolderNote,
  } = options
  const nodeRootPath = node.rootPath ?? rootPath
  const nodeKey = folderNodeKey({ path: node.path, rootPath: nodeRootPath })
  const isExpanded = (Reflect.get(expanded, nodeKey) as boolean | undefined) ?? false
  const isSelected = selectionMatchesFolder(selection, node, rootPath)
  const canUseDefaultFolderActions = !nodeRootPath || nodeRootPath === rootPath
  const canMutateFolder = node.path.length > 0 && canUseDefaultFolderActions
  const isRenaming = canMutateFolder && renamingFolderPath === node.path
  const depthIndent = 8 + depth * TREE_INDENT_PX
  const fileCount = entriesForFolderNode(entriesByFolder, node, rootPath).length
  const hasChildren = node.children.length > 0 || fileCount > 0
  const rowKey = `folder:${nodeKey}`

  return (
    <>
      {isRenaming && onRenameFolder && onCancelRenameFolder ? (
        <div style={{ paddingLeft: depthIndent }}>
          <FolderNameInput
            ariaLabel={translate(locale, 'sidebar.folder.name')}
            initialValue={node.name}
            placeholder={translate(locale, 'sidebar.folder.name')}
            selectTextOnFocus={true}
            submitOnBlur={true}
            testId="rename-folder-input"
            onCancel={() => onCancelRenameFolder?.()}
            onSubmit={(nextName) => onRenameFolder(node.path, nextName)}
          />
        </div>
      ) : (
        <FolderExplorerRowButton
          depth={depth}
          node={node}
          hasChildren={hasChildren}
          isExpanded={isExpanded}
          isSelected={isSelected}
          onSelect={() => onSelect(selectionForFolder(node.path, nodeRootPath))}
          onOpenMenu={onOpenMenu}
          onStartRenameFolder={canMutateFolder ? onStartRenameFolder : undefined}
          onToggle={() => onToggle(nodeKey)}
          onCanDropNote={onCanDropNote}
          onMoveNoteToFolder={onMoveNoteToFolder}
          keyboardNav={keyboardNav}
          rowKey={rowKey}
        />
      )}
      {isCreating && creationParentMatchesNode(creationParent, node, rootPath) && onCancelCreateFolder && onCreateFolderSubmit && (
        <div data-testid={`folder-create-parent:${node.path}`} style={{ paddingLeft: depthIndent + TREE_INDENT_PX }}>
          <FolderNameInput
            ariaLabel={translate(locale, 'sidebar.folder.newName')}
            initialValue=""
            leftInset={TREE_INDENT_PX}
            placeholder={translate(locale, 'sidebar.folder.name')}
            submitOnBlur={true}
            testId="new-folder-input"
            onCancel={onCancelCreateFolder}
            onSubmit={onCreateFolderSubmit}
          />
        </div>
      )}
      {isExpanded && (
        <div className="relative" data-testid={`folder-children:${node.path}`}>
          {node.children.map((child) => (
            <ExplorerFolderRow
              key={folderNodeKey({
                path: child.path,
                rootPath: child.rootPath ?? rootPath,
              })}
              depth={depth + 1}
              expanded={expanded}
              node={child}
              creationParent={creationParent}
              isCreating={isCreating}
              onCancelCreateFolder={onCancelCreateFolder}
              onCreateFolderSubmit={onCreateFolderSubmit}
              onDeleteFolder={onDeleteFolder}
              onOpenMenu={onOpenMenu}
              onRenameFolder={onRenameFolder}
              onSelect={onSelect}
              onStartRenameFolder={onStartRenameFolder}
              onToggle={onToggle}
              onCancelRenameFolder={onCancelRenameFolder}
              onCanDropNote={onCanDropNote}
              onMoveNoteToFolder={onMoveNoteToFolder}
              locale={locale}
              renamingFolderPath={renamingFolderPath}
              rootPath={rootPath}
              selection={selection}
              entriesByFolder={entriesByFolder}
              typeEntryMap={typeEntryMap}
              fileHandlers={fileHandlers}
              keyboardNav={keyboardNav}
            />
          ))}
          {fileCount > 0 && (
            <FolderFiles
              node={node}
              depth={depth}
              rootPath={nodeRootPath}
              entriesByFolder={entriesByFolder}
              selection={selection}
              typeEntryMap={typeEntryMap}
              handlers={fileHandlers}
              keyboardNav={keyboardNav}
              folderNoteDescriptions={folderNoteDescriptions}
              folderNotesExpanded={folderNotesExpanded}
              onToggleFolderNote={onToggleFolderNote}
            />
          )}
        </div>
      )}
    </>
  )
}

function FolderExplorerRowButton({
  depth,
  node,
  hasChildren,
  isExpanded,
  isSelected,
  onSelect,
  onOpenMenu,
  onStartRenameFolder,
  onToggle,
  onCanDropNote,
  onMoveNoteToFolder,
  keyboardNav,
  rowKey,
}: {
  depth: number
  node: FolderNode
  hasChildren: boolean
  isExpanded: boolean
  isSelected: boolean
  onSelect: () => void
  onOpenMenu: (node: FolderNode, event: ReactMouseEvent<HTMLElement>) => void
  onStartRenameFolder?: (folderPath: string) => void
  onToggle: () => void
  onCanDropNote?: (notePath: string, folderPath: string) => boolean
  onMoveNoteToFolder?: (notePath: string, folderPath: string) => Promise<unknown> | unknown
  keyboardNav: TreeKeyboardNav
  rowKey: string
}) {
  const rowRef = useRef<HTMLDivElement | null>(null)
  useEffect(() => {
    keyboardNav.registerRow(rowKey, rowRef.current)
    return () => keyboardNav.registerRow(rowKey, null)
  }, [keyboardNav, rowKey])
  const depthIndent = 8 + depth * TREE_INDENT_PX

  const canMoveDraggedNote = useCallback(
    (dataTransfer: DataTransfer | null) => {
      const notePath = readDraggedNotePath(dataTransfer)
      return notePath && onCanDropNote?.(notePath, node.path) ? notePath : null
    },
    [node.path, onCanDropNote],
  )
  const onDragOver = useCallback(
    (event: ReactDragEvent<HTMLElement>) => {
      if (!canMoveDraggedNote(event.dataTransfer)) return
      event.preventDefault()
      event.dataTransfer.dropEffect = 'move'
    },
    [canMoveDraggedNote],
  )
  const onDrop = useCallback(
    (event: ReactDragEvent<HTMLElement>) => {
      const notePath = canMoveDraggedNote(event.dataTransfer)
      if (!notePath) return
      event.preventDefault()
      event.dataTransfer.dropEffect = 'move'
      void onMoveNoteToFolder?.(notePath, node.path)
    },
    [canMoveDraggedNote, node.path, onMoveNoteToFolder],
  )

  const handleKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'ArrowDown') {
      event.preventDefault()
      keyboardNav.focusSiblingRow(rowKey, 1)
      return
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault()
      keyboardNav.focusSiblingRow(rowKey, -1)
      return
    }
    if (event.key === 'ArrowRight' && hasChildren && !isExpanded) {
      event.preventDefault()
      onToggle()
      return
    }
    if (event.key === 'ArrowLeft' && hasChildren && isExpanded) {
      event.preventDefault()
      onToggle()
      return
    }
    if (event.key === 'Enter') {
      event.preventDefault()
      if (hasChildren && !isExpanded) onToggle()
      else onSelect()
      return
    }
    if (event.key === 'F2') {
      event.preventDefault()
      onStartRenameFolder?.(node.path)
    }
  }

  return (
    <div
      ref={rowRef}
      className={cn(
        'group relative flex items-center gap-1 rounded transition-colors',
        isSelected ? 'bg-[var(--accent-blue-light)] text-primary' : 'text-foreground hover:bg-accent',
      )}
      style={{ paddingLeft: depthIndent, borderRadius: 4, paddingTop: 2, paddingBottom: 2 }}
      tabIndex={0}
      onKeyDown={handleKeyDown}
      onDragOver={onDragOver}
      onDrop={onDrop}
      onClick={(event) => {
        if (event.detail > 1) return
        if (hasChildren) onToggle()
        onSelect()
      }}
      onContextMenu={(event) => {
        event.preventDefault()
        onSelect()
        onOpenMenu(node, event)
      }}
      onDoubleClick={() => onStartRenameFolder?.(node.path)}
      data-testid={`folder-row:${node.path}`}
    >
      <Button
        type="button"
        variant="ghost"
        className="h-auto flex-1 justify-start gap-1.5 rounded text-left text-[13px] font-medium hover:bg-transparent data-[note-drop-state=valid]:!bg-[var(--accent-blue-light)]"
        style={{ paddingTop: 4, paddingBottom: 4, paddingLeft: 0, paddingRight: 12 }}
        title={node.path || node.name}
        aria-expanded={hasChildren ? isExpanded : undefined}
        tabIndex={-1}
        data-note-drop-folder={node.path}
      >
        {hasChildren ? (
          isExpanded
            ? <CaretDown size={12} className="shrink-0 text-muted-foreground" />
            : <CaretRight size={12} className="shrink-0 text-muted-foreground" />
        ) : null}
        <span className="truncate">{node.name}</span>
      </Button>
    </div>
  )
}

function CreateFolderButton({ locale, onCreate }: { locale: AppLocale; onCreate: () => void }) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-xs"
      className="h-auto w-auto min-w-0 rounded-none p-0 text-muted-foreground hover:bg-transparent hover:text-foreground"
      data-testid="create-folder-btn"
      title={translate(locale, 'sidebar.action.createFolder')}
      aria-label={translate(locale, 'sidebar.action.createFolder')}
      onClick={(event) => {
        event.stopPropagation()
        onCreate()
      }}
    >
      <Plus size={12} className="text-muted-foreground hover:text-foreground" />
    </Button>
  )
}
