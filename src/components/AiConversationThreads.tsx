import { memo, useCallback } from 'react'
import { Clock, Plus, Trash } from '@phosphor-icons/react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { translate, type AppLocale } from '../lib/i18n'
import type { ConversationRecord } from '../hooks/useAiConversations'

export interface AiConversationThreadsProps {
  conversations: ConversationRecord[]
  activeConversationId: string | null
  locale?: AppLocale
  collapsed?: boolean
  onToggleCollapsed?: () => void
  onNewChat: () => void
  onSelect: (id: string) => void
  onDelete?: (id: string) => void
}

function relativeTime(iso: string): string {
  const timestamp = Date.parse(iso)
  if (Number.isNaN(timestamp)) return ''
  const minutes = Math.round((Date.now() - timestamp) / 60_000)
  if (minutes < 1) return 'just now'
  if (minutes < 60) return `${minutes}m`
  const hours = Math.round(minutes / 60)
  if (hours < 24) return `${hours}h`
  const days = Math.round(hours / 24)
  return `${days}d`
}

function lastMessagePreview(conversation: ConversationRecord): string {
  const last = conversation.messages[conversation.messages.length - 1]
  if (!last) return 'No messages yet'
  const text = last.content.trim().replace(/\s+/g, ' ')
  return text.length > 60 ? `${text.slice(0, 60)}…` : text
}

/**
 * Phase 4 chat thread list for the AI panel: saved conversations with
 * new-chat, select, and delete actions. Collapsible to a slim rail.
 */
export const AiConversationThreads = memo(function AiConversationThreads({
  conversations,
  activeConversationId,
  locale = 'en',
  collapsed = false,
  onToggleCollapsed,
  onNewChat,
  onSelect,
  onDelete,
}: AiConversationThreadsProps) {
  const handleDelete = useCallback((id: string) => {
    onDelete?.(id)
  }, [onDelete])

  if (collapsed) {
    return (
      <div
        className="flex shrink-0 flex-col items-center gap-1 border-r border-border py-2"
        style={{ width: 36 }}
        data-testid="ai-threads-rail"
      >
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          className="h-6 w-6"
          onClick={onNewChat}
          aria-label={translate(locale, 'ai.threads.newChat')}
          title={translate(locale, 'ai.threads.newChat')}
          data-testid="ai-threads-new"
        >
          <Plus size={14} />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          className="h-6 w-6"
          onClick={onToggleCollapsed}
          aria-label={translate(locale, 'ai.threads.expand')}
          title={translate(locale, 'ai.threads.expand')}
          data-testid="ai-threads-expand"
        >
          <Clock size={14} />
        </Button>
      </div>
    )
  }

  return (
    <div
      className="flex shrink-0 flex-col border-r border-border bg-sidebar/60"
      style={{ width: 200 }}
      data-testid="ai-threads"
    >
      <div className="flex items-center justify-between px-2 py-2">
        <span className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
          {translate(locale, 'ai.threads.title')}
        </span>
        <div className="flex items-center gap-0.5">
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            className="h-6 w-6"
            onClick={onNewChat}
            aria-label={translate(locale, 'ai.threads.newChat')}
            title={translate(locale, 'ai.threads.newChat')}
            data-testid="ai-threads-new"
          >
            <Plus size={14} />
          </Button>
          {onToggleCollapsed && (
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              className="h-6 w-6"
              onClick={onToggleCollapsed}
              aria-label={translate(locale, 'ai.threads.collapse')}
              title={translate(locale, 'ai.threads.collapse')}
              data-testid="ai-threads-collapse"
            >
              <Clock size={14} />
            </Button>
          )}
        </div>
      </div>
      <div className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto px-1.5 pb-2">
        {conversations.length === 0 ? (
          <p className="px-2 py-3 text-[11px] text-muted-foreground">
            {translate(locale, 'ai.threads.empty')}
          </p>
        ) : (
          conversations.map((conversation) => {
            const active = conversation.id === activeConversationId
            return (
              <div
                key={conversation.id}
                className={cn(
                  'group relative rounded-md px-2 py-1.5 transition-colors',
                  active ? 'bg-[var(--hover)]' : 'hover:bg-[var(--hover)]',
                )}
              >
                <button
                  type="button"
                  className="block w-full text-left"
                  onClick={() => onSelect(conversation.id)}
                  data-testid={`ai-thread-${conversation.id}`}
                  aria-current={active || undefined}
                >
                  <span className={cn('block truncate text-[12px]', active ? 'font-semibold text-foreground' : 'text-foreground/90')}>
                    {conversation.title}
                  </span>
                  <span className="block truncate text-[11px] text-muted-foreground">
                    {lastMessagePreview(conversation)}
                  </span>
                  <span className="block text-[10px] text-muted-foreground/70">
                    {relativeTime(conversation.updated_at)}
                  </span>
                </button>
                {onDelete && (
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-xs"
                    className="absolute right-1 top-1 h-5 w-5 p-0 opacity-0 transition-opacity group-hover:opacity-100 focus:opacity-100"
                    onClick={(event) => {
                      event.stopPropagation()
                      handleDelete(conversation.id)
                    }}
                    aria-label={translate(locale, 'ai.threads.delete')}
                    title={translate(locale, 'ai.threads.delete')}
                    data-testid={`ai-thread-delete-${conversation.id}`}
                  >
                    <Trash size={11} />
                  </Button>
                )}
              </div>
            )
          })
        )}
      </div>
    </div>
  )
})
