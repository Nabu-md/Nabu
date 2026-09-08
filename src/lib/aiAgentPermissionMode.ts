import { createTranslator, type AppLocale } from './i18n'

export type AiAgentPermissionMode = 'safe' | 'power_user' | 'deep_research' | 'mini_app_builder' | 'rag'

export const DEFAULT_AI_AGENT_PERMISSION_MODE: AiAgentPermissionMode = 'safe'

export const AI_AGENT_PERMISSION_MODE_LABELS: Record<
  AiAgentPermissionMode,
  { short: string; control: string }
> = {
  safe: {
    short: 'Safe',
    control: 'Vault Safe',
  },
  power_user: {
    short: 'Power User',
    control: 'Power User',
  },
  deep_research: {
    short: 'Deep Research',
    control: 'Deep Research',
  },
  mini_app_builder: {
    short: 'Mini App Builder',
    control: 'Mini App Builder',
  },
  rag: {
    short: 'RAG',
    control: 'Semantic Search',
  },
}

export function normalizeAiAgentPermissionMode(value: unknown): AiAgentPermissionMode {
  if (value === 'power_user') return 'power_user'
  if (value === 'deep_research') return 'deep_research'
  if (value === 'mini_app_builder') return 'mini_app_builder'
  if (value === 'rag') return 'rag'
  return DEFAULT_AI_AGENT_PERMISSION_MODE
}

export function aiAgentPermissionModeLabels(
  mode: AiAgentPermissionMode,
  locale: AppLocale = 'en',
): { short: string; control: string } {
  const t = createTranslator(locale)
  if (mode === 'power_user') {
    return {
      short: t('ai.permission.powerUser.short'),
      control: t('ai.permission.powerUser.control'),
    }
  }
  if (mode === 'deep_research') {
    return {
      short: t('ai.permission.deepResearch.short'),
      control: t('ai.permission.deepResearch.control'),
    }
  }
  if (mode === 'mini_app_builder') {
    return {
      short: t('ai.permission.miniAppBuilder.short'),
      control: t('ai.permission.miniAppBuilder.control'),
    }
  }
  if (mode === 'rag') {
    return {
      short: t('ai.permission.rag.short'),
      control: t('ai.permission.rag.control'),
    }
  }
  return {
    short: t('ai.permission.safe.short'),
    control: t('ai.permission.safe.control'),
  }
}

export function aiAgentPermissionModeTooltipKey(
  mode: AiAgentPermissionMode,
): 'ai.permission.safe.tooltip' | 'ai.permission.powerUser.tooltip' | 'ai.permission.deepResearch.tooltip' | 'ai.permission.miniAppBuilder.tooltip' | 'ai.permission.rag.tooltip' {
  if (mode === 'power_user') return 'ai.permission.powerUser.tooltip'
  if (mode === 'deep_research') return 'ai.permission.deepResearch.tooltip'
  if (mode === 'mini_app_builder') return 'ai.permission.miniAppBuilder.tooltip'
  if (mode === 'rag') return 'ai.permission.rag.tooltip'
  return 'ai.permission.safe.tooltip'
}

export function aiAgentPermissionModeMarker(
  mode: AiAgentPermissionMode,
  locale: AppLocale = 'en',
): string {
  const t = createTranslator(locale)
  const label = aiAgentPermissionModeLabels(mode, locale).short
  return t('ai.permission.changed', { label })
}
