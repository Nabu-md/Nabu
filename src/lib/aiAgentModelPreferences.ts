import type { AiAgentId } from './aiAgents'

const STORAGE_KEY = 'nabu:ai-agent-model-preferences:v1'

type AgentModelPreferences = Partial<Record<AiAgentId, string>>

function storedPreferences(): unknown {
  return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}')
}

function validPreferences(value: unknown): AgentModelPreferences {
  if (!value || typeof value !== 'object') return {}
  if (Array.isArray(value)) return {}
  return Object.fromEntries(Object.entries(value).filter((entry): entry is [AiAgentId, string] => (
    typeof entry[1] === 'string' && Boolean(entry[1].trim())
  )))
}

function readPreferences(): AgentModelPreferences {
  if (typeof localStorage === 'undefined') return {}

  try {
    return validPreferences(storedPreferences())
  } catch {
    return {}
  }
}

function writePreferences(preferences: AgentModelPreferences): void {
  if (typeof localStorage === 'undefined') return
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(preferences))
  } catch {
    // Device preferences are optional when storage is unavailable.
  }
}

export function preferredAgentModel(agent: AiAgentId): string | null {
  return readPreferences()[agent]?.trim() || null
}

export function setPreferredAgentModel(agent: AiAgentId, modelId: string | null): void {
  const preferences = readPreferences()
  const normalized = modelId?.trim() || null
  if (normalized) preferences[agent] = normalized
  else delete preferences[agent]
  writePreferences(preferences)
}
