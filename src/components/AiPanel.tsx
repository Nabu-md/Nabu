import { useCallback, useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode, type RefObject } from 'react'
import { AiClarifyingForm } from './AiClarifyingForm'
import { AiPanelComposer, AiPanelHeader, AiPanelMessageHistory } from './AiPanelChrome'
import { AiConversationThreads } from './AiConversationThreads'
import { DEFAULT_AI_AGENT, getAiAgentDefinition, type AiAgentId, type AiAgentReadiness } from '../lib/aiAgents'
import type { AiTarget } from '../lib/aiTargets'
import type { AppLocale } from '../lib/i18n'
import type { NoteListItem } from '../utils/ai-context'
import type { VaultEntry } from '../types'
import { trackEvent } from '../lib/telemetry'
import { useAiConversations, type ConversationRecord } from '../hooks/useAiConversations'
import { useAiPanelController, type AiPanelController } from './useAiPanelController'
import { useAiPanelPromptQueue } from './useAiPanelPromptQueue'
import { useAiPanelFocus } from './useAiPanelFocus'
import { useClarifyingForm } from './useClarifyingForm'
import type { AiAgentMessage } from '../hooks/useCliAiAgent'
import { resumeEditorFocus, useInspectorFocusBoundary } from '../hooks/editorFocusOwnership'

export type { AiAgentMessage } from '../hooks/useCliAiAgent'

interface AiPanelProps {
  onClose: () => void
  onOpenNote?: (path: string) => void
  onUnsupportedAiPaste?: (message: string) => void
  defaultAiAgent?: AiAgentId
  defaultAiTarget?: AiTarget
  defaultAiAgentReadiness?: AiAgentReadiness
  defaultAiAgentReady?: boolean
  locale?: AppLocale
  onFileCreated?: (relativePath: string) => void
  onFileModified?: (relativePath: string) => void
  onVaultChanged?: () => void
  vaultPath: string
  vaultPaths?: string[]
  activeEntry?: VaultEntry | null
  /** Direct content of the active note from the editor tab. */
  activeNoteContent?: string | null
  entries?: VaultEntry[]
  openTabs?: VaultEntry[]
  noteList?: NoteListItem[]
  noteListFilter?: { type: string | null; query: string }
}

interface AiPanelViewProps {
  controller: AiPanelController
  onClose: () => void
  onOpenNote?: (path: string) => void
  onUnsupportedAiPaste?: (message: string) => void
  defaultAiAgent?: AiAgentId
  defaultAiTarget?: AiTarget
  defaultAiAgentReadiness?: AiAgentReadiness
  defaultAiAgentReady?: boolean
  locale?: AppLocale
  activeEntry?: VaultEntry | null
  entries?: VaultEntry[]
  interactive?: boolean
  showHeader?: boolean
  showLeftBorder?: boolean
  surface?: 'default' | 'sidebar'
  composerControls?: ReactNode
  onForkMessage?: (messageId: string) => void
  onQueuedPromptTarget?: (targetId: string) => void
  onSendPrompt?: (text: string) => void
  onMessageHistoryScrollStateChange?: (scrolled: boolean) => void
  targetId?: string
  vaultPath?: string
  vaultPaths?: string[]
  /** Deep research runs inline in this same chat surface when true. */
  researchMode?: boolean
}

function readinessFromReadyFlag(ready: boolean | undefined): AiAgentReadiness {
  return (ready ?? true) ? 'ready' : 'missing'
}

function resolveDefaultAgent(agent: AiAgentId | undefined): AiAgentId {
  return agent ?? DEFAULT_AI_AGENT
}

function resolveDefaultReadiness(
  readiness: AiAgentReadiness | undefined,
  ready: boolean | undefined,
): AiAgentReadiness {
  return readiness ?? readinessFromReadyFlag(ready)
}

function resolveDefaultTarget(target: AiTarget | undefined, agent: AiAgentId) {
  if (target) return { label: target.label, kind: target.kind }
  return { label: getAiAgentDefinition(agent).label, kind: 'agent' as const }
}

interface AiPanelViewModel {
  agentLabel: string
  defaultAiAgent: AiAgentId
  defaultAiAgentReadiness: AiAgentReadiness
  targetKind: AiTarget['kind']
}

function resolveAiPanelViewModel({
  defaultAiAgent,
  defaultAiAgentReadiness,
  defaultAiAgentReady,
  defaultAiTarget,
}: {
  defaultAiAgent?: AiAgentId
  defaultAiAgentReadiness?: AiAgentReadiness
  defaultAiAgentReady?: boolean
  defaultAiTarget?: AiTarget
}): AiPanelViewModel {
  const resolvedAgent = resolveDefaultAgent(defaultAiAgent)
  const resolvedReadiness = resolveDefaultReadiness(defaultAiAgentReadiness, defaultAiAgentReady)
  const resolvedTarget = resolveDefaultTarget(defaultAiTarget, resolvedAgent)

  return {
    agentLabel: resolvedTarget.label,
    defaultAiAgent: resolvedAgent,
    defaultAiAgentReadiness: resolvedReadiness,
    targetKind: resolvedTarget.kind,
  }
}

function aiPanelFrameStyle(isActive: boolean, showLeftBorder: boolean): CSSProperties {
  return {
    outline: 'none',
    borderLeft: showLeftBorder ? (isActive ? '2px solid var(--accent-blue)' : '1px solid var(--border)') : undefined,
    animation: showLeftBorder && isActive ? 'ai-border-pulse 2s ease-in-out infinite' : undefined,
    transition: showLeftBorder ? 'border-color 0.3s ease' : undefined,
  }
}

function useReleaseEditorFocusOnUnmount(panelRef: RefObject<HTMLElement | null>): void {
  useLayoutEffect(() => () => {
    const panel = panelRef.current
    if (panel?.contains(document.activeElement)) resumeEditorFocus()
  }, [panelRef])
}

function AiPanelFrame({
  children,
  isActive,
  panelRef,
  showLeftBorder,
  surface,
}: {
  children: ReactNode
  isActive: boolean
  panelRef: RefObject<HTMLElement | null>
  showLeftBorder: boolean
  surface: 'default' | 'sidebar'
}) {
  useInspectorFocusBoundary(panelRef)
  useReleaseEditorFocusOnUnmount(panelRef)

  return (
    <aside
      ref={panelRef}
      tabIndex={-1}
      className={`flex flex-1 flex-col overflow-hidden ${surface === 'sidebar' ? 'bg-sidebar text-sidebar-foreground' : 'bg-background text-foreground'}`}
      style={aiPanelFrameStyle(isActive, showLeftBorder)}
      data-testid="ai-panel"
      data-ai-active={isActive || undefined}
    >
      {children}
    </aside>
  )
}

export function AiPanelView(options: AiPanelViewProps) {
  const {
    controller,
    onClose,
    onOpenNote,
    onUnsupportedAiPaste,
    defaultAiAgent: providedDefaultAiAgent,
    defaultAiTarget,
    defaultAiAgentReadiness: providedDefaultAiAgentReadiness,
    defaultAiAgentReady: providedDefaultAiAgentReady,
    locale = 'en',
    entries,
    interactive = true,
    showHeader = true,
    showLeftBorder = true,
    surface = 'default',
    composerControls,
    onForkMessage,
    onQueuedPromptTarget,
    onSendPrompt,
    onMessageHistoryScrollStateChange,
    targetId,
    vaultPath,
    researchMode,
  } = options
  const view = resolveAiPanelViewModel({
    defaultAiAgent: providedDefaultAiAgent,
    defaultAiAgentReadiness: providedDefaultAiAgentReadiness,
    defaultAiAgentReady: providedDefaultAiAgentReady,
    defaultAiTarget,
  })
  const inputRef = useRef<HTMLDivElement>(null)
  const panelRef = useRef<HTMLElement>(null)
  const {
        agent,
        input,
        setInput,
        hasContext,
        isActive,
        permissionMode,
        handleSend,
        handleStop,
        handleNavigateWikilink,
        handlePermissionModeChange,
        handleNewChat,
      } = controller

      useAiPanelPromptQueue({
        agent,
        currentTargetId: targetId,
        input,
        isActive,
        onTargetChange: onQueuedPromptTarget,
        setInput,
        enabled: interactive,
      })
      useAiPanelFocus({
        inputRef,
        panelRef,
        hasMessages: agent.messages.length > 0,
        isActive,
        onClose,
        enabled: interactive,
      })
      const handleComposerSend = useCallback(
        (text: string, references: Parameters<typeof handleSend>[1]) => {
        if (!text.trim() || isActive) return
        onSendPrompt?.(text)
        handleSend(text, references)
        },
        [handleSend, isActive, onSendPrompt],
      )
  const threads = useAiThreadsIntegration({
    vaultPath: vaultPath ?? null,
    messages: agent.messages,
    isActive,
    onClearConversation: handleNewChat,
  })
  const clarifyingForm = useClarifyingForm(interactive)
  // Deep research is a mode, not a separate panel: the normal chat composer
  // stays, and prompts run through the same conversation. The backend routes
  // on the deep_research permission mode; we only track the research start
  // here for analytics from the new location.
  const handleResearchComposerSend = useCallback(
    (text: string, references: Parameters<typeof handleSend>[1]) => {
      if (!text.trim() || isActive) return
      onSendPrompt?.(text)
      trackEvent('deep_research_started')
      handleSend(`Deep research request: ${text}`, references)
    },
    [handleSend, isActive, onSendPrompt],
  )
  const composerSend = researchMode ? handleResearchComposerSend : handleComposerSend

      const panelBody = (
        <>
          {showHeader && (
            <AiPanelHeader
              agentLabel={view.agentLabel}
              agentReadiness={view.defaultAiAgentReadiness}
              targetKind={view.targetKind}
              locale={locale}
              permissionMode={permissionMode}
              permissionModeDisabled={isActive}
              onPermissionModeChange={handlePermissionModeChange}
              onClose={onClose}
              onNewChat={threads ? threads.handleNewChat : handleNewChat}
            />
          )}
          <AiPanelMessageHistory
            agentLabel={view.agentLabel}
            agentReadiness={view.defaultAiAgentReadiness}
            locale={locale}
            messages={agent.messages}
            isActive={isActive}
            onForkMessage={onForkMessage}
            onOpenNote={onOpenNote}
            onNavigateWikilink={handleNavigateWikilink}
            onRegenerateMessage={agent.regenerateMessage}
            onScrollStateChange={onMessageHistoryScrollStateChange}
            hasContext={hasContext}
          />
          {clarifyingForm && <AiClarifyingForm form={clarifyingForm} locale={locale} />}
          <AiPanelComposer
            entries={entries ?? []}
            agentLabel={view.agentLabel}
            agentReadiness={view.defaultAiAgentReadiness}
            locale={locale}
            input={input}
            inputRef={inputRef}
            isActive={isActive}
            controls={composerControls}
            onChange={setInput}
            onSend={composerSend}
            onStop={handleStop}
            onUnsupportedAiPaste={onUnsupportedAiPaste}
          />
        </>
      )

      if (!threads) {
        return (
          <AiPanelFrame panelRef={panelRef} isActive={isActive} showLeftBorder={showLeftBorder} surface={surface}>
            {panelBody}
          </AiPanelFrame>
        )
      }

      return (
        <AiPanelFrame panelRef={panelRef} isActive={isActive} showLeftBorder={showLeftBorder} surface={surface}>
          <div className="flex min-h-0 flex-1">
            <AiConversationThreads
              conversations={threads.conversations}
              activeConversationId={threads.activeConversationId}
              locale={locale}
              collapsed={threads.threadsCollapsed}
              onToggleCollapsed={threads.toggleThreadsCollapsed}
              onNewChat={threads.handleNewChat}
              onSelect={threads.handleSelectThread}
              onDelete={threads.handleDeleteThread}
            />
            <div className="flex min-w-0 flex-1 flex-col">{panelBody}</div>
          </div>
        </AiPanelFrame>
      )
    }

interface AiThreadsIntegration {
  conversations: ConversationRecord[]
  activeConversationId: string | null
  threadsCollapsed: boolean
  toggleThreadsCollapsed: () => void
  handleNewChat: () => void
  handleSelectThread: (id: string) => void
  handleDeleteThread: (id: string) => void
}

function useAiThreadsIntegration({
  vaultPath,
  messages,
  isActive,
  onClearConversation,
}: {
  vaultPath: string | null
  messages: AiAgentMessage[]
  isActive: boolean
  onClearConversation: () => void
}): AiThreadsIntegration | null {
  const [threadsCollapsed, setThreadsCollapsed] = useState(false)
  const conversations = useAiConversations({ vaultPath: vaultPath ?? '', enabled: !!vaultPath })
  const activeThreadIdRef = useRef<string | null>(null)
  const savedExchangeCountRef = useRef(0)

  const toggleThreadsCollapsed = useCallback(() => setThreadsCollapsed((current) => !current), [])
  const handleNewChat = useCallback(() => {
    activeThreadIdRef.current = null
    savedExchangeCountRef.current = 0
    onClearConversation()
  }, [onClearConversation])
  const handleSelectThread = useCallback((id: string) => {
    const thread = conversations.conversations.find((conversation) => conversation.id === id)
    activeThreadIdRef.current = id
    savedExchangeCountRef.current = thread?.messages.length ?? 0
    conversations.selectConversation(id)
  }, [conversations])
  const handleDeleteThread = useCallback((id: string) => {
    if (activeThreadIdRef.current === id) {
      activeThreadIdRef.current = null
      savedExchangeCountRef.current = 0
    }
    void conversations.deleteConversation(id)
  }, [conversations])

  // Ensure there is always a thread to write into once the first message lands.
  useEffect(() => {
    if (!vaultPath) return
    if (messages.length === 0) {
      activeThreadIdRef.current = null
      savedExchangeCountRef.current = 0
      return
    }
    if (activeThreadIdRef.current) return
    if (isActive) return
    void conversations.createConversation().then((created) => {
      activeThreadIdRef.current = created.id
    })
  }, [conversations, isActive, messages.length, vaultPath])

  // Auto-save after each completed exchange (user prompt + assistant reply).
  useEffect(() => {
    if (!vaultPath || !activeThreadIdRef.current) return
    const completed = messages.filter((message) => !message.isStreaming).length
    if (completed === 0 || completed === savedExchangeCountRef.current) return
    if (isActive) return
    savedExchangeCountRef.current = completed
    const firstUser = messages.find((message) => message.userMessage?.trim())
    void conversations.saveConversation(activeThreadIdRef.current, messages.map((message) => ({
      role: message.userMessage && !message.response ? 'user' : 'assistant',
      content: message.userMessage && !message.response
        ? message.userMessage
        : message.response ?? '',
      created_at: new Date().toISOString(),
    })), { titleFromFirstMessage: firstUser?.userMessage })
  }, [conversations, isActive, messages, vaultPath])

  if (!vaultPath) return null

  return {
    conversations: conversations.conversations,
    activeConversationId: conversations.activeConversationId,
    threadsCollapsed,
    toggleThreadsCollapsed,
    handleNewChat,
    handleSelectThread,
    handleDeleteThread,
  }
}

    export function AiPanel(options: AiPanelProps) {
      const {
      onClose,
      onOpenNote,
      onUnsupportedAiPaste,
      defaultAiAgent: providedDefaultAiAgent,
      defaultAiTarget,
      defaultAiAgentReadiness: providedDefaultAiAgentReadiness,
      defaultAiAgentReady: providedDefaultAiAgentReady,
      locale = 'en',
      onFileCreated,
      onFileModified,
      onVaultChanged,
      vaultPath,
      vaultPaths,
      activeEntry,
      activeNoteContent,
      entries,
      openTabs,
      noteList,
      noteListFilter,
  } = options
  const defaultAiAgentReadiness = providedDefaultAiAgentReadiness ?? readinessFromReadyFlag(providedDefaultAiAgentReady)
  const controller = useAiPanelController({
    vaultPath,
    vaultPaths,
    defaultAiAgent: providedDefaultAiAgent ?? DEFAULT_AI_AGENT,
    defaultAiTarget,
    defaultAiAgentReady: providedDefaultAiAgentReady ?? true,
    defaultAiAgentReadiness,
    activeEntry,
    activeNoteContent,
    entries,
    openTabs,
    noteList,
    noteListFilter,
    locale,
    onOpenNote,
    onFileCreated,
    onFileModified,
    onVaultChanged,
  })

  return (
    <AiPanelView
      controller={controller}
      onClose={onClose}
      onOpenNote={onOpenNote}
      onUnsupportedAiPaste={onUnsupportedAiPaste}
      defaultAiAgent={providedDefaultAiAgent}
      defaultAiTarget={defaultAiTarget}
      defaultAiAgentReadiness={defaultAiAgentReadiness}
      defaultAiAgentReady={providedDefaultAiAgentReady}
      locale={locale}
      activeEntry={activeEntry}
      entries={entries}
      targetId={defaultAiTarget?.id}
    />
  )
}
