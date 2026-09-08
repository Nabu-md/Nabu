import { describe, expect, it } from 'vitest'
import {
  AI_AGENT_PERMISSION_MODE_LABELS,
  DEFAULT_AI_AGENT_PERMISSION_MODE,
  aiAgentPermissionModeMarker,
  normalizeAiAgentPermissionMode,
} from './aiAgentPermissionMode'

describe('aiAgentPermissionMode', () => {
  it('defaults missing, null, and unknown values to vault safe mode', () => {
    expect(DEFAULT_AI_AGENT_PERMISSION_MODE).toBe('safe')
    expect(normalizeAiAgentPermissionMode(undefined)).toBe('safe')
    expect(normalizeAiAgentPermissionMode(null)).toBe('safe')
    expect(normalizeAiAgentPermissionMode('danger')).toBe('safe')
  })

  it('preserves known permission modes and exposes compact labels', () => {
    expect(normalizeAiAgentPermissionMode('safe')).toBe('safe')
    expect(normalizeAiAgentPermissionMode('power_user')).toBe('power_user')
    expect(normalizeAiAgentPermissionMode('deep_research')).toBe('deep_research')
    expect(normalizeAiAgentPermissionMode('mini_app_builder')).toBe('mini_app_builder')
    expect(normalizeAiAgentPermissionMode('rag')).toBe('rag')
    expect(AI_AGENT_PERMISSION_MODE_LABELS.safe.short).toBe('Safe')
    expect(AI_AGENT_PERMISSION_MODE_LABELS.safe.control).toBe('Vault Safe')
    expect(AI_AGENT_PERMISSION_MODE_LABELS.power_user.short).toBe('Power User')
    expect(AI_AGENT_PERMISSION_MODE_LABELS.deep_research.short).toBe('Deep Research')
    expect(AI_AGENT_PERMISSION_MODE_LABELS.deep_research.control).toBe('Deep Research')
    expect(AI_AGENT_PERMISSION_MODE_LABELS.mini_app_builder.short).toBe('Mini App Builder')
    expect(AI_AGENT_PERMISSION_MODE_LABELS.mini_app_builder.control).toBe('Mini App Builder')
    expect(AI_AGENT_PERMISSION_MODE_LABELS.rag.short).toBe('RAG')
    expect(AI_AGENT_PERMISSION_MODE_LABELS.rag.control).toBe('Semantic Search')
  })

  it('formats a local transcript marker for mode changes', () => {
    expect(aiAgentPermissionModeMarker('power_user')).toBe(
      'AI permission mode changed to Power User. It will apply to the next message.',
    )
  })
})
