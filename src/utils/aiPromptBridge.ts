import type { NoteReference } from './ai-context'

export const OPEN_AI_CHAT_EVENT = 'nabu:open-ai-chat'
export const AI_PROMPT_QUEUED_EVENT = 'nabu:ai-prompt-queued'
export const NEW_AI_CHAT_EVENT = 'nabu:new-ai-chat'
export const AI_WORKSPACE_DOCK_REQUESTED_EVENT = 'nabu:ai-workspace-dock-requested'
export const AI_WORKSPACE_OPEN_NOTE_REQUESTED_EVENT = 'nabu:ai-workspace-open-note-requested'
export const AI_WORKSPACE_FILE_CREATED_EVENT = 'nabu:ai-workspace-file-created'
export const AI_WORKSPACE_FILE_MODIFIED_EVENT = 'nabu:ai-workspace-file-modified'
export const AI_WORKSPACE_VAULT_CHANGED_EVENT = 'nabu:ai-workspace-vault-changed'

export interface QueuedAiPrompt {
  id: number
  text: string
  references: NoteReference[]
  targetId?: string
}

let nextQueuedPromptId = 1
let pendingPrompt: QueuedAiPrompt | null = null

export function queueAiPrompt(
  text: string,
  references: NoteReference[],
  targetId?: string,
): QueuedAiPrompt {
  const queuedPrompt = {
    id: nextQueuedPromptId++,
    text,
    references,
    targetId,
  }
  pendingPrompt = queuedPrompt
  window.dispatchEvent(new Event(AI_PROMPT_QUEUED_EVENT))
  return queuedPrompt
}

export function takeQueuedAiPrompt(): QueuedAiPrompt | null {
  const queuedPrompt = pendingPrompt
  pendingPrompt = null
  return queuedPrompt
}

export function requestOpenAiChat() {
  window.dispatchEvent(new Event(OPEN_AI_CHAT_EVENT))
}

export function requestNewAiChat() {
  requestOpenAiChat()
  window.setTimeout(() => window.dispatchEvent(new Event(NEW_AI_CHAT_EVENT)), 0)
}

export function requestDockAiWorkspace() {
  window.dispatchEvent(new Event(AI_WORKSPACE_DOCK_REQUESTED_EVENT))
}
