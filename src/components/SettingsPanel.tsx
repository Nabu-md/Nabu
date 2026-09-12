import { Copy, Cube, Microphone, Monitor, Moon, Sparkle, SpeakerHigh, Sun, X } from '@phosphor-icons/react'
import { MiniAppsSettingsSection } from './MiniAppsSettingsSection'
import { BuzzSettingsSection } from './BuzzSettingsSection'
import {
  AI_AGENT_DEFINITIONS,
  createMissingAiAgentsStatus,
  getAiAgentAvailability,
  getAiAgentDefinition,
  resolveDefaultAiAgent,
  type AiAgentId,
  type AiAgentsStatus,
} from '../lib/aiAgents'
import {
  agentTargetId,
  configuredModelTargets,
  normalizeAiModelProviders,
  resolveAiTarget,
  type AiModelProvider,
} from '../lib/aiTargets'
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react'
import type { GitProviderId, Settings } from '../types'
import {
  APP_LOCALES,
  SYSTEM_UI_LANGUAGE,
  createTranslator,
  localeDisplayName,
  resolveEffectiveLocale,
  serializeUiLanguagePreference,
  type AppLocale,
  type UiLanguagePreference,
} from '../lib/i18n'
import {
  applyThemeSelectionToDocument,
  DEFAULT_THEME_MODE,
  readStoredThemeMode,
  type ThemeMode,
  writeStoredThemeMode,
} from '../lib/themeMode'
import { normalizeReleaseChannel, serializeReleaseChannel, type ReleaseChannel } from '../lib/releaseChannel'
import { shouldHideGitignoredFiles } from '../lib/gitignoredVisibility'
import { areGitFeaturesEnabled } from '../lib/gitSettings'
import { areAiFeaturesEnabled } from '../lib/aiFeatures'
import { areAutomaticUpdateChecksEnabled } from '../lib/automaticUpdateChecks'
import { trackAllNotesVisibilityChanged } from '../lib/productAnalytics'
import { AiProviderSettings } from './AiProviderSettings'
import { AiAgentIcon } from './AiAgentIcon'
import { GitSettingsSection } from './GitSettingsSection'
import { PrivacySettingsSection } from './PrivacySettingsSection'
import { SettingsBodyNav } from './SettingsBodyNav'
import {
  SectionHeading,
  SelectControl,
  SettingsGroup,
  SettingsRow,
  SettingsSection,
  SettingsSwitchRow,
} from './SettingsControls'
import { SettingsFooter } from './SettingsFooter'
import { VaultContentSettingsSection } from './VaultContentSettingsSection'
import { WorkspaceSettingsSection } from './WorkspaceSettingsSection'
import {
  resolveAllNotesFileVisibility,
  settingsWithAllNotesFileVisibility,
  type AllNotesFileVisibility,
} from '../utils/allNotesFileVisibility'
import { DEFAULT_NOTE_WIDTH_MODE, normalizeNoteWidthMode } from '../utils/noteWidth'
import { DEFAULT_DATE_DISPLAY_FORMAT, normalizeDateDisplayFormat, type DateDisplayFormat } from '../utils/dateDisplay'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { Slider } from './ui/slider'
import { invoke } from '@tauri-apps/api/core'
import { isTauri, mockInvoke } from '../mock-tauri'
import { Tabs, TabsContent, TabsList, TabsTrigger } from './ui/tabs'
import type { NoteWidthMode } from '../types'
import type { VaultOption } from './status-bar/types'
import { SETTINGS_SECTION_IDS } from './settingsSectionIds'
import { trackSettingsPreferenceChanges, trackTelemetryConsentChange } from './settingsPreferenceTracking'
import { useSettingsPanelAutofocus, useSettingsPanelFocusTrap } from './useSettingsPanelFocus'
import { registerMacosDismissableEscapeSurface } from '../utils/macosDismissableEscapeSurface'

interface SettingsPanelProps {
  open: boolean
  settings: Settings
  aiAgentsStatus?: AiAgentsStatus
  initialSectionId?: string | null
  locale?: AppLocale
  systemLocale?: AppLocale
  onSave: (settings: Settings) => void
  onCopyMcpConfig?: () => void
  vaults?: VaultOption[]
  defaultWorkspacePath?: string | null
  onRemoveVault?: (path: string) => void
  onReorderVaults?: (orderedPaths: string[]) => void
  onSetDefaultWorkspace?: (path: string) => void
  onUpdateWorkspaceIdentity?: (path: string, patch: Partial<VaultOption>) => void
  isGitVault?: boolean
  vaultPath?: string
  explicitOrganizationEnabled?: boolean
  onSaveExplicitOrganization?: (enabled: boolean) => void
  onClose: () => void
}

interface SettingsDraft {
  pullInterval: number
  gitFeaturesEnabled: boolean
  gitProvider: GitProviderId
  gitWslDistro: string | null
  autoGitEnabled: boolean
  autoGitAiCommitMessagesEnabled: boolean
  autoGitIdleThresholdSeconds: number
  autoGitInactiveThresholdSeconds: number
  autoAdvanceInboxAfterOrganize: boolean
  aiFeaturesEnabled: boolean
  defaultAiAgent: AiAgentId
  defaultAiTarget: string
  aiModelProviders: AiModelProvider[]
  releaseChannel: ReleaseChannel
  automaticUpdateChecksEnabled: boolean
  themeMode: ThemeMode
  uiLanguage: UiLanguagePreference
  dateDisplayFormat: DateDisplayFormat
  defaultNoteWidth: NoteWidthMode
  sidebarTypePluralizationEnabled: boolean
  initialH1AutoRename: boolean
  hideGitignoredFiles: boolean
  allNotesFileVisibility: AllNotesFileVisibility
  multiWorkspaceEnabled: boolean
  crashReporting: boolean
  analytics: boolean
  explicitOrganization: boolean
  dictationEnabled: boolean
  dictationPosition: 'bottom-right' | 'bottom-left'
  dictationOpacity: number
  dictationBackend: 'web_speech' | 'fluidvoice'
  fluidvoiceModel: string
  ttsEngine: 'system' | 'kokoro'
  kokoroVoice: string
  kokoroSpeed: number
  ttsHighlightEnabled: boolean
  miniAppsEnabled: boolean
  grammarCheckEnabled: boolean
  ocrEnabled: boolean
  documentConversionEnabled: boolean
  sidebarOpacity: number
  sidebarBlurRadius: number
  editorOpacity: number
  editorBlurRadius: number
  aiPanelOpacity: number
  aiPanelBlurRadius: number
  windowOpacity: number
  windowBlurRadius: number
  editorFontFamily: string
  aiChatFontFamily: string
  sidebarFontFamily: string
  buzzEnabled: boolean
  buzzChannel: string
  miniAppsWebAccessEnabled: boolean
  dockIconVariant: 'variant-1' | 'variant-2' | 'variant-3' | 'variant-4' | 'variant-5' | 'variant-6' | 'variant-7' | 'variant-8' | 'variant-9' | 'variant-10' | null
}

interface SettingsBodyProps {
  t: Translate
  pullInterval: number
  setPullInterval: (value: number) => void
  gitFeaturesEnabled: boolean
  setGitFeaturesEnabled: (value: boolean) => void
  gitProvider: GitProviderId
  setGitProvider: (value: GitProviderId) => void
  gitWslDistro: string | null
  setGitWslDistro: (value: string | null) => void
  isGitVault: boolean
  vaultPath: string
  autoGitEnabled: boolean
  setAutoGitEnabled: (value: boolean) => void
  autoGitAiCommitMessagesEnabled: boolean
  setAutoGitAiCommitMessagesEnabled: (value: boolean) => void
  autoGitIdleThresholdSeconds: number
  setAutoGitIdleThresholdSeconds: (value: number) => void
  autoGitInactiveThresholdSeconds: number
  setAutoGitInactiveThresholdSeconds: (value: number) => void
  autoAdvanceInboxAfterOrganize: boolean
  setAutoAdvanceInboxAfterOrganize: (value: boolean) => void
  aiFeaturesEnabled: boolean
  setAiFeaturesEnabled: (value: boolean) => void
  aiAgentsStatus: AiAgentsStatus
  defaultAiAgent: AiAgentId
  setDefaultAiAgent: (value: AiAgentId) => void
  defaultAiTarget: string
  setDefaultAiTarget: (value: string) => void
  aiModelProviders: AiModelProvider[]
  setAiModelProviders: (value: AiModelProvider[]) => void
  onCopyMcpConfig?: () => void
  releaseChannel: ReleaseChannel
  setReleaseChannel: (value: ReleaseChannel) => void
  automaticUpdateChecksEnabled: boolean
  setAutomaticUpdateChecksEnabled: (value: boolean) => void
  themeMode: ThemeMode
  setThemeMode: (value: ThemeMode) => void
  uiLanguage: UiLanguagePreference
  setUiLanguage: (value: UiLanguagePreference) => void
  dateDisplayFormat: DateDisplayFormat
  setDateDisplayFormat: (value: DateDisplayFormat) => void
  defaultNoteWidth: NoteWidthMode
  setDefaultNoteWidth: (value: NoteWidthMode) => void
  sidebarTypePluralizationEnabled: boolean
  setSidebarTypePluralizationEnabled: (value: boolean) => void
  locale: AppLocale
  systemLocale: AppLocale
  initialH1AutoRename: boolean
  setInitialH1AutoRename: (value: boolean) => void
  hideGitignoredFiles: boolean
  setHideGitignoredFiles: (value: boolean) => void
  allNotesFileVisibility: AllNotesFileVisibility
  setAllNotesFileVisibility: (value: AllNotesFileVisibility) => void
  multiWorkspaceEnabled: boolean
  setMultiWorkspaceEnabled: (value: boolean) => void
  vaults: VaultOption[]
  defaultWorkspacePath?: string | null
  onRemoveVault?: (path: string) => void
  onReorderVaults?: (orderedPaths: string[]) => void
  onSetDefaultWorkspace?: (path: string) => void
  onUpdateWorkspaceIdentity?: (path: string, patch: Partial<VaultOption>) => void
  explicitOrganization: boolean
  setExplicitOrganization: (value: boolean) => void
  crashReporting: boolean
  setCrashReporting: (value: boolean) => void
  analytics: boolean
  setAnalytics: (value: boolean) => void
  dictationEnabled: boolean
  setDictationEnabled: (value: boolean) => void
  dictationPosition: 'bottom-right' | 'bottom-left'
  setDictationPosition: (value: 'bottom-right' | 'bottom-left') => void
  dictationOpacity: number
  setDictationOpacity: (value: number) => void
  dictationBackend: 'web_speech' | 'fluidvoice'
  setDictationBackend: (value: 'web_speech' | 'fluidvoice') => void
  fluidvoiceModel: string
  setFluidvoiceModel: (value: string) => void
  ttsEngine: 'system' | 'kokoro'
  setTtsEngine: (value: 'system' | 'kokoro') => void
  kokoroVoice: string
  setKokoroVoice: (value: string) => void
  kokoroSpeed: number
  setKokoroSpeed: (value: number) => void
  ttsHighlightEnabled: boolean
  setTtsHighlightEnabled: (value: boolean) => void
  miniAppsEnabled: boolean
  setMiniAppsEnabled: (value: boolean) => void
  grammarCheckEnabled: boolean
  setGrammarCheckEnabled: (value: boolean) => void
  ocrEnabled: boolean
  setOcrEnabled: (value: boolean) => void
  documentConversionEnabled: boolean
  setDocumentConversionEnabled: (value: boolean) => void
  sidebarOpacity: number
  setSidebarOpacity: (value: number) => void
  sidebarBlurRadius: number
  setSidebarBlurRadius: (value: number) => void
  editorOpacity: number
  setEditorOpacity: (value: number) => void
  editorBlurRadius: number
  setEditorBlurRadius: (value: number) => void
  aiPanelOpacity: number
  setAiPanelOpacity: (value: number) => void
  aiPanelBlurRadius: number
  setAiPanelBlurRadius: (value: number) => void
  windowOpacity: number
  setWindowOpacity: (value: number) => void
  windowBlurRadius: number
  setWindowBlurRadius: (value: number) => void
  editorFontFamily: string
  setEditorFontFamily: (value: string) => void
  aiChatFontFamily: string
  setAiChatFontFamily: (value: string) => void
  sidebarFontFamily: string
  setSidebarFontFamily: (value: string) => void
  buzzEnabled: boolean
  setBuzzEnabled: (value: boolean) => void
  buzzChannel: string
  setBuzzChannel: (value: string) => void
  miniAppsWebAccessEnabled: boolean
  setMiniAppsWebAccessEnabled: (value: boolean) => void
  dockIconVariant: 'variant-1' | 'variant-2' | 'variant-3' | 'variant-4' | 'variant-5' | 'variant-6' | 'variant-7' | 'variant-8' | 'variant-9' | 'variant-10' | null
  setDockIconVariant: (value: 'variant-1' | 'variant-2' | 'variant-3' | 'variant-4' | 'variant-5' | 'variant-6' | 'variant-7' | 'variant-8' | 'variant-9' | 'variant-10' | null) => void
}

const PULL_INTERVAL_OPTIONS = [1, 2, 5, 10, 15, 30] as const
const DEFAULT_AUTOGIT_IDLE_THRESHOLD_SECONDS = 90
const DEFAULT_AUTOGIT_INACTIVE_THRESHOLD_SECONDS = 30
type Translate = ReturnType<typeof createTranslator>

function isSaveShortcut(event: { ctrlKey: boolean; key: string; metaKey: boolean }): boolean {
  return event.key === 'Enter' && (event.metaKey || event.ctrlKey)
}

function createSettingsDraft(settings: Settings, explicitOrganizationEnabled: boolean): SettingsDraft {
  return {
    pullInterval: settings.auto_pull_interval_minutes ?? 5,
    gitFeaturesEnabled: areGitFeaturesEnabled(settings),
    gitProvider: normalizeSettingsGitProvider(settings.git_provider),
    gitWslDistro: settings.git_wsl_distro?.trim() || null,
    autoGitEnabled: settings.autogit_enabled ?? false,
    autoGitAiCommitMessagesEnabled: settings.autogit_use_ai_commit_messages ?? false,
    autoGitIdleThresholdSeconds: sanitizePositiveInteger(
      settings.autogit_idle_threshold_seconds,
      DEFAULT_AUTOGIT_IDLE_THRESHOLD_SECONDS,
    ),
    autoGitInactiveThresholdSeconds: sanitizePositiveInteger(
      settings.autogit_inactive_threshold_seconds,
      DEFAULT_AUTOGIT_INACTIVE_THRESHOLD_SECONDS,
    ),
    autoAdvanceInboxAfterOrganize: settings.auto_advance_inbox_after_organize ?? false,
    aiFeaturesEnabled: areAiFeaturesEnabled(settings),
    defaultAiAgent: resolveDefaultAiAgent(settings.default_ai_agent),
    defaultAiTarget: resolveAiTarget(settings).id,
    aiModelProviders: normalizeAiModelProviders(settings.ai_model_providers),
    releaseChannel: normalizeReleaseChannel(settings.release_channel),
    automaticUpdateChecksEnabled: areAutomaticUpdateChecksEnabled(settings),
    themeMode: resolveSettingsDraftThemeMode(settings.theme_mode),
    uiLanguage: settings.ui_language ?? SYSTEM_UI_LANGUAGE,
    dateDisplayFormat: normalizeDateDisplayFormat(settings.date_display_format) ?? DEFAULT_DATE_DISPLAY_FORMAT,
    defaultNoteWidth: normalizeNoteWidthMode(settings.note_width_mode) ?? DEFAULT_NOTE_WIDTH_MODE,
    sidebarTypePluralizationEnabled: settings.sidebar_type_pluralization_enabled ?? true,
    initialH1AutoRename: settings.initial_h1_auto_rename_enabled ?? true,
    hideGitignoredFiles: shouldHideGitignoredFiles(settings),
    allNotesFileVisibility: resolveAllNotesFileVisibility(settings),
    multiWorkspaceEnabled: settings.multi_workspace_enabled === true,
    crashReporting: settings.crash_reporting_enabled ?? false,
    analytics: settings.analytics_enabled ?? false,
    explicitOrganization: explicitOrganizationEnabled,
    dictationEnabled: settings.dictation_enabled ?? false,
    dictationPosition: settings.dictation_position ?? 'bottom-right',
    dictationOpacity: settings.dictation_opacity ?? 0.85,
    dictationBackend: settings.dictation_backend ?? 'web_speech',
    fluidvoiceModel: settings.fluidvoice_model ?? 'parakeet',
    ttsEngine: settings.tts_engine === 'kokoro' ? 'kokoro' : 'system',
    kokoroVoice: settings.kokoro_voice ?? 'af_sky',
    kokoroSpeed: clampKokoroSpeed(settings.kokoro_speed),
    ttsHighlightEnabled: settings.tts_highlight_enabled ?? true,
    miniAppsEnabled: settings.mini_apps_enabled ?? true,
    grammarCheckEnabled: settings.grammar_check_enabled ?? true,
    ocrEnabled: settings.ocr_enabled ?? true,
    documentConversionEnabled: settings.document_conversion_enabled ?? true,
    sidebarOpacity: settings.sidebar_opacity ?? 1,
    sidebarBlurRadius: settings.sidebar_blur_radius ?? 0,
    editorOpacity: settings.editor_opacity ?? 1,
    editorBlurRadius: settings.editor_blur_radius ?? 0,
    aiPanelOpacity: settings.ai_panel_opacity ?? 1,
    aiPanelBlurRadius: settings.ai_panel_blur_radius ?? 0,
    windowOpacity: settings.window_opacity ?? 1,
    windowBlurRadius: settings.window_blur_radius ?? 0,
    editorFontFamily: settings.editor_font_family ?? '',
    aiChatFontFamily: settings.ai_chat_font_family ?? '',
    sidebarFontFamily: settings.sidebar_font_family ?? '',
    buzzEnabled: settings.buzz_enabled ?? false,
    buzzChannel: settings.buzz_default_channel ?? '',
    miniAppsWebAccessEnabled: settings.mini_apps_web_access_enabled ?? false,
    dockIconVariant: settings.dock_icon_variant ?? 'variant-1',
  }
}

function clampKokoroSpeed(value: Settings['kokoro_speed']): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return 1
  return Math.min(2, Math.max(0.5, value))
}

function resolveSettingsDraftThemeMode(themeMode: Settings['theme_mode']): ThemeMode {
  if (themeMode) return themeMode
  if (typeof window === 'undefined') return DEFAULT_THEME_MODE
  return readStoredThemeMode(window.localStorage) ?? DEFAULT_THEME_MODE
}

function resolveTelemetryConsent(settings: Settings, draft: SettingsDraft): boolean | null {
  if (draft.crashReporting || draft.analytics) return true
  return settings.telemetry_consent === null ? null : false
}

function resolveAnonymousId(settings: Settings, draft: SettingsDraft): string | null {
  if (draft.crashReporting || draft.analytics) {
    return settings.anonymous_id ?? crypto.randomUUID()
  }

  return settings.anonymous_id
}

function buildSettingsFromDraft(settings: Settings, draft: SettingsDraft): Settings {
  const nextSettings: Settings = {
    auto_pull_interval_minutes: draft.pullInterval,
    git_enabled: draft.gitFeaturesEnabled,
    git_provider: draft.gitProvider === 'native' ? null : draft.gitProvider,
    git_wsl_distro: draft.gitProvider === 'wsl' ? draft.gitWslDistro : null,
    autogit_enabled: draft.autoGitEnabled,
    autogit_use_ai_commit_messages: draft.autoGitAiCommitMessagesEnabled,
    autogit_idle_threshold_seconds: draft.autoGitIdleThresholdSeconds,
    autogit_inactive_threshold_seconds: draft.autoGitInactiveThresholdSeconds,
    auto_advance_inbox_after_organize: draft.autoAdvanceInboxAfterOrganize,
    telemetry_consent: resolveTelemetryConsent(settings, draft),
    crash_reporting_enabled: draft.crashReporting,
    analytics_enabled: draft.analytics,
    anonymous_id: resolveAnonymousId(settings, draft),
    release_channel: serializeReleaseChannel(draft.releaseChannel),
    automatic_update_checks_enabled: draft.automaticUpdateChecksEnabled ? null : false,
    theme_mode: draft.themeMode,
    ui_language: serializeUiLanguagePreference(draft.uiLanguage),
    date_display_format: draft.dateDisplayFormat,
    note_width_mode: draft.defaultNoteWidth,
    sidebar_type_pluralization_enabled: draft.sidebarTypePluralizationEnabled,
    initial_h1_auto_rename_enabled: draft.initialH1AutoRename,
    ai_features_enabled: draft.aiFeaturesEnabled,
    default_ai_agent: draft.defaultAiAgent,
    default_ai_target: draft.defaultAiTarget,
    ai_model_providers: draft.aiModelProviders.length > 0 ? draft.aiModelProviders : null,
    hide_gitignored_files: draft.hideGitignoredFiles,
    multi_workspace_enabled: draft.multiWorkspaceEnabled,
    dictation_enabled: draft.dictationEnabled,
    dictation_position: draft.dictationPosition,
    dictation_opacity: draft.dictationOpacity,
    dictation_backend: draft.dictationBackend,
    fluidvoice_model: draft.fluidvoiceModel,
    tts_engine: (draft.ttsEngine === 'kokoro' ? 'kokoro' : 'system') as Settings['tts_engine'],
    kokoro_voice: draft.kokoroVoice,
    kokoro_speed: draft.kokoroSpeed,
    tts_highlight_enabled: draft.ttsHighlightEnabled,
    mini_apps_enabled: draft.miniAppsEnabled,
    grammar_check_enabled: draft.grammarCheckEnabled,
    ocr_enabled: draft.ocrEnabled,
    document_conversion_enabled: draft.documentConversionEnabled,
    sidebar_opacity: draft.sidebarOpacity,
    sidebar_blur_radius: draft.sidebarBlurRadius,
    editor_opacity: draft.editorOpacity,
    editor_blur_radius: draft.editorBlurRadius,
    ai_panel_opacity: draft.aiPanelOpacity,
    ai_panel_blur_radius: draft.aiPanelBlurRadius,
    window_opacity: draft.windowOpacity,
    window_blur_radius: draft.windowBlurRadius,
    editor_font_family: draft.editorFontFamily.trim() || null,
    ai_chat_font_family: draft.aiChatFontFamily.trim() || null,
    sidebar_font_family: draft.sidebarFontFamily.trim() || null,
    buzz_enabled: draft.buzzEnabled,
    buzz_default_channel: draft.buzzChannel.trim() || null,
    mini_apps_web_access_enabled: draft.miniAppsWebAccessEnabled,
    dock_icon_variant: draft.dockIconVariant === 'variant-1' ? null : draft.dockIconVariant,
  }
  return settingsWithAllNotesFileVisibility(nextSettings, draft.allNotesFileVisibility)
}

function normalizeSettingsGitProvider(value: Settings['git_provider']): GitProviderId {
  return value === 'wsl' ? 'wsl' : 'native'
}

function sanitizePositiveInteger(value: number | null | undefined, fallback: number): number {
  if (value === null || value === undefined || !Number.isFinite(value) || value < 1) return fallback
  return Math.round(value)
}

function applyThemeModeSelection(value: ThemeMode): void {
  const matchMedia = typeof window !== 'undefined' ? window.matchMedia?.bind(window) : undefined
  if (typeof document !== 'undefined') applyThemeSelectionToDocument(document, value, matchMedia)
  if (typeof window !== 'undefined') writeStoredThemeMode(window.localStorage, value)
}

export function SettingsPanel(options: SettingsPanelProps) {
  const { open, settings, aiAgentsStatus = createMissingAiAgentsStatus(), initialSectionId = null, locale = 'en', systemLocale = locale, onSave, onCopyMcpConfig, vaults = [], defaultWorkspacePath = null, onRemoveVault, onReorderVaults, onSetDefaultWorkspace, onUpdateWorkspaceIdentity, isGitVault = true, vaultPath = '', explicitOrganizationEnabled = true, onSaveExplicitOrganization, onClose } = options
  if (!open) return null
  const initialDraft = createSettingsDraft(settings, explicitOrganizationEnabled)

  return (
    <SettingsPanelInner
      key={JSON.stringify(initialDraft)}
      settings={settings}
      aiAgentsStatus={aiAgentsStatus}
      initialDraft={initialDraft}
      initialSectionId={initialSectionId}
      locale={locale}
      systemLocale={systemLocale}
      onSave={onSave}
      onCopyMcpConfig={onCopyMcpConfig}
      vaults={vaults}
      defaultWorkspacePath={defaultWorkspacePath}
      {...{
        onRemoveVault,
        onReorderVaults,
        onSetDefaultWorkspace,
        onUpdateWorkspaceIdentity,
      }}
      isGitVault={isGitVault}
      vaultPath={vaultPath}
      explicitOrganizationEnabled={explicitOrganizationEnabled}
      onSaveExplicitOrganization={onSaveExplicitOrganization}
      onClose={onClose}
    />
  )
}

type SettingsPanelInnerProps = Omit<
  SettingsPanelProps,
  'open' | 'explicitOrganizationEnabled' | 'aiAgentsStatus' | 'isGitVault' | 'vaultPath'
> & {
  aiAgentsStatus: AiAgentsStatus
  initialDraft: SettingsDraft
  initialSectionId: string | null
  locale: AppLocale
  systemLocale: AppLocale
  isGitVault: boolean
  vaultPath: string
  explicitOrganizationEnabled: boolean
}

function useSettingsDraftActions(options: Pick<SettingsPanelInnerProps, 'initialDraft' | 'onClose' | 'onSave' | 'onSaveExplicitOrganization' | 'settings'>) {
  const { initialDraft, onClose, onSave, onSaveExplicitOrganization, settings } = options
  const [draft, setDraft] = useState(initialDraft)
  const updateDraft = useCallback(<Key extends keyof SettingsDraft>(key: Key, value: SettingsDraft[Key]) => {
    setDraft((current) => ({ ...current, [key]: value }))
  }, [])
  const handleGitignoredVisibilityChange = useCallback((value: boolean) => {
    updateDraft('hideGitignoredFiles', value)
    onSave({ ...settings, hide_gitignored_files: value })
  }, [onSave, settings, updateDraft])
  const handleAllNotesFileVisibilityChange = useCallback((value: AllNotesFileVisibility) => {
    trackAllNotesVisibilityChanged(draft.allNotesFileVisibility, value)
    updateDraft('allNotesFileVisibility', value)
    onSave(settingsWithAllNotesFileVisibility(settings, value))
  }, [draft.allNotesFileVisibility, onSave, settings, updateDraft])
  const handleThemeModeChange = useCallback((value: ThemeMode) => {
    updateDraft('themeMode', value)
    applyThemeModeSelection(value)
    onSave({ ...settings, theme_mode: value })
  }, [onSave, settings, updateDraft])
  const handleDockIconVariantChange = useCallback((value: 'variant-1' | 'variant-2' | 'variant-3' | 'variant-4' | 'variant-5' | 'variant-6' | 'variant-7' | 'variant-8' | 'variant-9' | 'variant-10' | null) => {
    updateDraft('dockIconVariant', value)
  }, [updateDraft])
  const handleSave = useCallback(() => {
    trackTelemetryConsentChange(settings.analytics_enabled === true, draft.analytics)
    trackSettingsPreferenceChanges(settings, draft)
    onSave(buildSettingsFromDraft(settings, draft))
    onSaveExplicitOrganization?.(draft.explicitOrganization)
    onClose()
  }, [draft, onClose, onSave, onSaveExplicitOrganization, settings])
  return { draft, updateDraft, handleGitignoredVisibilityChange, handleAllNotesFileVisibilityChange, handleThemeModeChange, handleDockIconVariantChange, handleSave }
}

function useSettingsPanelInteractions(options: {
  backdropRef: React.RefObject<HTMLDivElement | null>
  handleSave: () => void
  initialSectionId?: string | null
  onClose: () => void
  panelRef: React.RefObject<HTMLDivElement | null>
}): void {
  const { backdropRef, handleSave, initialSectionId, onClose, panelRef } = options
  useEffect(registerMacosDismissableEscapeSurface, [])
  useSettingsPanelAutofocus(panelRef)
  useSettingsPanelFocusTrap(panelRef)
  useEffect(() => {
    if (!initialSectionId) return
    const timer = window.setTimeout(() => document.getElementById(initialSectionId)?.scrollIntoView({ block: 'start' }), 50)
    return () => window.clearTimeout(timer)
  }, [initialSectionId])
  useEffect(() => {
    const handleKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.stopPropagation()
        onClose()
      } else if (isSaveShortcut(event)) {
        event.preventDefault()
        handleSave()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [handleSave, onClose])
  useEffect(() => {
    const backdrop = backdropRef.current
    if (!backdrop) return
    const handleBackdropClick = (event: MouseEvent) => {
      if (event.target === backdrop) onClose()
    }
    backdrop.addEventListener('click', handleBackdropClick)
    return () => backdrop.removeEventListener('click', handleBackdropClick)
  }, [backdropRef, onClose])
}

function SettingsPanelInner(options: SettingsPanelInnerProps) {
  const { settings, aiAgentsStatus, initialDraft, initialSectionId, systemLocale, onSave, onCopyMcpConfig, vaults, defaultWorkspacePath, onRemoveVault, onReorderVaults, onSetDefaultWorkspace, onUpdateWorkspaceIdentity, isGitVault, vaultPath, onSaveExplicitOrganization, onClose } = options
  const backdropRef = useRef<HTMLDivElement>(null)
  const panelRef = useRef<HTMLDivElement>(null)
  const { draft, updateDraft, handleGitignoredVisibilityChange, handleAllNotesFileVisibilityChange, handleThemeModeChange, handleDockIconVariantChange, handleSave } = useSettingsDraftActions({ initialDraft, onClose, onSave, onSaveExplicitOrganization, settings })
  const draftLocale = resolveEffectiveLocale(draft.uiLanguage, [systemLocale])
  const t = createTranslator(draftLocale)
  useSettingsPanelInteractions({ backdropRef, handleSave, initialSectionId, onClose, panelRef })

  return (
    <div
      ref={backdropRef}
      className="fixed inset-0 z-[1300] flex items-center justify-center"
      style={{ background: 'var(--shadow-overlay)' }}
      data-testid="settings-panel"
    >
      <SettingsBackdropCloseButton onClose={onClose} t={t} />
      <div
        ref={panelRef}
        className="relative rounded-lg border border-border bg-background shadow-[0_18px_55px_var(--shadow-dialog)]"
        style={{
          width: 'min(960px, calc(100vw - 48px))',
          maxHeight: '86vh',
          display: 'flex',
          flexDirection: 'column',
        }}
      >
        <SettingsHeader onClose={onClose} t={t} />
        <SettingsBodyFromDraft
          t={t}
          draft={draft}
          locale={draftLocale}
          systemLocale={systemLocale}
          updateDraft={updateDraft}
          isGitVault={isGitVault}
          vaultPath={vaultPath}
          aiAgentsStatus={aiAgentsStatus}
          onCopyMcpConfig={onCopyMcpConfig}
          vaults={vaults ?? []}
          defaultWorkspacePath={defaultWorkspacePath}
          {...{
            onRemoveVault,
            onReorderVaults,
            onSetDefaultWorkspace,
            onUpdateWorkspaceIdentity,
          }}
          setThemeMode={handleThemeModeChange}
          setHideGitignoredFiles={handleGitignoredVisibilityChange}
          setAllNotesFileVisibility={handleAllNotesFileVisibilityChange}
          dockIconVariant={draft.dockIconVariant}
          setDockIconVariant={handleDockIconVariantChange}
        />
        <SettingsFooter onClose={onClose} onSave={handleSave} t={t} />
      </div>
    </div>
  )
}

function SettingsBackdropCloseButton({ onClose, t }: { onClose: () => void; t: Translate }) {
  return (
    <button
      type="button"
      aria-label={t('settings.close')}
      className="absolute inset-0 cursor-default border-0 bg-transparent p-0"
      onClick={onClose}
    />
  )
}

function SettingsHeader({ onClose, t }: { onClose: () => void; t: Translate }) {
  return (
    <div
      className="flex items-center justify-between shrink-0"
      style={{
        height: 56,
        padding: '0 24px',
        borderBottom: '1px solid var(--border)',
      }}
    >
      <span style={{ fontSize: 16, fontWeight: 600, color: 'var(--foreground)' }}>{t('settings.title')}</span>
      <Button
        variant="ghost"
        size="icon-sm"
        onClick={onClose}
        title={t('settings.close')}
        aria-label={t('settings.close')}
      >
        <X size={16} />
      </Button>
    </div>
  )
}

interface SettingsBodyFromDraftProps {
  t: Translate
  draft: SettingsDraft
  locale: AppLocale
  systemLocale: AppLocale
  updateDraft: <Key extends keyof SettingsDraft>(key: Key, value: SettingsDraft[Key]) => void
  isGitVault: boolean
  vaultPath: string
  aiAgentsStatus: AiAgentsStatus
  onCopyMcpConfig?: () => void
  vaults: VaultOption[]
  defaultWorkspacePath?: string | null
  onRemoveVault?: (path: string) => void
  onReorderVaults?: (orderedPaths: string[]) => void
  onSetDefaultWorkspace?: (path: string) => void
  onUpdateWorkspaceIdentity?: (path: string, patch: Partial<VaultOption>) => void
  setThemeMode: (value: ThemeMode) => void
  setHideGitignoredFiles: (value: boolean) => void
  setAllNotesFileVisibility: (value: AllNotesFileVisibility) => void
  dockIconVariant: 'variant-1' | 'variant-2' | 'variant-3' | 'variant-4' | 'variant-5' | 'variant-6' | 'variant-7' | 'variant-8' | 'variant-9' | 'variant-10' | null
  setDockIconVariant: (value: 'variant-1' | 'variant-2' | 'variant-3' | 'variant-4' | 'variant-5' | 'variant-6' | 'variant-7' | 'variant-8' | 'variant-9' | 'variant-10' | null) => void
}

function SettingsBodyFromDraft(options: SettingsBodyFromDraftProps) {
  const { t, draft, locale, systemLocale, updateDraft, isGitVault, vaultPath, aiAgentsStatus, onCopyMcpConfig, vaults, defaultWorkspacePath, onRemoveVault, onReorderVaults, onSetDefaultWorkspace, onUpdateWorkspaceIdentity, setThemeMode, setHideGitignoredFiles, setAllNotesFileVisibility } = options
  return (
    <SettingsBody
      t={t}
      locale={locale}
      systemLocale={systemLocale}
      pullInterval={draft.pullInterval}
      setPullInterval={(value) => updateDraft('pullInterval', value)}
      gitFeaturesEnabled={draft.gitFeaturesEnabled}
      setGitFeaturesEnabled={(value) => updateDraft('gitFeaturesEnabled', value)}
      gitProvider={draft.gitProvider}
      setGitProvider={(value) => updateDraft('gitProvider', value)}
      gitWslDistro={draft.gitWslDistro}
      setGitWslDistro={(value) => updateDraft('gitWslDistro', value)}
      isGitVault={isGitVault}
      vaultPath={vaultPath}
      autoGitEnabled={draft.autoGitEnabled}
      setAutoGitEnabled={(value) => updateDraft('autoGitEnabled', value)}
      autoGitAiCommitMessagesEnabled={draft.autoGitAiCommitMessagesEnabled}
      setAutoGitAiCommitMessagesEnabled={(value) => updateDraft('autoGitAiCommitMessagesEnabled', value)}
      autoGitIdleThresholdSeconds={draft.autoGitIdleThresholdSeconds}
      setAutoGitIdleThresholdSeconds={(value) => updateDraft('autoGitIdleThresholdSeconds', value)}
      autoGitInactiveThresholdSeconds={draft.autoGitInactiveThresholdSeconds}
      setAutoGitInactiveThresholdSeconds={(value) => updateDraft('autoGitInactiveThresholdSeconds', value)}
      autoAdvanceInboxAfterOrganize={draft.autoAdvanceInboxAfterOrganize}
      setAutoAdvanceInboxAfterOrganize={(value) => updateDraft('autoAdvanceInboxAfterOrganize', value)}
      aiFeaturesEnabled={draft.aiFeaturesEnabled}
      setAiFeaturesEnabled={(value) => updateDraft('aiFeaturesEnabled', value)}
      aiAgentsStatus={aiAgentsStatus}
      defaultAiAgent={draft.defaultAiAgent}
      setDefaultAiAgent={(value) => updateDraft('defaultAiAgent', value)}
      defaultAiTarget={draft.defaultAiTarget}
      setDefaultAiTarget={(value) => updateDraft('defaultAiTarget', value)}
      aiModelProviders={draft.aiModelProviders}
      setAiModelProviders={(value) => updateDraft('aiModelProviders', value)}
      onCopyMcpConfig={onCopyMcpConfig}
      releaseChannel={draft.releaseChannel}
      setReleaseChannel={(value) => updateDraft('releaseChannel', value)}
      automaticUpdateChecksEnabled={draft.automaticUpdateChecksEnabled}
      setAutomaticUpdateChecksEnabled={(value) => updateDraft('automaticUpdateChecksEnabled', value)}
      themeMode={draft.themeMode}
      setThemeMode={setThemeMode}
      uiLanguage={draft.uiLanguage}
      setUiLanguage={(value) => updateDraft('uiLanguage', value)}
      dateDisplayFormat={draft.dateDisplayFormat}
      setDateDisplayFormat={(value) => updateDraft('dateDisplayFormat', value)}
      defaultNoteWidth={draft.defaultNoteWidth}
      setDefaultNoteWidth={(value) => updateDraft('defaultNoteWidth', value)}
      sidebarTypePluralizationEnabled={draft.sidebarTypePluralizationEnabled}
      setSidebarTypePluralizationEnabled={(value) => updateDraft('sidebarTypePluralizationEnabled', value)}
      initialH1AutoRename={draft.initialH1AutoRename}
      setInitialH1AutoRename={(value) => updateDraft('initialH1AutoRename', value)}
      hideGitignoredFiles={draft.hideGitignoredFiles}
      setHideGitignoredFiles={setHideGitignoredFiles}
      allNotesFileVisibility={draft.allNotesFileVisibility}
      setAllNotesFileVisibility={setAllNotesFileVisibility}
      multiWorkspaceEnabled={draft.multiWorkspaceEnabled}
      setMultiWorkspaceEnabled={(value) => updateDraft('multiWorkspaceEnabled', value)}
      vaults={vaults}
      defaultWorkspacePath={defaultWorkspacePath}
      {...{
        onRemoveVault,
        onReorderVaults,
        onSetDefaultWorkspace,
        onUpdateWorkspaceIdentity,
      }}
      explicitOrganization={draft.explicitOrganization}
      setExplicitOrganization={(value) => updateDraft('explicitOrganization', value)}
      crashReporting={draft.crashReporting}
      setCrashReporting={(value) => updateDraft('crashReporting', value)}
      analytics={draft.analytics}
      setAnalytics={(value) => updateDraft('analytics', value)}
      dictationEnabled={draft.dictationEnabled}
      setDictationEnabled={(value) => updateDraft('dictationEnabled', value)}
      dictationPosition={draft.dictationPosition}
      setDictationPosition={(value) => updateDraft('dictationPosition', value)}
      dictationOpacity={draft.dictationOpacity}
      setDictationOpacity={(value) => updateDraft('dictationOpacity', value)}
      dictationBackend={draft.dictationBackend}
      setDictationBackend={(value) => updateDraft('dictationBackend', value)}
      fluidvoiceModel={draft.fluidvoiceModel}
      setFluidvoiceModel={(value) => updateDraft('fluidvoiceModel', value)}
      ttsEngine={draft.ttsEngine}
      setTtsEngine={(value) => updateDraft('ttsEngine', value)}
      kokoroVoice={draft.kokoroVoice}
      setKokoroVoice={(value) => updateDraft('kokoroVoice', value)}
      kokoroSpeed={draft.kokoroSpeed}
      setKokoroSpeed={(value) => updateDraft('kokoroSpeed', value)}
      ttsHighlightEnabled={draft.ttsHighlightEnabled}
      setTtsHighlightEnabled={(value) => updateDraft('ttsHighlightEnabled', value)}
      miniAppsEnabled={draft.miniAppsEnabled}
      setMiniAppsEnabled={(value) => updateDraft('miniAppsEnabled', value)}
      grammarCheckEnabled={draft.grammarCheckEnabled}
      setGrammarCheckEnabled={(value) => updateDraft('grammarCheckEnabled', value)}
      ocrEnabled={draft.ocrEnabled}
      setOcrEnabled={(value) => updateDraft('ocrEnabled', value)}
      documentConversionEnabled={draft.documentConversionEnabled}
      setDocumentConversionEnabled={(value) => updateDraft('documentConversionEnabled', value)}
      sidebarOpacity={draft.sidebarOpacity}
      setSidebarOpacity={(value) => updateDraft('sidebarOpacity', value)}
      sidebarBlurRadius={draft.sidebarBlurRadius}
      setSidebarBlurRadius={(value) => updateDraft('sidebarBlurRadius', value)}
      editorOpacity={draft.editorOpacity}
      setEditorOpacity={(value) => updateDraft('editorOpacity', value)}
      editorBlurRadius={draft.editorBlurRadius}
      setEditorBlurRadius={(value) => updateDraft('editorBlurRadius', value)}
      aiPanelOpacity={draft.aiPanelOpacity}
      setAiPanelOpacity={(value) => updateDraft('aiPanelOpacity', value)}
      aiPanelBlurRadius={draft.aiPanelBlurRadius}
      setAiPanelBlurRadius={(value) => updateDraft('aiPanelBlurRadius', value)}
      windowOpacity={draft.windowOpacity}
      setWindowOpacity={(value) => updateDraft('windowOpacity', value)}
      windowBlurRadius={draft.windowBlurRadius}
      setWindowBlurRadius={(value) => updateDraft('windowBlurRadius', value)}
      editorFontFamily={draft.editorFontFamily}
      setEditorFontFamily={(value) => updateDraft('editorFontFamily', value)}
      aiChatFontFamily={draft.aiChatFontFamily}
      setAiChatFontFamily={(value) => updateDraft('aiChatFontFamily', value)}
      sidebarFontFamily={draft.sidebarFontFamily}
      setSidebarFontFamily={(value) => updateDraft('sidebarFontFamily', value)}
      buzzEnabled={draft.buzzEnabled}
      setBuzzEnabled={(value) => updateDraft('buzzEnabled', value)}
      buzzChannel={draft.buzzChannel}
      setBuzzChannel={(value) => updateDraft('buzzChannel', value)}
      miniAppsWebAccessEnabled={draft.miniAppsWebAccessEnabled}
      setMiniAppsWebAccessEnabled={(value) => updateDraft('miniAppsWebAccessEnabled', value)}
      dockIconVariant={draft.dockIconVariant}
      setDockIconVariant={(value) => updateDraft('dockIconVariant', value)}
    />
  )
}

function SettingsBody(props: SettingsBodyProps) {
  return (
    <div className="flex min-h-0 flex-1 overflow-hidden">
      <SettingsBodyNav t={props.t} />
      <div className="min-w-0 flex-1 overflow-auto px-6 py-4">
        <SettingsSyncAndAppearanceSections {...props} />
        <SettingsContentSections {...props} />
        <SettingsAgentWorkflowSections {...props} />
      </div>
    </div>
  )
}

function SettingsSyncAndAppearanceSections(options: SettingsBodyProps) {
  const { t, locale, systemLocale, pullInterval, setPullInterval, gitFeaturesEnabled, setGitFeaturesEnabled, gitProvider, setGitProvider, gitWslDistro, setGitWslDistro, isGitVault, vaultPath, autoGitEnabled, setAutoGitEnabled, autoGitAiCommitMessagesEnabled, setAutoGitAiCommitMessagesEnabled, autoGitIdleThresholdSeconds, setAutoGitIdleThresholdSeconds, autoGitInactiveThresholdSeconds, setAutoGitInactiveThresholdSeconds, releaseChannel, setReleaseChannel, automaticUpdateChecksEnabled, setAutomaticUpdateChecksEnabled, multiWorkspaceEnabled, setMultiWorkspaceEnabled, vaults, defaultWorkspacePath, onRemoveVault, onReorderVaults, onSetDefaultWorkspace, onUpdateWorkspaceIdentity, themeMode, setThemeMode, uiLanguage, setUiLanguage, buzzEnabled, setBuzzEnabled, buzzChannel, setBuzzChannel, miniAppsEnabled, setMiniAppsEnabled, miniAppsWebAccessEnabled, setMiniAppsWebAccessEnabled, dockIconVariant, setDockIconVariant } = options
  return (
    <>
      <SettingsSection id={SETTINGS_SECTION_IDS.sync} showDivider={false}>
        <SyncAndUpdatesSection
          t={t}
          pullInterval={pullInterval}
          setPullInterval={setPullInterval}
          releaseChannel={releaseChannel}
          setReleaseChannel={setReleaseChannel}
          automaticUpdateChecksEnabled={automaticUpdateChecksEnabled}
          setAutomaticUpdateChecksEnabled={setAutomaticUpdateChecksEnabled}
        />
      </SettingsSection>
      <SettingsSection id={SETTINGS_SECTION_IDS.workspaces}>
        <SectionHeading icon={<Cube size={16} aria-hidden="true" />} title={t('settings.workspaces.title')} />
        <WorkspaceSettingsSection
          defaultWorkspacePath={defaultWorkspacePath}
          enabled={multiWorkspaceEnabled}
          locale={locale}
          onEnabledChange={setMultiWorkspaceEnabled}
          {...{
            onRemoveVault,
            onReorderVaults,
            onSetDefaultWorkspace,
            onUpdateWorkspaceIdentity,
          }}
          vaults={vaults}
        />
      </SettingsSection>
      <SettingsSection id={SETTINGS_SECTION_IDS.autogit}>
        <GitSettingsSection
          t={t}
          gitFeaturesEnabled={gitFeaturesEnabled}
          setGitFeaturesEnabled={setGitFeaturesEnabled}
          gitProvider={gitProvider}
          setGitProvider={setGitProvider}
          gitWslDistro={gitWslDistro}
          setGitWslDistro={setGitWslDistro}
          isGitVault={isGitVault}
          vaultPath={vaultPath}
          autoGitEnabled={autoGitEnabled}
          setAutoGitEnabled={setAutoGitEnabled}
          autoGitAiCommitMessagesEnabled={autoGitAiCommitMessagesEnabled}
          setAutoGitAiCommitMessagesEnabled={setAutoGitAiCommitMessagesEnabled}
          autoGitIdleThresholdSeconds={autoGitIdleThresholdSeconds}
          setAutoGitIdleThresholdSeconds={setAutoGitIdleThresholdSeconds}
          autoGitInactiveThresholdSeconds={autoGitInactiveThresholdSeconds}
          setAutoGitInactiveThresholdSeconds={setAutoGitInactiveThresholdSeconds}
        />
      </SettingsSection>

      <SettingsSection id={SETTINGS_SECTION_IDS.appearance}>
        <SectionHeading title={t('settings.appearance.title')} />
        <SettingsGroup>
          <AppearanceSettingsSection t={t} themeMode={themeMode} setThemeMode={setThemeMode} dockIconVariant={dockIconVariant} setDockIconVariant={setDockIconVariant} />
          <LanguageSettingsSection
            t={t}
            locale={locale}
            systemLocale={systemLocale}
            uiLanguage={uiLanguage}
            setUiLanguage={setUiLanguage}
          />
        </SettingsGroup>
      </SettingsSection>

      <SettingsSection id={SETTINGS_SECTION_IDS.transparency}>
        <TransparencySettingsSection {...options} />
      </SettingsSection>

      {/* Widgets: dictation pill + text-to-speech — everything that floats on
          top of the workspace or speaks (user-requested grouping). */}
      <SettingsSection id={SETTINGS_SECTION_IDS.widgets}>
        <SectionHeading icon={<Microphone size={16} aria-hidden="true" />} title={t('settings.widgets.title')} />
        <DictationSettingsSection {...options} />
      </SettingsSection>

      <SettingsSection id={SETTINGS_SECTION_IDS.miniApps}>
        <MiniAppsSettingsSection
          t={t}
          miniAppsEnabled={miniAppsEnabled}
          setMiniAppsEnabled={setMiniAppsEnabled}
          webAccessEnabled={miniAppsWebAccessEnabled}
          setWebAccessEnabled={setMiniAppsWebAccessEnabled}
        />
      </SettingsSection>

      <SettingsSection id={SETTINGS_SECTION_IDS.buzz}>
        <BuzzSettingsSection
          t={t}
          locale={locale}
          buzzEnabled={buzzEnabled}
          setBuzzEnabled={setBuzzEnabled}
          buzzChannel={buzzChannel}
          setBuzzChannel={setBuzzChannel}
        />
      </SettingsSection>
    </>
  )
}

function SettingsContentSections(options: SettingsBodyProps) {
  const { t, dateDisplayFormat, setDateDisplayFormat, defaultNoteWidth, setDefaultNoteWidth, sidebarTypePluralizationEnabled, setSidebarTypePluralizationEnabled, initialH1AutoRename, setInitialH1AutoRename, hideGitignoredFiles, setHideGitignoredFiles, allNotesFileVisibility, setAllNotesFileVisibility } = options
  return (
    <SettingsSection id={SETTINGS_SECTION_IDS.content}>
      <VaultContentSettingsSection
        t={t}
        dateDisplayFormat={dateDisplayFormat}
        setDateDisplayFormat={setDateDisplayFormat}
        defaultNoteWidth={defaultNoteWidth}
        setDefaultNoteWidth={setDefaultNoteWidth}
        sidebarTypePluralizationEnabled={sidebarTypePluralizationEnabled}
        setSidebarTypePluralizationEnabled={setSidebarTypePluralizationEnabled}
        initialH1AutoRename={initialH1AutoRename}
        setInitialH1AutoRename={setInitialH1AutoRename}
        hideGitignoredFiles={hideGitignoredFiles}
        setHideGitignoredFiles={setHideGitignoredFiles}
        allNotesFileVisibility={allNotesFileVisibility}
        setAllNotesFileVisibility={setAllNotesFileVisibility}
      />
    </SettingsSection>
  )
}

function SettingsAgentWorkflowSections(options: SettingsBodyProps) {
  const { t, autoAdvanceInboxAfterOrganize, setAutoAdvanceInboxAfterOrganize, aiFeaturesEnabled, setAiFeaturesEnabled, aiAgentsStatus, defaultAiAgent, setDefaultAiAgent, defaultAiTarget, setDefaultAiTarget, aiModelProviders, setAiModelProviders, onCopyMcpConfig, explicitOrganization, setExplicitOrganization, crashReporting, setCrashReporting, analytics, setAnalytics } = options
  return (
    <>
      <SettingsSection id={SETTINGS_SECTION_IDS.ai}>
        <AiAgentSettingsSection
          t={t}
          aiFeaturesEnabled={aiFeaturesEnabled}
          setAiFeaturesEnabled={setAiFeaturesEnabled}
          aiAgentsStatus={aiAgentsStatus}
          defaultAiAgent={defaultAiAgent}
          setDefaultAiAgent={setDefaultAiAgent}
          defaultAiTarget={defaultAiTarget}
          setDefaultAiTarget={setDefaultAiTarget}
          aiModelProviders={aiModelProviders}
          setAiModelProviders={setAiModelProviders}
          onCopyMcpConfig={onCopyMcpConfig}
        />
      </SettingsSection>

      <SettingsSection id={SETTINGS_SECTION_IDS.workflow}>
        <OrganizationWorkflowSection
          t={t}
          checked={explicitOrganization}
          onChange={setExplicitOrganization}
          autoAdvanceInboxAfterOrganize={autoAdvanceInboxAfterOrganize}
          onChangeAutoAdvanceInboxAfterOrganize={setAutoAdvanceInboxAfterOrganize}
        />
      </SettingsSection>

      <SettingsSection id={SETTINGS_SECTION_IDS.privacy}>
        <PrivacySettingsSection
          t={t}
          crashReporting={crashReporting}
          setCrashReporting={setCrashReporting}
          analytics={analytics}
          setAnalytics={setAnalytics}
        />
      </SettingsSection>
    </>
  )
}

function SyncAndUpdatesSection({
  t,
  pullInterval,
  setPullInterval,
  releaseChannel,
  setReleaseChannel,
  automaticUpdateChecksEnabled,
  setAutomaticUpdateChecksEnabled,
}: Pick<
  SettingsBodyProps,
  | 't'
  | 'pullInterval'
  | 'setPullInterval'
  | 'releaseChannel'
  | 'setReleaseChannel'
  | 'automaticUpdateChecksEnabled'
  | 'setAutomaticUpdateChecksEnabled'
>) {
  return (
    <>
      <SectionHeading title={t('settings.sync.title')} />

      <SettingsGroup>
        <SettingsRow label={t('settings.pullInterval')} description={t('settings.pullIntervalDescription')}>
          <SelectControl
            ariaLabel={t('settings.pullInterval')}
            value={`${pullInterval}`}
            onValueChange={(value) => setPullInterval(Number(value))}
            options={PULL_INTERVAL_OPTIONS.map((value) => ({
              value: `${value}`,
              label: `${value}`,
            }))}
            testId="settings-pull-interval"
            autoFocus={true}
          />
        </SettingsRow>

        <SettingsRow label={t('settings.releaseChannel')} description={t('settings.releaseChannelDescription')}>
          <SelectControl
            ariaLabel={t('settings.releaseChannel')}
            value={releaseChannel}
            onValueChange={(value) => setReleaseChannel(value as ReleaseChannel)}
            options={[
              { value: 'stable', label: t('settings.releaseStable') },
              { value: 'alpha', label: t('settings.releaseAlpha') },
            ]}
            testId="settings-release-channel"
          />
        </SettingsRow>

        <SettingsSwitchRow
          label={t('settings.automaticUpdateChecks')}
          description={t('settings.automaticUpdateChecksDescription')}
          checked={automaticUpdateChecksEnabled}
          onChange={setAutomaticUpdateChecksEnabled}
          testId="settings-automatic-update-checks"
        />
      </SettingsGroup>
    </>
  )
}

function AppearanceSettingsSection({
  t,
  themeMode,
  setThemeMode,
  dockIconVariant,
  setDockIconVariant,
}: {
  t: Translate
  themeMode: ThemeMode
  setThemeMode: (value: ThemeMode) => void
  dockIconVariant: 'variant-1' | 'variant-2' | 'variant-3' | 'variant-4' | 'variant-5' | 'variant-6' | 'variant-7' | 'variant-8' | 'variant-9' | 'variant-10' | null
  setDockIconVariant: (value: 'variant-1' | 'variant-2' | 'variant-3' | 'variant-4' | 'variant-5' | 'variant-6' | 'variant-7' | 'variant-8' | 'variant-9' | 'variant-10' | null) => void
}) {
  return (
    <>
      <SettingsRow label={t('settings.theme.label')} description={t('settings.appearance.description')}>
        <ThemeModeControl value={themeMode} onChange={setThemeMode} t={t} />
      </SettingsRow>
      <SettingsRow label={t('settings.appearance.dockIcon')} description={t('settings.appearance.dockIconDescription')}>
        <SelectControl
          ariaLabel={t('settings.appearance.dockIcon')}
          value={dockIconVariant ?? 'variant-1'}
          onValueChange={(value) => setDockIconVariant(value as 'variant-1' | 'variant-2' | 'variant-3' | 'variant-4' | 'variant-5' | 'variant-6' | 'variant-7' | 'variant-8' | 'variant-9' | 'variant-10' | null)}
          options={[
            { value: 'variant-1', label: t('settings.appearance.dockIconVariant1') },
            { value: 'variant-2', label: t('settings.appearance.dockIconVariant2') },
            { value: 'variant-3', label: t('settings.appearance.dockIconVariant3') },
            { value: 'variant-4', label: t('settings.appearance.dockIconVariant4') },
            { value: 'variant-5', label: t('settings.appearance.dockIconVariant5') },
            { value: 'variant-6', label: t('settings.appearance.dockIconVariant6') },
            { value: 'variant-7', label: t('settings.appearance.dockIconVariant7') },
            { value: 'variant-8', label: t('settings.appearance.dockIconVariant8') },
            { value: 'variant-9', label: t('settings.appearance.dockIconVariant9') },
            { value: 'variant-10', label: t('settings.appearance.dockIconVariant10') },
          ]}
          testId="settings-dock-icon-variant"
        />
      </SettingsRow>
    </>
  )
}

function ThemeModeControl({
  value,
  onChange,
  t,
}: {
  value: ThemeMode
  onChange: (value: ThemeMode) => void
  t: Translate
}) {
  return (
    <div
      className="inline-flex w-full rounded-md border border-border bg-muted p-1"
      role="radiogroup"
      aria-label={t('settings.theme.label')}
      data-testid="settings-theme-mode"
    >
      <ThemeModeButton label={t('settings.theme.light')} selected={value === 'light'} value="light" onSelect={onChange}>
        <Sun size={14} />
      </ThemeModeButton>
      <ThemeModeButton label={t('settings.theme.dark')} selected={value === 'dark'} value="dark" onSelect={onChange}>
        <Moon size={14} />
      </ThemeModeButton>
      <ThemeModeButton
        label={t('settings.theme.system')}
        selected={value === 'system'}
        value="system"
        onSelect={onChange}
      >
        <Monitor size={14} />
      </ThemeModeButton>
    </div>
  )
}

function ThemeModeButton({
  children,
  label,
  selected,
  value,
  onSelect,
}: {
  children: ReactNode
  label: string
  selected: boolean
  value: ThemeMode
  onSelect: (value: ThemeMode) => void
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      role="radio"
      aria-checked={selected}
      aria-label={label}
      data-testid={`settings-theme-${value}`}
      className={
        selected
          ? 'h-7 flex-1 border border-border bg-background text-foreground shadow-xs hover:bg-background'
          : 'h-7 flex-1 text-muted-foreground hover:text-foreground'
      }
      onClick={() => onSelect(value)}
    >
      {children}
      {label}
    </Button>
  )
}

function buildLanguageOptions(t: Translate, locale: AppLocale, systemLocale: AppLocale) {
  return [
    {
      value: SYSTEM_UI_LANGUAGE,
      label: t('settings.language.system', {
        language: localeDisplayName(systemLocale, locale),
      }),
    },
    ...APP_LOCALES.map((appLocale) => ({
      value: appLocale,
      label: localeDisplayName(appLocale, locale),
    })),
  ]
}

function LanguageSettingsSection({
  t,
  locale,
  systemLocale,
  uiLanguage,
  setUiLanguage,
}: Pick<SettingsBodyProps, 't' | 'locale' | 'systemLocale' | 'uiLanguage' | 'setUiLanguage'>) {
  return (
    <SettingsRow
      label={t('settings.language.title')}
      description={`${t('settings.language.description')} ${t('settings.language.summary')}`}
    >
      <SelectControl
        ariaLabel={t('settings.language.label')}
        value={uiLanguage}
        onValueChange={(value) => setUiLanguage(value as UiLanguagePreference)}
        options={buildLanguageOptions(t, locale, systemLocale)}
        testId="settings-ui-language"
      />
    </SettingsRow>
  )
}

function buildDefaultAiTargetOptions(
  aiAgentsStatus: AiAgentsStatus,
  providers: AiModelProvider[],
  t: Translate,
): Array<{ value: string; label: string }> {
  const agentOptions = AI_AGENT_DEFINITIONS.map((definition) => {
    const status = getAiAgentAvailability(aiAgentsStatus, definition.id)
    const suffix =
      status.status === 'installed'
      ? ` (${t('settings.aiAgents.installed')}${status.version ? ` ${status.version}` : ''})`
      : ` (${t('settings.aiAgents.missing')})`
    return {
      value: agentTargetId(definition.id),
      label: `${t('settings.aiAgents.agentGroup')}: ${definition.label}${suffix}`,
    }
  })
  const modelOptions = configuredModelTargets(providers).map((target) => ({
    value: target.id,
    label: `${target.provider.kind === 'ollama' || target.provider.kind === 'lm_studio' ? t('settings.aiAgents.localGroup') : t('settings.aiAgents.apiGroup')}: ${target.label}`,
  }))
  return [...agentOptions, ...modelOptions]
}

function AiAgentSettingsSection(
  functionOptions: Pick<
  SettingsBodyProps,
  | 't'
  | 'aiFeaturesEnabled'
  | 'setAiFeaturesEnabled'
  | 'aiAgentsStatus'
  | 'defaultAiAgent'
  | 'setDefaultAiAgent'
  | 'defaultAiTarget'
  | 'setDefaultAiTarget'
  | 'aiModelProviders'
  | 'setAiModelProviders'
  | 'onCopyMcpConfig'
  >,
) {
  const {
    t,
    aiFeaturesEnabled,
    setAiFeaturesEnabled,
    aiAgentsStatus,
    defaultAiAgent,
    setDefaultAiAgent,
    defaultAiTarget,
    setDefaultAiTarget,
    aiModelProviders,
    setAiModelProviders,
    onCopyMcpConfig,
  } = functionOptions
  const selectedTarget = resolveAiTarget({
    default_ai_agent: defaultAiAgent,
    default_ai_target: defaultAiTarget,
    ai_model_providers: aiModelProviders,
  } as Settings)

  return (
    <>
      <SectionHeading title={t('settings.aiAgents.title')} />

      <SettingsGroup>
        <SettingsSwitchRow
          label={t('settings.aiFeatures.enable')}
          description={t('settings.aiFeatures.enableDescription')}
          checked={aiFeaturesEnabled}
          onChange={setAiFeaturesEnabled}
          testId="settings-ai-features-enabled"
        />
      </SettingsGroup>

      {aiFeaturesEnabled ? (
        <>
          <SettingsGroup>
            <SettingsRow
              label={t('settings.aiAgents.defaultTarget')}
              description={renderDefaultAiTargetSummary(selectedTarget, aiAgentsStatus, t)}
              controlWidth="wide"
            >
              <SelectControl
                ariaLabel={t('settings.aiAgents.defaultTarget')}
                value={defaultAiTarget}
                onValueChange={(value) => {
                  setDefaultAiTarget(value)
                  if (value.startsWith('agent:')) {
                    const agent = value.replace('agent:', '') as AiAgentId
                    setDefaultAiAgent(agent)
                  }
                }}
                options={buildDefaultAiTargetOptions(aiAgentsStatus, aiModelProviders, t)}
                testId="settings-default-ai-agent"
              />
            </SettingsRow>
          </SettingsGroup>

          <AiTargetManagementTabs
            t={t}
            aiAgentsStatus={aiAgentsStatus}
            aiModelProviders={aiModelProviders}
            setAiModelProviders={setAiModelProviders}
            onCopyMcpConfig={onCopyMcpConfig}
          />
        </>
      ) : null}
    </>
  )
}

function AiTargetManagementTabs({
  t,
  aiAgentsStatus,
  aiModelProviders,
  setAiModelProviders,
  onCopyMcpConfig,
}: {
  t: Translate
  aiAgentsStatus: AiAgentsStatus
  aiModelProviders: AiModelProvider[]
  setAiModelProviders: (value: AiModelProvider[]) => void
  onCopyMcpConfig?: () => void
}) {
  return (
    <Tabs defaultValue="agents" className="gap-3">
      <TabsList className="grid h-9 w-full grid-cols-3">
        <TabsTrigger value="agents">{t('settings.aiAgents.agentGroup')}</TabsTrigger>
        <TabsTrigger value="local">{t('settings.aiAgents.localGroup')}</TabsTrigger>
        <TabsTrigger value="api">{t('settings.aiAgents.apiGroup')}</TabsTrigger>
      </TabsList>
      <TabsContent value="agents" className="space-y-3">
        <AiAgentsInstalledSection t={t} aiAgentsStatus={aiAgentsStatus} />
        {onCopyMcpConfig ? <CopyMcpConfigButton t={t} onCopyMcpConfig={onCopyMcpConfig} /> : null}
      </TabsContent>
      <TabsContent value="local">
        <AiProviderSettings t={t} mode="local" providers={aiModelProviders} onChange={setAiModelProviders} />
      </TabsContent>
      <TabsContent value="api">
        <AiProviderSettings t={t} mode="api" providers={aiModelProviders} onChange={setAiModelProviders} />
      </TabsContent>
    </Tabs>
  )
}

function CopyMcpConfigButton({ t, onCopyMcpConfig }: { t: Translate; onCopyMcpConfig: () => void }) {
  return (
    <Button
      type="button"
      variant="outline"
      size="sm"
      onClick={onCopyMcpConfig}
      className="w-fit gap-2"
      aria-label={t('ai.panel.copyMcpConfig')}
      data-testid="settings-copy-mcp-config"
    >
      <Copy size={15} />
      {t('ai.panel.copyMcpConfig')}
    </Button>
  )
}

function AiAgentsInstalledSection({ t, aiAgentsStatus }: { t: Translate; aiAgentsStatus: AiAgentsStatus }) {
  return (
    <div className="rounded-md border border-border bg-card p-3">
      <div className="text-sm font-medium text-foreground">{t('settings.aiAgents.installedTitle')}</div>
      <div className="mt-1 text-xs leading-5 text-muted-foreground">{t('settings.aiAgents.installedDescription')}</div>
      <div className="mt-3 grid grid-cols-2 gap-2">
        {AI_AGENT_DEFINITIONS.map((definition) => {
          const status = getAiAgentAvailability(aiAgentsStatus, definition.id)
          const installed = status.status === 'installed'
          return (
            <div key={definition.id} className="rounded-md border border-border bg-background px-3 py-2">
              <div className="flex items-center justify-between gap-2">
                <div className="flex min-w-0 items-center gap-2">
                  <AiAgentIcon agent={definition.id} size={16} />
                  <div className="truncate text-sm font-medium text-foreground">{definition.label}</div>
                </div>
                <div className={installed ? 'text-xs text-emerald-700' : 'text-xs text-muted-foreground'}>
                  {installed ? t('settings.aiAgents.installed') : t('settings.aiAgents.missing')}
                </div>
              </div>
              <div className="mt-1 truncate text-xs text-muted-foreground">
                {status.version || t('settings.aiAgents.noVersion')}
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}

function renderDefaultAiAgentSummary(defaultAiAgent: AiAgentId, aiAgentsStatus: AiAgentsStatus, t: Translate): string {
  const definition = getAiAgentDefinition(defaultAiAgent)
  const status = getAiAgentAvailability(aiAgentsStatus, defaultAiAgent)
  if (status.status === 'installed') {
    return t('settings.aiAgents.ready', {
      agent: definition.label,
      version: status.version ? ` ${status.version}` : '',
    })
  }
  return t('settings.aiAgents.notInstalled', { agent: definition.label })
}

function renderDefaultAiTargetSummary(
  target: ReturnType<typeof resolveAiTarget>,
  aiAgentsStatus: AiAgentsStatus,
  t: Translate,
): string {
  if (target.kind === 'api_model') {
    const storage =
      target.provider.api_key_storage === 'local_file'
      ? t('settings.aiAgents.apiLocalKey')
      : target.provider.api_key_env_var
          ? t('settings.aiAgents.apiEnv', {
              env: target.provider.api_key_env_var,
            })
      : t('settings.aiAgents.apiNoKey')
    return t('settings.aiAgents.apiReady', { target: target.label, storage })
  }
  return renderDefaultAiAgentSummary(target.agent, aiAgentsStatus, t)
}

function OrganizationWorkflowSection({
  t,
  checked,
  onChange,
  autoAdvanceInboxAfterOrganize,
  onChangeAutoAdvanceInboxAfterOrganize,
}: {
  t: Translate
  checked: boolean
  onChange: (value: boolean) => void
  autoAdvanceInboxAfterOrganize: boolean
  onChangeAutoAdvanceInboxAfterOrganize: (value: boolean) => void
}) {
  return (
    <>
      <SectionHeading title={t('settings.workflow.title')} />

      <SettingsGroup>
        <SettingsSwitchRow
          label={t('settings.workflow.explicit')}
          description={t('settings.workflow.explicitDescription')}
          checked={checked}
          onChange={onChange}
          testId="settings-explicit-organization"
        />

        <SettingsSwitchRow
          label={t('settings.workflow.autoAdvance')}
          description={t('settings.workflow.autoAdvanceDescription')}
          checked={autoAdvanceInboxAfterOrganize}
          onChange={onChangeAutoAdvanceInboxAfterOrganize}
          testId="settings-auto-advance-inbox-after-organize"
        />
      </SettingsGroup>
    </>
  )
}

const DICTATION_POSITION_OPTIONS = [
  { value: 'bottom-right', label: 'settings.dictation.positionBottomRight' },
  { value: 'bottom-left', label: 'settings.dictation.positionBottomLeft' },
] as const

const DICTATION_BACKEND_OPTIONS = [
  { value: 'web_speech', label: 'settings.dictation.backendWebSpeech' },
  { value: 'fluidvoice', label: 'settings.dictation.backendFluidvoice' },
] as const

const FLUIDVOICE_MODEL_FALLBACKS = ['nemotron', 'parakeet', 'apple_speech', 'whisper']

/** Loads the FluidVoice model list (same source the dictation pill uses). */
function useFluidVoiceModelOptions(): string[] {
  const [models, setModels] = useState<string[]>(FLUIDVOICE_MODEL_FALLBACKS)
  useEffect(() => {
    let cancelled = false
    const request = isTauri()
      ? invoke<string[]>('fluidvoice_models')
      : mockInvoke<string[]>('fluidvoice_models', {})
    request
      .then((available) => {
        if (!cancelled && Array.isArray(available) && available.length > 0) setModels(available)
      })
      .catch(() => undefined)
    return () => {
      cancelled = true
    }
  }, [])
  return models
}

function formatModelLabel(model: string): string {
  return model.charAt(0).toUpperCase() + model.slice(1).replace(/_/g, ' ')
}

function PercentSliderControl({
  label,
  value,
  onChange,
  testId,
}: {
  label: string
  value: number
  onChange: (value: number) => void
  testId: string
}) {
  return (
    <div className="flex items-center gap-3">
      <Slider ariaLabel={label} min={0.05} max={1} step={0.05} value={value} onValueChange={onChange} />
      <span className="w-10 text-right text-xs tabular-nums text-muted-foreground" data-testid={testId}>
        {Math.round(value * 100)}%
      </span>
    </div>
  )
}

function BlurSliderControl({
  label,
  value,
  onChange,
  testId,
}: {
  label: string
  value: number
  onChange: (value: number) => void
  testId: string
}) {
  return (
    <div className="flex items-center gap-3">
      <Slider ariaLabel={label} min={0} max={20} step={1} value={value} onValueChange={onChange} />
      <span className="w-10 text-right text-xs tabular-nums text-muted-foreground" data-testid={testId}>
        {value}px
      </span>
    </div>
  )
}

/** Warp-style transparency: window-level opacity/blur plus per-panel frosted glass. */
function TransparencySettingsSection(options: SettingsBodyProps) {
  const {
    t,
    windowOpacity,
    setWindowOpacity,
    windowBlurRadius,
    setWindowBlurRadius,
    sidebarOpacity,
    setSidebarOpacity,
    sidebarBlurRadius,
    setSidebarBlurRadius,
    editorOpacity,
    setEditorOpacity,
    editorBlurRadius,
    setEditorBlurRadius,
    aiPanelOpacity,
    setAiPanelOpacity,
    aiPanelBlurRadius,
    setAiPanelBlurRadius,
    editorFontFamily,
    setEditorFontFamily,
    aiChatFontFamily,
    setAiChatFontFamily,
    sidebarFontFamily,
    setSidebarFontFamily,
  } = options
  return (
    <>
      <SectionHeading icon={<Sparkle size={16} aria-hidden="true" />} title={t('settings.transparency.title')} />
      <SettingsGroup>
        <SettingsRow
          label={t('settings.transparency.windowOpacity')}
          description={t('settings.transparency.windowOpacityDescription')}
        >
          <PercentSliderControl
            label={t('settings.transparency.windowOpacity')}
            value={windowOpacity}
            onChange={setWindowOpacity}
            testId="settings-window-opacity-value"
          />
        </SettingsRow>
        <SettingsRow
          label={t('settings.transparency.windowBlur')}
          description={t('settings.transparency.windowBlurDescription')}
        >
          <BlurSliderControl
            label={t('settings.transparency.windowBlur')}
            value={windowBlurRadius}
            onChange={setWindowBlurRadius}
            testId="settings-window-blur-value"
          />
        </SettingsRow>
      </SettingsGroup>
      <SettingsGroup>
        <SettingsRow
          label={t('settings.transparency.sidebarOpacity')}
          description={t('settings.transparency.sidebarOpacityDescription')}
        >
          <PercentSliderControl
            label={t('settings.transparency.sidebarOpacity')}
            value={sidebarOpacity}
            onChange={setSidebarOpacity}
            testId="settings-sidebar-opacity-value"
          />
        </SettingsRow>
        <SettingsRow label={t('settings.transparency.sidebarBlur')}>
          <BlurSliderControl
            label={t('settings.transparency.sidebarBlur')}
            value={sidebarBlurRadius}
            onChange={setSidebarBlurRadius}
            testId="settings-sidebar-blur-value"
          />
        </SettingsRow>
        <SettingsRow
          label={t('settings.transparency.editorOpacity')}
          description={t('settings.transparency.editorOpacityDescription')}
        >
          <PercentSliderControl
            label={t('settings.transparency.editorOpacity')}
            value={editorOpacity}
            onChange={setEditorOpacity}
            testId="settings-editor-opacity-value"
          />
        </SettingsRow>
        <SettingsRow label={t('settings.transparency.editorBlur')}>
          <BlurSliderControl
            label={t('settings.transparency.editorBlur')}
            value={editorBlurRadius}
            onChange={setEditorBlurRadius}
            testId="settings-editor-blur-value"
          />
        </SettingsRow>
        <SettingsRow
          label={t('settings.transparency.aiPanelOpacity')}
          description={t('settings.transparency.aiPanelOpacityDescription')}
        >
          <PercentSliderControl
            label={t('settings.transparency.aiPanelOpacity')}
            value={aiPanelOpacity}
            onChange={setAiPanelOpacity}
            testId="settings-ai-panel-opacity-value"
          />
        </SettingsRow>
        <SettingsRow label={t('settings.transparency.aiPanelBlur')}>
          <BlurSliderControl
            label={t('settings.transparency.aiPanelBlur')}
            value={aiPanelBlurRadius}
            onChange={setAiPanelBlurRadius}
            testId="settings-ai-panel-blur-value"
          />
        </SettingsRow>
      </SettingsGroup>
      <SettingsGroup>
        <SettingsRow label={t('settings.transparency.editorFont')}>
          <Input
            value={editorFontFamily}
            onChange={(event) => setEditorFontFamily(event.target.value)}
            placeholder={t('settings.transparency.fontPlaceholder')}
            aria-label={t('settings.transparency.editorFont')}
            data-testid="settings-editor-font"
            className="bg-transparent"
          />
        </SettingsRow>
        <SettingsRow label={t('settings.transparency.aiChatFont')}>
          <Input
            value={aiChatFontFamily}
            onChange={(event) => setAiChatFontFamily(event.target.value)}
            placeholder={t('settings.transparency.fontPlaceholder')}
            aria-label={t('settings.transparency.aiChatFont')}
            data-testid="settings-ai-chat-font"
            className="bg-transparent"
          />
        </SettingsRow>
        <SettingsRow label={t('settings.transparency.sidebarFont')}>
          <Input
            value={sidebarFontFamily}
            onChange={(event) => setSidebarFontFamily(event.target.value)}
            placeholder={t('settings.transparency.fontPlaceholder')}
            aria-label={t('settings.transparency.sidebarFont')}
            data-testid="settings-sidebar-font"
            className="bg-transparent"
          />
        </SettingsRow>
      </SettingsGroup>
    </>
  )
}

/** Dictation pill controls plus FluidVoice backend/model and AI tool toggles. */
function DictationSettingsSection(options: SettingsBodyProps) {
  const {
    t,
    dictationEnabled,
    setDictationEnabled,
    dictationPosition,
    setDictationPosition,
    dictationOpacity,
    setDictationOpacity,
    dictationBackend,
    setDictationBackend,
    fluidvoiceModel,
    setFluidvoiceModel,
    ttsEngine,
    setTtsEngine,
    kokoroVoice,
    setKokoroVoice,
    kokoroSpeed,
    setKokoroSpeed,
    ttsHighlightEnabled,
    setTtsHighlightEnabled,
    grammarCheckEnabled,
    setGrammarCheckEnabled,
    ocrEnabled,
    setOcrEnabled,
    documentConversionEnabled,
    setDocumentConversionEnabled,
  } = options
  const models = useFluidVoiceModelOptions()
  const modelOptions = models.map((model) => ({ value: model, label: formatModelLabel(model) }))
  return (
    <>
      <SectionHeading icon={<Microphone size={16} aria-hidden="true" />} title={t('settings.dictation.title')} />
      <SettingsGroup>
        <SettingsSwitchRow
          label={t('settings.dictation.enable')}
          description={t('settings.dictation.enableDescription')}
          checked={dictationEnabled}
          onChange={setDictationEnabled}
          testId="settings-dictation-enabled"
        />
        <SettingsRow label={t('settings.dictation.position')}>
          <SelectControl
            value={dictationPosition}
            onValueChange={(value) => {
              if (value === 'bottom-right' || value === 'bottom-left') setDictationPosition(value)
            }}
            options={DICTATION_POSITION_OPTIONS.map((option) => ({
              value: option.value,
              label: t(option.label as Parameters<typeof t>[0]),
            }))}
            testId="settings-dictation-position"
            ariaLabel={t('settings.dictation.position')}
          />
        </SettingsRow>
        <SettingsRow
          label={t('settings.dictation.opacity')}
          description={t('settings.dictation.opacityDescription')}
        >
          <PercentSliderControl
            label={t('settings.dictation.opacity')}
            value={dictationOpacity}
            onChange={setDictationOpacity}
            testId="settings-dictation-opacity-value"
          />
        </SettingsRow>
        <SettingsRow
          label={t('settings.dictation.backend')}
          description={t('settings.dictation.backendDescription')}
        >
          <SelectControl
            value={dictationBackend}
            onValueChange={(value) => {
              if (value === 'web_speech' || value === 'fluidvoice') setDictationBackend(value)
            }}
            options={DICTATION_BACKEND_OPTIONS.map((option) => ({
              value: option.value,
              label: t(option.label as Parameters<typeof t>[0]),
            }))}
            testId="settings-dictation-backend"
            ariaLabel={t('settings.dictation.backend')}
          />
        </SettingsRow>
        <SettingsRow
          label={t('settings.dictation.model')}
          description={t('settings.dictation.modelDescription')}
        >
          <SelectControl
            value={modelOptions.some((option) => option.value === fluidvoiceModel) ? fluidvoiceModel : (modelOptions[0]?.value ?? 'parakeet')}
            onValueChange={setFluidvoiceModel}
            options={modelOptions}
            testId="settings-fluidvoice-model"
            ariaLabel={t('settings.dictation.model')}
          />
        </SettingsRow>
      </SettingsGroup>
      <SettingsGroup>
        <SettingsSwitchRow
          label={t('settings.dictation.grammarCheck')}
          description={t('settings.dictation.grammarCheckDescription')}
          checked={grammarCheckEnabled}
          onChange={setGrammarCheckEnabled}
          testId="settings-grammar-check"
        />
        <SettingsSwitchRow
          label={t('settings.dictation.ocr')}
          description={t('settings.dictation.ocrDescription')}
          checked={ocrEnabled}
          onChange={setOcrEnabled}
          testId="settings-ocr"
        />
        <SettingsSwitchRow
          label={t('settings.dictation.documentConversion')}
          description={t('settings.dictation.documentConversionDescription')}
          checked={documentConversionEnabled}
          onChange={setDocumentConversionEnabled}
          testId="settings-document-conversion"
        />
      </SettingsGroup>
      <TtsSettingsSection
        t={t}
        ttsEngine={ttsEngine}
        setTtsEngine={setTtsEngine}
        kokoroVoice={kokoroVoice}
        setKokoroVoice={setKokoroVoice}
        kokoroSpeed={kokoroSpeed}
        setKokoroSpeed={setKokoroSpeed}
        ttsHighlightEnabled={ttsHighlightEnabled}
        setTtsHighlightEnabled={setTtsHighlightEnabled}
      />
    </>
  )
}

const KOKORO_SPEED_OPTIONS = [0.5, 0.75, 1, 1.1, 1.25, 1.5, 2]

interface TtsSettingsSectionProps {
  t: Translate
  ttsEngine: 'system' | 'kokoro'
  setTtsEngine: (value: 'system' | 'kokoro') => void
  kokoroVoice: string
  setKokoroVoice: (value: string) => void
  kokoroSpeed: number
  setKokoroSpeed: (value: number) => void
  ttsHighlightEnabled: boolean
  setTtsHighlightEnabled: (value: boolean) => void
}

/** Text-to-speech section (plan 4 §1.7): engine, voice, speed, highlighting. */
function TtsSettingsSection(props: TtsSettingsSectionProps) {
  const { t, ttsEngine, setTtsEngine, kokoroVoice, setKokoroVoice, kokoroSpeed, setKokoroSpeed, ttsHighlightEnabled, setTtsHighlightEnabled } = props
  const [kokoroReady, setKokoroReady] = useState(false)
  const [voices, setVoices] = useState<string[]>([])

  useEffect(() => {
    let cancelled = false
    void (isTauri() ? invoke<boolean>('kokoro_available') : mockInvoke<boolean>('kokoro_available'))
      .then((available) => {
        if (cancelled || !available) return
        setKokoroReady(true)
        return (isTauri() ? invoke<string[]>('kokoro_list_voices') : mockInvoke<string[]>('kokoro_list_voices')).then((list) => {
          if (!cancelled) setVoices(list)
        })
      })
      .catch(() => undefined)
    return () => {
      cancelled = true
    }
  }, [])

  const engineOptions = [
    { value: 'system' as const, label: t('settings.tts.engineSystem') },
    ...(kokoroReady ? [{ value: 'kokoro' as const, label: t('settings.tts.engineKokoro') }] : []),
  ]
  const voiceOptions = voices.map((voice) => ({ value: voice, label: voice }))
  const speedOptions = KOKORO_SPEED_OPTIONS.map((speed) => ({ value: String(speed), label: `${speed}x` }))

  return (
    <SettingsGroup>
      <SectionHeading icon={<SpeakerHigh size={16} aria-hidden="true" />} title={t('settings.tts.title')} />
      <SettingsRow label={t('settings.tts.engine')} description={t('settings.tts.engineDescription')}>
        <SelectControl
          value={ttsEngine}
          onValueChange={(value) => {
            if (value === 'system' || value === 'kokoro') setTtsEngine(value)
          }}
          options={engineOptions}
          testId="settings-tts-engine"
          ariaLabel={t('settings.tts.engine')}
        />
      </SettingsRow>
      {ttsEngine === 'kokoro' && !kokoroReady && (
        <p className="px-3 text-[12px] text-muted-foreground" data-testid="settings-tts-kokoro-missing">
          {t('settings.tts.kokoroNotInstalled')}
        </p>
      )}
      {ttsEngine === 'kokoro' && kokoroReady && (
        <>
          <SettingsRow label={t('settings.tts.voice')}>
            <SelectControl
              value={voiceOptions.some((option) => option.value === kokoroVoice) ? kokoroVoice : (voiceOptions[0]?.value ?? 'af_sky')}
              onValueChange={setKokoroVoice}
              options={voiceOptions}
              testId="settings-tts-voice"
              ariaLabel={t('settings.tts.voice')}
            />
          </SettingsRow>
          <SettingsRow label={t('settings.tts.speed')}>
            <SelectControl
              value={String(kokoroSpeed)}
              onValueChange={(value) => setKokoroSpeed(Number(value))}
              options={speedOptions}
              testId="settings-tts-speed"
              ariaLabel={t('settings.tts.speed')}
            />
          </SettingsRow>
        </>
      )}
      <SettingsSwitchRow
        label={t('settings.tts.highlight')}
        description={t('settings.tts.highlightDescription')}
        checked={ttsHighlightEnabled}
        onChange={setTtsHighlightEnabled}
        testId="settings-tts-highlight"
      />
    </SettingsGroup>
  )
}
