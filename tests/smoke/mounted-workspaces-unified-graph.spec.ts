import { test, expect, type Page } from '@playwright/test'
import { sendShortcut } from './helpers'

const PRIMARY_VAULT_PATH = '/Users/mock/Field Notes'
const RESEARCH_VAULT_PATH = '/Users/mock/Research Lab'
const GETTING_STARTED_PATH = '/Users/mock/Getting Started'
const QUICK_OPEN_INPUT = 'input[placeholder="Search notes..."]'
const GLOBAL_SEARCH_INPUT = 'input[placeholder="Search in all notes..."]'
const SEARCH_NEEDLE = 'crossgraph-needle'

interface MockEntry {
  path: string
  filename: string
  title: string
  isA: string
  aliases: string[]
  belongsTo: string[]
  relatedTo: string[]
  status: string | null
  archived: boolean
  modifiedAt: number | null
  createdAt: number | null
  fileSize: number
  snippet: string
  wordCount: number
  relationships: Record<string, string[]>
  outgoingLinks: string[]
  properties: Record<string, unknown>
  template: null
  sort: null
}

interface SearchResultData {
  title: string
  path: string
  snippet: string
  score: number
  note_type: string | null
}

type MockHandler = (args?: Record<string, unknown>) => unknown
type MockWindow = Window & {
  __mockHandlers?: Record<string, MockHandler>
  __mountedWorkspaceSearchCalls?: string[]
}
interface MountedWorkspaceInitData {
  contentByPathData: Record<string, string>
  entriesByVaultData: Record<string, MockEntry[]>
  gettingStartedPath: string
  primaryVaultPath: string
  researchVaultPath: string
}

function createMockEntry(vaultPath: string, filename: string, title: string, snippet: string): MockEntry {
  return {
    path: `${vaultPath}/${filename}`,
    filename,
    title,
    isA: 'Note',
    aliases: [],
    belongsTo: [],
    relatedTo: [],
    status: null,
    archived: false,
    modifiedAt: 1_700_000_000,
    createdAt: null,
    fileSize: 256,
    snippet,
    wordCount: 12,
    relationships: {},
    outgoingLinks: [],
    properties: {},
    template: null,
    sort: null,
  }
}

async function installMountedWorkspaceMocks(page: Page): Promise<void> {
  await page.route('**/api/vault/**', (route) => route.abort())

  const primaryEntry = createMockEntry(
    PRIMARY_VAULT_PATH,
    'primary-signal.md',
    'Primary Signal',
    `Primary workspace has ${SEARCH_NEEDLE}`,
  )
  const researchEntry = createMockEntry(
    RESEARCH_VAULT_PATH,
    'research-beacon.md',
    'Research Beacon',
    `Research workspace has ${SEARCH_NEEDLE}`,
  )
  const entriesByVault: Record<string, MockEntry[]> = {
    [PRIMARY_VAULT_PATH]: [primaryEntry],
    [RESEARCH_VAULT_PATH]: [researchEntry],
  }
  const contentByPath = Object.fromEntries(
    [primaryEntry, researchEntry].map((entry) => [entry.path, `# ${entry.title}\n\n${entry.snippet}`]),
  )

  await page.addInitScript((data: MountedWorkspaceInitData) => {
    localStorage.clear()
    localStorage.setItem('nabu:claude-code-onboarding-dismissed', '1')

    const mockWindow = window as MockWindow
    mockWindow.__mountedWorkspaceSearchCalls = []
    let ref: Record<string, MockHandler> | undefined
    const entriesByVaultData = data.entriesByVaultData
    const contentByPathData = data.contentByPathData

    Object.defineProperty(window, '__mockHandlers', {
      configurable: true,
      set(value: Record<string, MockHandler>) {
        ref = value
        ref.load_vault_list = () => ({
          vaults: [
            {
              label: 'Field Notes',
              path: data.primaryVaultPath,
              alias: 'field',
              color: 'blue',
              icon: null,
              mounted: true,
            },
            {
              label: 'Research Lab',
              path: data.researchVaultPath,
              alias: 'lab',
              color: 'green',
              icon: null,
              mounted: true,
            },
          ],
          active_vault: data.primaryVaultPath,
          default_workspace_path: data.researchVaultPath,
          hidden_defaults: [],
        })
        ref.save_vault_list = () => null
        ref.get_settings = () => ({ multi_workspace_enabled: true })
        ref.get_default_vault_path = () => data.gettingStartedPath
        ref.check_vault_exists = (args?: Record<string, unknown>) =>
          args?.path === data.primaryVaultPath || args?.path === data.researchVaultPath
        ref.list_vault = (args?: Record<string, unknown>) => entriesByVaultData[String(args?.path ?? '')] ?? []
        ref.reload_vault = (args?: Record<string, unknown>) => entriesByVaultData[String(args?.path ?? '')] ?? []
        ref.list_vault_folders = () => []
        ref.list_views = () => []
        ref.get_all_content = () => contentByPathData
        ref.get_note_content = (args?: Record<string, unknown>) => contentByPathData[String(args?.path ?? '')] ?? ''
        ref.get_modified_files = () => []
        ref.get_file_history = () => []
        ref.search_vault = (args?: Record<string, unknown>) => {
          const vaultPath = String(args?.vaultPath ?? '')
          mockWindow.__mountedWorkspaceSearchCalls?.push(vaultPath)
          const query = String(args?.query ?? '').toLowerCase()
          const results = (entriesByVaultData[vaultPath] ?? [])
            .filter((entry) => `${entry.title} ${entry.snippet}`.toLowerCase().includes(query))
            .map((entry): SearchResultData => ({
              title: entry.title,
              path: entry.path,
              snippet: entry.snippet,
              score: 1,
              note_type: entry.isA,
            }))
          return { results, elapsed_ms: 1 }
        }
      },
      get() {
        return ref
      },
    })
  }, {
    contentByPathData: contentByPath,
    entriesByVaultData: entriesByVault,
    gettingStartedPath: GETTING_STARTED_PATH,
    primaryVaultPath: PRIMARY_VAULT_PATH,
    researchVaultPath: RESEARCH_VAULT_PATH,
  })
}

async function openQuickOpen(page: Page): Promise<void> {
  await page.locator('body').click()
  await sendShortcut(page, 'p', ['Control'])
  await expect(page.locator(QUICK_OPEN_INPUT)).toBeVisible()
}

async function openGlobalSearch(page: Page): Promise<void> {
  await page.locator('body').click()
  await sendShortcut(page, 'f', ['Control', 'Shift'])
  await expect(page.locator(GLOBAL_SEARCH_INPUT)).toBeVisible()
}

test('mounted workspaces share the graph across note list, quick open, and search @smoke', async ({ page }) => {
  await installMountedWorkspaceMocks(page)

  await page.goto('/', { waitUntil: 'domcontentloaded' })

  await page.getByText('All Notes', { exact: true }).click()

  const noteList = page.getByTestId('note-list-container')
  await expect(noteList.getByText('Primary Signal').first()).toBeVisible()
  await expect(noteList.getByText('Research Beacon').first()).toBeVisible()
  await expect(noteList.getByText('FN').first()).toBeVisible()
  await expect(noteList.getByText('RL').first()).toBeVisible()

  await openQuickOpen(page)
  await page.locator(QUICK_OPEN_INPUT).fill('Beacon')
  const quickOpen = page.getByTestId('quick-open-palette')
  await expect(quickOpen.getByText('Research Beacon')).toBeVisible()
  await expect(quickOpen.getByTestId('note-search-workspace-badge').filter({ hasText: 'RL' })).toBeVisible()
  await page.keyboard.press('Escape')

  await openGlobalSearch(page)
  await page.locator(GLOBAL_SEARCH_INPUT).fill(SEARCH_NEEDLE)
  await expect(page.getByText('Primary Signal')).toBeVisible({ timeout: 5_000 })
  await expect(page.getByText('Research Beacon')).toBeVisible()
  await expect(page.getByTestId('search-result-workspace-badge').filter({ hasText: 'FN' })).toBeVisible()
  await expect(page.getByTestId('search-result-workspace-badge').filter({ hasText: 'RL' })).toBeVisible()

  const searchCalls = await page.evaluate(() => (window as MockWindow).__mountedWorkspaceSearchCalls ?? [])
  expect(new Set(searchCalls)).toEqual(new Set([PRIMARY_VAULT_PATH, RESEARCH_VAULT_PATH]))
})
