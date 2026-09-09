import { ArrowSquareOut as ExternalLink, Copy, SpeakerHigh } from '@phosphor-icons/react'
import { Component, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import {
  GridSuggestionMenuController,
  BlockNoteViewRaw,
  ComponentsContext,
  DeleteLinkButton,
  EditLinkButton,
  LinkToolbar,
  LinkToolbarController,
  SideMenuController,
  SuggestionMenuController,
  useComponentsContext,
  type useCreateBlockNote,
  useDictionary,
  type LinkToolbarProps,
  type SideMenuProps,
} from '@blocknote/react'
import { components } from '@blocknote/mantine'
import { MantineContext, MantineProvider } from '@mantine/core'
import { trackEvent } from '../lib/telemetry'
import { useDocumentThemeMode } from '../hooks/useDocumentThemeMode'
import { useEditorTheme } from '../hooks/useTheme'
import { useImageDrop, type ImageImportError } from '../hooks/useImageDrop'
import { useImageLightbox } from '../hooks/useImageLightbox'
import { createTranslator, type AppLocale } from '../lib/i18n'
import { writeClipboardText } from '../utils/clipboardText'
import { buildTypeEntryMap } from '../utils/typeColors'
import { workspacePathForEntry } from '../utils/workspaces'
import { observeNativeTextAssistanceDisabled } from '../lib/nativeTextAssistance'
import { getRuntimeStyleNonce } from '../lib/runtimeStyleNonce'
import { WikilinkSuggestionMenu, type WikilinkSuggestionItem } from './WikilinkSuggestionMenu'
import type { VaultEntry } from '../types'
import { _wikilinkEntriesRef } from './editorSchema'
import { openEditorAttachmentOrUrl } from './editorAttachmentActions'
import { insertImageBlockAfterCursor } from './editorImageInsertion'
import { useBlockNoteSideMenuHoverGuard } from './blockNoteSideMenuHoverGuard'
import { NabuSlashMenu } from './NabuSlashMenu'
import { NabuFormattingToolbar, NabuFormattingToolbarController } from './nabuEditorFormatting'
import { NabuCollapsedHeadingsController, NabuSideMenu } from './nabuBlockNoteSideMenu'
import { useEditorLinkActivation } from './useEditorLinkActivation'
import { ImageLightbox } from './ImageLightbox'
import { NabuFilePanelController } from './NabuFilePanel'
import { refreshCodeBlockSyntaxHighlighting } from './editorCodeBlockHighlightRefresh'
import { ActionTooltip } from './ui/action-tooltip'
import { Button } from './ui/button'
import { TtsPlaybackControls } from './TtsPlaybackControls'
import { useKokoroTts } from '../hooks/useKokoroTts'
import { useTtsTextSource } from '../hooks/useTtsTextSource'
import { VaultExpressionProvider } from './VaultExpressionContext'
import { subscribeRichEditorExternalChange } from './editorExternalChangeEvents'
import {
  activatePlainTextPasteTarget,
  registerPlainTextPasteTarget,
  type PlainTextPasteTarget,
} from '../utils/plainTextPaste'
import {
  blockNoteRenderRecoveryReason,
  isRecoverableBlockNoteRenderError,
  markRecoveredBlockNoteRenderError,
  type BlockNoteRenderRecoveryReason,
} from './blockNoteRenderRecovery'
import { repairEditorDocumentForRenderRecovery } from './blockNoteRenderRecoveryDocument'
import { useEditorPasteHandler } from './titleHeadingInteractions'
import {
  CODE_BLOCK_SELECTOR,
  codeBlockText,
  richEditorClipboardPayload,
  selectedCodeBlockText,
  selectedEditorDomHtml,
  selectedEditorPlainText,
  selectedEditorRange,
  writeRichEditorClipboardPayload,
} from './editorRichCopy'
import {
  buildBaseSuggestionItems,
  type SuggestionAction,
  useInsertWikilink,
  useSuggestionMenuItems,
} from './singleEditorSuggestionItems'
import {
  useEditorContainerClickHandler,
  useEditorWhitespaceMouseSelection,
} from './singleEditorPointerInteractions'

const TEST_TABLE_MARKDOWN = `| Head 1 | Head 2 | Head 3 |
| --- | --- | --- |
| A | B | C |
| D | E | F |
`
const TOOLBAR_MOUSE_DOWN_ALLOW_SELECTOR = [
  '[role="menu"]',
  '[role="dialog"]',
  'button[aria-haspopup]',
  'input',
  'textarea',
  '[contenteditable="true"]',
].join(', ')
const MAX_BLOCKNOTE_RENDER_RECOVERY_RETRIES = 1

type TestTableBlock = {
  type?: string
  content?: { type?: string; columnWidths?: Array<number | null> }
}
type BlockNoteRenderRecoveryState = {
  error: unknown
  recoveryKey: number
  retries: number
}

class BlockNoteRenderRecoveryBoundary extends Component<
  {
  children: (recoveryKey: number) => ReactNode
  onRecover?: (attempt: number, reason: BlockNoteRenderRecoveryReason) => void
  },
  BlockNoteRenderRecoveryState
> {
  state: BlockNoteRenderRecoveryState = {
    error: null,
    recoveryKey: 0,
    retries: 0,
  }

  static getDerivedStateFromError(error: unknown): Partial<BlockNoteRenderRecoveryState> {
    markRecoveredBlockNoteRenderError(error)
    return { error }
  }

  componentDidCatch(error: unknown) {
    const reason = blockNoteRenderRecoveryReason(error)
    if (!reason) return
    if (this.state.retries >= MAX_BLOCKNOTE_RENDER_RECOVERY_RETRIES) return

    const attempt = this.state.retries + 1
    trackEvent('editor_render_recovered', { reason, attempt })
    this.props.onRecover?.(attempt, reason)
    this.setState(({ recoveryKey, retries }) => ({
      error: null,
      recoveryKey: recoveryKey + 1,
      retries: retries + 1,
    }))
  }

  render() {
    if (this.state.error) {
      if (!isRecoverableBlockNoteRenderError(this.state.error)) {
        throw this.state.error
      }

      return null
    }

    return this.props.children(this.state.recoveryKey)
  }
}

function isEditorReadyForSuggestionAction(
  editor: ReturnType<typeof useCreateBlockNote>,
  container: HTMLElement | null,
) {
  if (!container?.isConnected) return false

  const editorElement = editor.domElement
  if (!(editorElement instanceof HTMLElement)) return true

  return editorElement.isConnected
}

function runSuggestionActionSafely({
  action,
  container,
  editor,
}: {
  action: SuggestionAction
  container: HTMLElement | null
  editor: ReturnType<typeof useCreateBlockNote>
}) {
  if (!isEditorReadyForSuggestionAction(editor, container)) return

  try {
    action()
  } catch (error) {
    console.warn('[editor] Ignored stale suggestion menu action:', error)
  }
}

function SharedContextBlockNoteView(props: React.ComponentProps<typeof BlockNoteViewRaw>) {
  const { children, className, theme, ...rest } = props
  const mantineContext = useContext(MantineContext)
  const colorScheme = theme === 'dark' ? 'dark' : 'light'
  const view = (
    <ComponentsContext.Provider value={components}>
      <BlockNoteViewRaw
        {...rest}
        className={['bn-mantine', className].filter(Boolean).join(' ')}
        data-mantine-color-scheme={colorScheme}
        theme={theme}
      >
        {children}
      </BlockNoteViewRaw>
    </ComponentsContext.Provider>
  )

  if (mantineContext) return view

  return (
    <MantineProvider
      // BlockNote scopes Mantine defaults under `.bn-mantine` instead of `:root`.
      withCssVariables={false}
      getStyleNonce={getRuntimeStyleNonce}
      getRootElement={() => undefined}
    >
      {view}
    </MantineProvider>
  )
}

function shouldAllowToolbarMouseDown(target: HTMLElement) {
  return Boolean(target.closest(TOOLBAR_MOUSE_DOWN_ALLOW_SELECTOR))
}

function handleToolbarMouseDownCapture(event: Pick<React.MouseEvent<HTMLElement>, 'target' | 'preventDefault'>) {
  if (!(event.target instanceof HTMLElement) || shouldAllowToolbarMouseDown(event.target)) {
    return
  }

  event.preventDefault()
}

function useRequiredComponentsContext() {
  const components = useComponentsContext()
  if (!components) throw new Error('BlockNote components context is unavailable')
  return components
}

function NabuOpenLinkButton({ url, vaultPath }: Pick<LinkToolbarProps, 'url'> & { vaultPath?: string }) {
  const Components = useRequiredComponentsContext()
  const dict = useDictionary()
  const handleOpen = useCallback(() => {
    openEditorAttachmentOrUrl({ url, vaultPath, source: 'link' })
  }, [url, vaultPath])

  return (
    <Components.LinkToolbar.Button
      className="bn-button"
      label={dict.link_toolbar.open.tooltip}
      mainTooltip={dict.link_toolbar.open.tooltip}
      isSelected={false}
      onClick={handleOpen}
      icon={<ExternalLink size={16} />}
    />
  )
}

function NabuLinkToolbar({ vaultPath, ...props }: LinkToolbarProps & { vaultPath?: string }) {
  return (
    <LinkToolbar {...props}>
      <EditLinkButton
        url={props.url}
        text={props.text}
        range={props.range}
        setToolbarOpen={props.setToolbarOpen}
        setToolbarPositionFrozen={props.setToolbarPositionFrozen}
      />
      <NabuOpenLinkButton url={props.url} vaultPath={vaultPath} />
      <DeleteLinkButton range={props.range} setToolbarOpen={props.setToolbarOpen} />
    </LinkToolbar>
  )
}

function applySeededColumnWidths(parsedBlocks: Array<TestTableBlock>, columnWidths?: Array<number | null>) {
  if (!columnWidths) return

  const tableBlock = parsedBlocks[0]
  if (tableBlock?.type !== 'table') return

  const tableContent = tableBlock.content
  if (tableContent?.type !== 'tableContent') return

  tableContent.columnWidths = [...columnWidths]
}

async function seedEditorWithTestTable(
  editor: ReturnType<typeof useCreateBlockNote>,
  columnWidths?: Array<number | null>,
) {
  const parsedBlocks = (await Promise.resolve(
    editor.tryParseMarkdownToBlocks(TEST_TABLE_MARKDOWN),
  )) as Array<TestTableBlock>

  applySeededColumnWidths(parsedBlocks, columnWidths)

  const tableMarkup = editor.blocksToHTMLLossy([
    ...parsedBlocks,
    { type: 'paragraph', content: [], children: [] },
  ] as typeof editor.document)
  editor._tiptapEditor.commands.setContent(tableMarkup)
  editor.focus()
}

function useSeedBlockNoteTableBridge(editor: ReturnType<typeof useCreateBlockNote>) {
  useEffect(() => {
    const seedBlockNoteTable = (columnWidths?: Array<number | null>) => seedEditorWithTestTable(editor, columnWidths)

    window.__laputaTest = {
      ...window.__laputaTest,
      seedBlockNoteTable,
    }

    return () => {
      if (window.__laputaTest?.seedBlockNoteTable === seedBlockNoteTable) {
        delete window.__laputaTest.seedBlockNoteTable
      }
    }
  }, [editor])
}

const CODE_BLOCK_COPY_RESET_MS = 1200

type CodeBlockCopyTarget = {
  codeBlock: HTMLElement
  left: number
  top: number
}

function codeBlockCopyTarget(codeBlock: HTMLElement, container: HTMLElement): CodeBlockCopyTarget {
  const codeBlockRect = codeBlock.getBoundingClientRect()
  const containerRect = container.getBoundingClientRect()

  return {
    codeBlock,
    left: codeBlockRect.right - containerRect.left + container.scrollLeft - 30,
    top: codeBlockRect.top - containerRect.top + container.scrollTop + 6,
  }
}

function sameCopyTarget(left: CodeBlockCopyTarget | null, right: CodeBlockCopyTarget): boolean {
  return Boolean(left && left.codeBlock === right.codeBlock && left.left === right.left && left.top === right.top)
}

function stopCopyButtonEvent(event: React.MouseEvent<HTMLButtonElement>): void {
  event.preventDefault()
  event.stopPropagation()
}

function reportCopyFailure(error: unknown): void {
  console.warn('[editor] Failed to copy code block:', error)
}

function useCodeBlockCopyTarget(containerRef: React.RefObject<HTMLDivElement | null>) {
  const [copyTarget, setCopyTarget] = useState<CodeBlockCopyTarget | null>(null)

  const showCopyTarget = useCallback(
    (codeBlock: HTMLElement) => {
    const container = containerRef.current
      if (!container?.contains(codeBlock)) return

    const nextTarget = codeBlockCopyTarget(codeBlock, container)
      setCopyTarget((previous) => (sameCopyTarget(previous, nextTarget) ? previous : nextTarget))
    },
    [containerRef],
  )

  const updateFromEventTarget = useCallback(
    (target: EventTarget | null) => {
    const container = containerRef.current
    if (!(target instanceof HTMLElement) || !container) return
    if (target.closest('[data-editor-code-copy]')) return

    const codeBlock = target.closest<HTMLElement>(CODE_BLOCK_SELECTOR)
    if (codeBlock && container.contains(codeBlock)) {
      showCopyTarget(codeBlock)
      return
    }

    setCopyTarget(null)
    },
    [containerRef, showCopyTarget],
  )

  const handleMouseMove = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
    updateFromEventTarget(event.target)
    },
    [updateFromEventTarget],
  )

  const handleFocus = useCallback(
    (event: React.FocusEvent<HTMLDivElement>) => {
    updateFromEventTarget(event.target)
    },
    [updateFromEventTarget],
  )

  const clearCopyTarget = useCallback(() => setCopyTarget(null), [])

  return { clearCopyTarget, copyTarget, handleFocus, handleMouseMove }
}

function CodeBlockCopyButton({ copyTarget, locale }: { copyTarget: CodeBlockCopyTarget; locale: AppLocale }) {
  const [active, setActive] = useState(false)
  const resetTimerRef = useRef<number | null>(null)
  const t = useMemo(() => createTranslator(locale), [locale])
  const label = t('editor.codeBlock.copy')

  useEffect(() => () => {
    if (resetTimerRef.current !== null) window.clearTimeout(resetTimerRef.current)
  }, [])

  const handleCopy = useCallback(
    (event: React.MouseEvent<HTMLButtonElement>) => {
    stopCopyButtonEvent(event)

    void writeClipboardText(codeBlockText(copyTarget.codeBlock))
      .then(() => {
        trackEvent('code_block_copied')
        setActive(true)
        if (resetTimerRef.current !== null) window.clearTimeout(resetTimerRef.current)
        resetTimerRef.current = window.setTimeout(() => {
          setActive(false)
          resetTimerRef.current = null
        }, CODE_BLOCK_COPY_RESET_MS)
      })
      .catch(reportCopyFailure)
    },
    [copyTarget],
  )

  const stopEditorMouseDown = useCallback((event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault()
    event.stopPropagation()
  }, [])

  return (
    <div
      className="editor__code-block-copy"
      contentEditable={false}
      data-editor-code-copy
      style={{ left: copyTarget.left, top: copyTarget.top }}
    >
      <ActionTooltip copy={{ label }} side="left" align="center">
        <Button
          aria-label={label}
          className="border-transparent bg-transparent text-muted-foreground shadow-none hover:bg-transparent hover:text-foreground focus-visible:bg-transparent focus-visible:text-foreground"
          data-editor-code-copy-button
          onBlur={() => setActive(false)}
          onClick={handleCopy}
          onFocus={() => setActive(true)}
          onMouseDown={stopEditorMouseDown}
          onMouseEnter={() => setActive(true)}
          onMouseLeave={() => setActive(false)}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <Copy aria-hidden="true" className="size-6" weight={active ? 'fill' : 'regular'} />
        </Button>
      </ActionTooltip>
    </div>
  )
}

function useCompositionAwareEditorChange(options: {
  containerRef: React.RefObject<HTMLDivElement | null>
  onChange?: () => void
}) {
  const { containerRef, onChange } = options
  const onChangeRef = useRef(onChange)
  const composingRef = useRef(false)
  const pendingChangeRef = useRef(false)

  useEffect(() => {
    onChangeRef.current = onChange
  }, [onChange])

  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    const flushPendingChange = () => {
      if (composingRef.current || !pendingChangeRef.current) return
      pendingChangeRef.current = false
      onChangeRef.current?.()
    }

    const handleCompositionStart = () => {
      composingRef.current = true
    }

    const handleCompositionEnd = () => {
      composingRef.current = false
      queueMicrotask(flushPendingChange)
    }

    container.addEventListener('compositionstart', handleCompositionStart, true)
    container.addEventListener('compositionend', handleCompositionEnd, true)
    return () => {
      container.removeEventListener('compositionstart', handleCompositionStart, true)
      container.removeEventListener('compositionend', handleCompositionEnd, true)
    }
  }, [containerRef])

  return useCallback(() => {
    if (composingRef.current) {
      pendingChangeRef.current = true
      return
    }

    pendingChangeRef.current = false
    onChangeRef.current?.()
  }, [])
}

function handleCodeBlockCopy(event: React.ClipboardEvent<HTMLDivElement>): boolean {
  const codeText = selectedCodeBlockText({
    selection: window.getSelection(),
    container: event.currentTarget,
  })
  if (codeText === null) return false

  event.clipboardData.setData('text/plain', codeText)
  event.preventDefault()
  return true
}

function handleSelectedEditorCopy(
  event: React.ClipboardEvent<HTMLDivElement>,
  editor: ReturnType<typeof useCreateBlockNote>,
) {
  const selection = window.getSelection()
  const range = selectedEditorRange(selection, event.currentTarget)
  if (!selection || !range) return

  const plainText = selectedEditorPlainText(selection, range)
  if (plainText === null) return

  event.clipboardData.setData('text/plain', plainText)

  const richPayload = richEditorClipboardPayload(editor)
  if (richPayload) {
    writeRichEditorClipboardPayload(event.clipboardData, richPayload)
  } else {
    const markup = selectedEditorDomHtml(range)
    if (markup.length > 0) {
      event.clipboardData.setData('text/html', markup)
    }
  }

  event.preventDefault()
}

function handleEditorCopy(event: React.ClipboardEvent<HTMLDivElement>, editor: ReturnType<typeof useCreateBlockNote>) {
  if (handleCodeBlockCopy(event)) return

  handleSelectedEditorCopy(event, editor)
}

type EditorInteractionControllersProps = ReturnType<typeof useSuggestionMenuItems> & {
  locale: AppLocale
  runEditorAction: (action: SuggestionAction) => void
  vaultPath?: string
}

function EditorInteractionControllers({
  getAtWikilinkItems,
  getEmojiItems,
  getSlashMenuItems,
  getWikilinkItems,
  locale,
  runEditorAction,
  vaultPath,
}: EditorInteractionControllersProps) {
  const sideMenu = useCallback((props: SideMenuProps) => <NabuSideMenu {...props} locale={locale} />, [locale])

  return (
    <>
      <NabuCollapsedHeadingsController />
      <SideMenuController sideMenu={sideMenu} />
      <NabuFormattingToolbarController
        formattingToolbar={(props) => <NabuFormattingToolbar {...props} locale={locale} vaultPath={vaultPath} />}
        floatingUIOptions={{
          elementProps: {
            onMouseDownCapture: handleToolbarMouseDownCapture,
          },
        }}
      />
      <LinkToolbarController
        linkToolbar={(props) => <NabuLinkToolbar {...props} vaultPath={vaultPath} />}
        floatingUIOptions={{
          elementProps: {
            onMouseDownCapture: handleToolbarMouseDownCapture,
          },
        }}
      />
      <NabuFilePanelController />
      <SuggestionMenuController
        triggerCharacter="/"
        getItems={getSlashMenuItems}
        suggestionMenuComponent={NabuSlashMenu}
      />
      <GridSuggestionMenuController triggerCharacter=":" columns={10} minQueryLength={1} getItems={getEmojiItems} />
      <SuggestionMenuController
        triggerCharacter="[["
        getItems={getWikilinkItems}
        suggestionMenuComponent={WikilinkSuggestionMenu}
        onItemClick={(item: WikilinkSuggestionItem) => runEditorAction(item.onItemClick)}
      />
      <SuggestionMenuController
        triggerCharacter="@"
        getItems={getAtWikilinkItems}
        suggestionMenuComponent={WikilinkSuggestionMenu}
        onItemClick={(item: WikilinkSuggestionItem) => runEditorAction(item.onItemClick)}
      />
    </>
  )
}

/** Insert an image block after the current cursor position. */
function useInsertImageCallback(editor: ReturnType<typeof useCreateBlockNote>) {
  const editorRef = useRef(editor)
  useEffect(() => {
    editorRef.current = editor
  }, [editor])
  return useCallback((url: string) => {
    insertImageBlockAfterCursor(editorRef.current, url)
  }, [])
}

function useRichEditorPlainTextPasteTarget(options: {
  containerRef: React.RefObject<HTMLDivElement | null>
  editable: boolean
  editor: ReturnType<typeof useCreateBlockNote>
  runEditorAction: (action: SuggestionAction) => void
}) {
  const { containerRef, editable, editor, runEditorAction } = options
  const targetRef = useRef<PlainTextPasteTarget | null>(null)

  useEffect(() => {
    const target: PlainTextPasteTarget = {
      surface: 'rich_editor',
      contains: (element) => Boolean(element && containerRef.current?.contains(element)),
      isConnected: () => containerRef.current?.isConnected === true,
      insert: (text) => {
        if (!editable) return false

        let inserted = false
        runEditorAction(() => {
          editor.focus()
          editor.insertInlineContent(text, { updateSelection: true })
          inserted = true
        })
        return inserted
      },
    }
    targetRef.current = target
    const unregister = registerPlainTextPasteTarget(target)

    return () => {
      unregister()
      if (targetRef.current === target) {
        targetRef.current = null
      }
    }
  }, [containerRef, editable, editor, runEditorAction])

  return useCallback(() => {
    if (targetRef.current) {
      activatePlainTextPasteTarget(targetRef.current)
    }
  }, [])
}

/** Single BlockNote editor view — content is swapped via replaceBlocks */
export function SingleEditorView(options: {
  currentContent?: string
  editor: ReturnType<typeof useCreateBlockNote>
  entries: VaultEntry[]
  onNavigateWikilink: (target: string) => void
  onChange?: () => void
  onImageImportError?: (error: ImageImportError) => void
  sourceEntry?: VaultEntry | null
  vaultPath?: string
  editable?: boolean
  locale?: AppLocale
}) {
  const { currentContent = '', editor, entries, onNavigateWikilink, onChange, onImageImportError, sourceEntry, vaultPath, editable = true, locale = 'en' } = options
  const { cssVars } = useEditorTheme()
  const themeMode = useDocumentThemeMode()
  const previousThemeModeRef = useRef(themeMode)
  const containerRef = useRef<HTMLDivElement>(null)
  const suppressNextContainerClickRef = useRef(false)
  const handleContainerClick = useEditorContainerClickHandler({
    editable,
    editor,
    suppressNextContainerClickRef,
    vaultPath,
  })
  const handleWhitespaceMouseSelection = useEditorWhitespaceMouseSelection({
    containerRef,
    editable,
    editor,
    suppressNextContainerClickRef,
  })
  const handleEditorChange = useCompositionAwareEditorChange({
    containerRef,
    onChange,
  })
  const onImageUrl = useInsertImageCallback(editor)
  const { isDragOver } = useImageDrop({
    containerRef,
    onImageImportError,
    onImageUrl,
    vaultPath,
  })
  const lightbox = useImageLightbox({ containerRef })
  const {
    clearCopyTarget,
    copyTarget,
    handleFocus: handleCodeBlockCopyFocus,
    handleMouseMove: handleCodeBlockCopyMouseMove,
  } = useCodeBlockCopyTarget(containerRef)
  useBlockNoteSideMenuHoverGuard(containerRef)
  useEditorLinkActivation(
    containerRef,
    onNavigateWikilink,
    vaultPath,
    sourceEntry?.path,
    (sourceEntry ? workspacePathForEntry(sourceEntry) : null) ?? vaultPath,
  )

  useEffect(() => {
    _wikilinkEntriesRef.current = entries
  }, [entries])

  useEffect(() => {
    if (previousThemeModeRef.current === themeMode) return

    previousThemeModeRef.current = themeMode
    refreshCodeBlockSyntaxHighlighting(editor)
  }, [editor, themeMode])

  useEffect(() => {
    return subscribeRichEditorExternalChange(editor, handleEditorChange)
  }, [editor, handleEditorChange])

  useEffect(() => {
    const container = containerRef.current
    if (!container) return
    return observeNativeTextAssistanceDisabled(container)
  }, [])

  useSeedBlockNoteTableBridge(editor)

  // Plan 4 §1.6A: read the current note aloud with Kokoro. Text is extracted
  // from the editor document lazily when the speaker button is pressed.
  const tts = useKokoroTts()
  const ttsSource = useTtsTextSource({
    tts,
    sourceId: 'note',
    enabled: editable,
    getText: useCallback(() => {
      try {
        return editor.blocksToMarkdownLossy(editor.document)
      } catch {
        return currentContent ?? ''
      }
    }, [currentContent, editor]),
  })

  const typeEntryMap = useMemo(() => buildTypeEntryMap(entries), [entries])
  const baseItems = useMemo(() => buildBaseSuggestionItems(entries), [entries])
  const runEditorAction = useCallback(
    (action: SuggestionAction) => {
    runSuggestionActionSafely({
      action,
      container: containerRef.current,
      editor,
    })
    },
    [editor],
  )
  const activatePlainTextPaste = useRichEditorPlainTextPasteTarget({
    containerRef,
    editable,
    editor,
    runEditorAction,
  })
  const handlePasteCapture = useEditorPasteHandler({
    editable,
    editor,
    runEditorAction,
  })
  const handleFocusCapture = useCallback(
    (event: React.FocusEvent<HTMLDivElement>) => {
    activatePlainTextPaste()
    handleCodeBlockCopyFocus(event)
    },
    [activatePlainTextPaste, handleCodeBlockCopyFocus],
  )
  const handleMouseDownCapture = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
    activatePlainTextPaste()
    handleWhitespaceMouseSelection(event)
    },
    [activatePlainTextPaste, handleWhitespaceMouseSelection],
  )
  const handleCopyCapture = useCallback(
    (event: React.ClipboardEvent<HTMLDivElement>) => {
    handleEditorCopy(event, editor)
    },
    [editor],
  )

  useEffect(() => {
    const container = containerRef.current
    if (!container) return
    const handleClick = (event: MouseEvent) => {
      handleContainerClick(event as unknown as React.MouseEvent<HTMLDivElement>)
    }
    container.addEventListener('click', handleClick)
    return () => container.removeEventListener('click', handleClick)
  }, [handleContainerClick])

  const insertWikilink = useInsertWikilink(editor, runEditorAction)
  const suggestionMenuItems = useSuggestionMenuItems({
    baseItems,
    editor,
    entries,
    insertWikilink,
    locale,
    onNavigateWikilink,
    runEditorAction,
    sourceEntry: sourceEntry ?? undefined,
    typeEntryMap,
    vaultPath,
  })

  return (
    <div
      ref={containerRef}
      role="application"
      aria-label="Rich text editor"
      className={`editor__blocknote-container${isDragOver ? ' editor__blocknote-container--drag-over' : ''}`}
      style={cssVars as React.CSSProperties}
      onCopyCapture={handleCopyCapture}
      onFocusCapture={handleFocusCapture}
      onMouseLeave={clearCopyTarget}
      onMouseDownCapture={handleMouseDownCapture}
      onMouseMove={handleCodeBlockCopyMouseMove}
      onPasteCapture={handlePasteCapture}
    >
      {isDragOver && (
        <div className="editor__drop-overlay">
          <div className="editor__drop-overlay-label">Drop image here</div>
        </div>
      )}
      <BlockNoteRenderRecoveryBoundary onRecover={(_, reason) => repairEditorDocumentForRenderRecovery(editor, reason)}>
        {(recoveryKey) => (
          <VaultExpressionProvider
            currentContent={currentContent}
            entries={entries}
            locale={locale}
            sourceEntry={sourceEntry ?? null}
            vaultPath={vaultPath ?? ''}
          >
            <SharedContextBlockNoteView
              key={recoveryKey}
              editor={editor}
              theme={themeMode}
              onChange={handleEditorChange}
              editable={editable}
              emojiPicker={false}
              formattingToolbar={false}
              linkToolbar={false}
              slashMenu={false}
              sideMenu={false}
              filePanel={false}
            >
              <EditorInteractionControllers
                {...suggestionMenuItems}
                locale={locale}
                runEditorAction={runEditorAction}
                vaultPath={vaultPath}
              />
            </SharedContextBlockNoteView>
          </VaultExpressionProvider>
        )}
      </BlockNoteRenderRecoveryBoundary>
      {copyTarget && <CodeBlockCopyButton copyTarget={copyTarget} locale={locale} />}
      {tts.status === 'ready' && (
        <div className="absolute right-4 top-2 z-20" data-testid="note-tts-speaker-wrap">
          <ActionTooltip copy={{ label: 'Read aloud' }} side="left">
            <Button
              type="button"
              variant={ttsSource.active ? 'default' : 'ghost'}
              size="icon-sm"
              aria-label="Read aloud"
              title="Read aloud"
              data-testid="note-tts-speaker"
              onClick={() => void ttsSource.speak()}
            >
              <SpeakerHigh size={15} aria-hidden />
            </Button>
          </ActionTooltip>
        </div>
      )}
      {tts.status === 'ready' && ttsSource.active && (
        <div className="absolute bottom-4 left-1/2 z-30 -translate-x-1/2">
          <TtsPlaybackControls tts={tts} />
        </div>
      )}
      <ImageLightbox image={lightbox.image} locale={locale} onClose={lightbox.close} />
    </div>
  )
}
