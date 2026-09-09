import { useCallback, useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { isTauri, mockInvoke } from '../mock-tauri'

export interface ConversationMessageRecord {
  role: 'user' | 'assistant' | 'system'
  content: string
  created_at: string
}

export interface ConversationRecord {
  id: string
  title: string
  created_at: string
  updated_at: string
  messages: ConversationMessageRecord[]
}

const STORAGE_PREFIX = 'nabu:ai-conversations:'

function storageKey(vaultPath: string): string {
  return `${STORAGE_PREFIX}${vaultPath}`
}

function readLocalConversations(vaultPath: string): ConversationRecord[] {
  try {
    const raw = localStorage.getItem(storageKey(vaultPath))
    if (!raw) return []
    const parsed = JSON.parse(raw) as ConversationRecord[]
    return Array.isArray(parsed) ? parsed : []
  } catch {
    return []
  }
}

function writeLocalConversations(vaultPath: string, conversations: ConversationRecord[]): void {
  try {
    localStorage.setItem(storageKey(vaultPath), JSON.stringify(conversations))
  } catch {
    // Storage may be unavailable; best-effort persistence only.
  }
}

function conversationInvoke<T>(command: string, args: Record<string, unknown>): Promise<T> {
  return isTauri() ? invoke<T>(command, args) : mockInvoke<T>(command, args)
}

function titleFromMessage(text: string): string {
  const trimmed = text.trim().replace(/\s+/g, ' ')
  if (!trimmed) return 'New chat'
  return trimmed.length > 50 ? `${trimmed.slice(0, 50)}…` : trimmed
}

export interface UseAiConversationsOptions {
  vaultPath: string
  enabled?: boolean
}

/**
 * Phase 4 chat threads: persisted conversation list for the AI panel with
 * create/select/save semantics. Desktop persistence goes through the
 * conversations IPC; browser mode falls back to localStorage.
 */
export function useAiConversations({ vaultPath, enabled = true }: UseAiConversationsOptions) {
  const [conversations, setConversations] = useState<ConversationRecord[]>([])
  const [activeConversationId, setActiveConversationId] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const loadedVaultRef = useRef<string | null>(null)

  const refresh = useCallback(async () => {
    if (!enabled || !vaultPath) return
    setLoading(true)
    try {
      const list = await conversationInvoke<ConversationRecord[]>('list_conversations', { vaultPath })
      setConversations(Array.isArray(list) ? list : [])
      loadedVaultRef.current = vaultPath
    } catch (error) {
      console.warn('Failed to list conversations:', error)
      const local = readLocalConversations(vaultPath)
      setConversations(local)
      loadedVaultRef.current = vaultPath
    } finally {
      setLoading(false)
    }
  }, [enabled, vaultPath])

  useEffect(() => {
    if (loadedVaultRef.current === vaultPath) return
    void refresh()
  }, [refresh, vaultPath])

  const createConversation = useCallback(async (title?: string): Promise<ConversationRecord> => {
    const record: ConversationRecord = {
      id: `local-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      title: title ?? 'New chat',
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
      messages: [],
    }
    if (isTauri() && vaultPath) {
      try {
        const created = await conversationInvoke<ConversationRecord>('create_conversation', {
          vaultPath,
          title: record.title,
        })
        setConversations((current) => [created, ...current])
        setActiveConversationId(created.id)
        return created
      } catch (error) {
        console.warn('Failed to create conversation on disk, using local fallback:', error)
      }
    }
    setConversations((current) => {
      const next = [record, ...current]
      if (vaultPath) writeLocalConversations(vaultPath, next)
      return next
    })
    setActiveConversationId(record.id)
    return record
  }, [vaultPath])

  const selectConversation = useCallback((id: string) => {
    setActiveConversationId(id)
  }, [])

  const saveConversation = useCallback(async (
    id: string,
    messages: ConversationMessageRecord[],
    options: { titleFromFirstMessage?: string } = {},
  ): Promise<void> => {
    if (!vaultPath || !id) return
    const now = new Date().toISOString()
    let saved = false
    if (isTauri()) {
      try {
        const existing = conversations.find((conversation) => conversation.id === id)
        const conversation: ConversationRecord = {
          id,
          title: existing?.id === id && existing.title !== 'New chat'
            ? existing.title
            : options.titleFromFirstMessage
              ? titleFromMessage(options.titleFromFirstMessage)
              : existing?.title ?? 'New chat',
          created_at: existing?.created_at ?? now,
          updated_at: now,
          messages,
        }
        await conversationInvoke<null>('save_conversation', { vaultPath, conversation })
        setConversations((current) => {
          const without = current.filter((candidate) => candidate.id !== id)
          return [conversation, ...without].sort((left, right) => right.updated_at.localeCompare(left.updated_at))
        })
        saved = true
      } catch (error) {
        console.warn('Failed to persist conversation via IPC, using local fallback:', error)
      }
    }
    if (!saved) {
      setConversations((current) => {
        const next = current.map((conversation) => (
          conversation.id === id
            ? {
              ...conversation,
              title: conversation.title !== 'New chat'
                ? conversation.title
                : options.titleFromFirstMessage
                  ? titleFromMessage(options.titleFromFirstMessage)
                  : conversation.title,
              updated_at: now,
              messages,
            }
            : conversation
        ))
        writeLocalConversations(vaultPath, next)
        return next
      })
    }
  }, [conversations, vaultPath])

  const deleteConversation = useCallback(async (id: string): Promise<void> => {
    if (!vaultPath || !id) return
    if (isTauri()) {
      try {
        await conversationInvoke<null>('delete_conversation', { vaultPath, id })
      } catch (error) {
        console.warn('Failed to delete conversation:', error)
      }
    }
    setConversations((current) => {
      const next = current.filter((conversation) => conversation.id !== id)
      if (!isTauri()) writeLocalConversations(vaultPath, next)
      return next
    })
    setActiveConversationId((current) => (current === id ? null : current))
  }, [vaultPath])

  const activeConversation = conversations.find((conversation) => conversation.id === activeConversationId) ?? null

  return {
    conversations,
    activeConversation,
    activeConversationId,
    loading,
    refresh,
    createConversation,
    selectConversation,
    saveConversation,
    deleteConversation,
    setActiveConversationId,
  }
}

export { titleFromMessage }
