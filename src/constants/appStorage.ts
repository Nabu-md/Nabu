export const APP_STORAGE_KEYS = {
  theme: 'nabu-theme',
  zoom: 'nabu:zoom-level',
  viewMode: 'nabu-view-mode',
  tagColors: 'nabu:tag-color-overrides',
  statusColors: 'nabu:status-color-overrides',
  propertyModes: 'nabu:display-mode-overrides',
  configMigrationFlag: 'nabu:config-migrated-to-vault',
  legacyMigrationFlag: 'nabu:legacy-storage-migrated',
  sortPreferences: 'nabu-sort-preferences',
  sidebarCollapsed: 'nabu:sidebar-collapsed',
  rightPanelCollapsed: 'nabu:right-panel-collapsed',
  lastActiveNotePath: 'nabu:last-active-note-path',
  layoutPanels: 'nabu:layout-panels',
  welcomeDismissed: 'nabu_welcome_dismissed',
} as const

export const LEGACY_APP_STORAGE_KEYS = {
  theme: 'laputa-theme',
  zoom: 'laputa:zoom-level',
  viewMode: 'laputa-view-mode',
  tagColors: 'laputa:tag-color-overrides',
  statusColors: 'laputa:status-color-overrides',
  propertyModes: 'laputa:display-mode-overrides',
  configMigrationFlag: 'laputa:config-migrated-to-vault',
  sortPreferences: 'laputa-sort-preferences',
  sidebarCollapsed: 'laputa:sidebar-collapsed',
  layoutPanels: 'laputa:layout-panels',
  welcomeDismissed: 'laputa_welcome_dismissed',
} as const

type MigratableStorageKey = keyof typeof LEGACY_APP_STORAGE_KEYS

const MIGRATABLE_STORAGE_KEYS: MigratableStorageKey[] = [
  'theme',
  'zoom',
  'viewMode',
  'tagColors',
  'statusColors',
  'propertyModes',
  'configMigrationFlag',
  'sortPreferences',
  'sidebarCollapsed',
  'layoutPanels',
  'welcomeDismissed',
]

export function copyLegacyAppStorageKeys(): void {
  try {
    if (localStorage.getItem(APP_STORAGE_KEYS.legacyMigrationFlag) === '1') return

    for (const key of MIGRATABLE_STORAGE_KEYS) {
      const storageKey = Reflect.get(APP_STORAGE_KEYS, key) as string
      const legacyStorageKey = Reflect.get(LEGACY_APP_STORAGE_KEYS, key) as string
      if (localStorage.getItem(storageKey) !== null) continue

      const legacyValue = localStorage.getItem(legacyStorageKey)
      if (legacyValue !== null) {
        localStorage.setItem(storageKey, legacyValue)
      }
    }

    localStorage.setItem(APP_STORAGE_KEYS.legacyMigrationFlag, '1')
  } catch {
    // Ignore unavailable or restricted localStorage implementations.
  }
}

export function getAppStorageItem(key: MigratableStorageKey): string | null {
  try {
    const storageKey = Reflect.get(APP_STORAGE_KEYS, key) as string
    const legacyStorageKey = Reflect.get(LEGACY_APP_STORAGE_KEYS, key) as string
    return localStorage.getItem(storageKey) ?? localStorage.getItem(legacyStorageKey)
  } catch {
    return null
  }
}
