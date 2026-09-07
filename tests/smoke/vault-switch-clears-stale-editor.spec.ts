import { test, expect, type Page } from '@playwright/test'
import { executeCommand, openCommandPalette } from './helpers'

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
  modifiedAt: number
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

interface SaveCall {
  path: string
  content: string
  activeVaultPath: string
}

interface SmokeVaultData {
  contentByPath: Record<string, string>
  entriesByVault: Record<string, MockEntry[]>
  personalVaultPath: string
  workVaultPath: string
}

interface CodeMirrorView {
  state: { doc: { length: number } }
  dispatch: (transaction: { changes: { from: number; to: number; insert: string } }) => void
}

interface CodeMirrorElement extends Element {
  cmTile?: { view?: CodeMirrorView }
}

type MockHandler = (args?: Record<string, unknown>) => unknown

function makeEntry(vaultPath: string, filename: string, title: string): MockEntry {
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
    fileSize: 128,
    snippet: `${title} body`,
    wordCount: 4,
    relationships: {},
    outgoingLinks: [],
    properties: {},
    template: null,
    sort: null,
  }
}

function buildSmokeVaultData(): SmokeVaultData {
  const workVaultPath = '/Users/mock/Work'
  const personalVaultPath = '/Users/mock/Personal'
  const workEntry = makeEntry(workVaultPath, 'work-home.md', 'Work Home')
  const personalEntry = makeEntry(personalVaultPath, 'personal-home.md', 'Personal Home')

  return {
    workVaultPath,
    personalVaultPath,
    entriesByVault: {
      [workVaultPath]: [workEntry],
      [personalVaultPath]: [personalEntry],
    },
    contentByPath: {
      [workEntry.path]: '# Work Home\n\nWork body',
      [personalEntry.path]: '# Personal Home\n\nPersonal body',
    },
  }
}

async function installVaultSwitchMocks(page: Page): Promise<SmokeVaultData> {
  const data = buildSmokeVaultData()
  await page.addInitScript((data: SmokeVaultData) => {
    localStorage.clear()
    localStorage.setItem('nabu_welcome_dismissed', '1')
    localStorage.setItem('nabu:claude-code-onboarding-dismissed', '1')

    let activeVaultPath = data.workVaultPath
    let handlers: Record<string, MockHandler> | null = null
    const saveCalls: SaveCall[] = []
    const contentByPath = { ...data.contentByPath }
    const vaultPaths = new Set([data.workVaultPath, data.personalVaultPath])
    const smokeWindow = window as Window & { __staleVaultSaveProbe?: SaveCall[] }
    smokeWindow.__staleVaultSaveProbe = saveCalls

    Object.defineProperty(window, '__mockHandlers', {
      configurable: true,
      set(value: unknown) {
        handlers = value as Record<string, MockHandler>
        handlers.load_vault_list = () => ({
          vaults: [
            { label: 'Work Vault', path: data.workVaultPath },
            { label: 'Personal Vault', path: data.personalVaultPath },
          ],
          active_vault: activeVaultPath,
          hidden_defaults: [],
        })
        handlers.save_vault_list = (args = {}) => {
          const list = args.list as { active_vault?: unknown }
          if (typeof list.active_vault === 'string') activeVaultPath = list.active_vault
          return null
        }
        handlers.check_vault_exists = (args = {}) => vaultPaths.has(String(args.path))
        handlers.get_default_vault_path = () => data.workVaultPath
        handlers.list_vault = (args = {}) => data.entriesByVault[String(args.path)] ?? []
        handlers.reload_vault = handlers.list_vault
        handlers.list_vault_folders = () => []
        handlers.list_views = () => []
        handlers.get_note_content = (args = {}) => contentByPath[String(args.path)] ?? ''
        handlers.validate_note_content = () => true
        handlers.save_note_content = (args = {}) => {
          const path = String(args.path)
          const content = String(args.content)
          saveCalls.push({ path, content, activeVaultPath })
          contentByPath[path] = content
          return null
        }
        handlers.get_modified_files = () => []
        handlers.get_file_history = () => []
        handlers.is_git_repo = () => true
        handlers.sync_mcp_bridge_vault = () => null
      },
      get() {
        return handlers
      },
    })
  }, data)
  return data
}

async function openNote(page: Page, title: string): Promise<void> {
  await page.getByTestId('note-list-container').getByText(title, { exact: true }).click()
}

async function openRawMode(page: Page): Promise<void> {
  await openCommandPalette(page)
  await executeCommand(page, 'Toggle Raw')
  await expect(page.locator('.cm-content')).toBeVisible({ timeout: 5_000 })
}

async function setRawEditorContent(page: Page, content: string): Promise<void> {
  await page.evaluate((nextContent) => {
    const element = document.querySelector('.cm-content') as CodeMirrorElement | null
    const view = element?.cmTile?.view
    if (!view) throw new Error('CodeMirror view is missing')

    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: nextContent },
    })
  }, content)
}

async function readSaveProbe(page: Page): Promise<SaveCall[]> {
  return page.evaluate(() => {
    const smokeWindow = window as Window & { __staleVaultSaveProbe?: SaveCall[] }
    return smokeWindow.__staleVaultSaveProbe ?? []
  })
}

async function clearSaveProbe(page: Page): Promise<void> {
  await page.evaluate(() => {
    const smokeWindow = window as Window & { __staleVaultSaveProbe?: SaveCall[] }
    if (smokeWindow.__staleVaultSaveProbe) smokeWindow.__staleVaultSaveProbe.length = 0
  })
}

async function switchToPersonalVault(page: Page): Promise<void> {
  await page.getByTestId('status-vault-trigger').click()
  await page.getByTestId('vault-menu-item-Personal Vault').click()
}

test('switching vaults clears stale pending editor saves @smoke', async ({ page }) => {
  const data = await installVaultSwitchMocks(page)
  await page.goto('/', { waitUntil: 'domcontentloaded' })

  await openNote(page, 'Work Home')
  await openRawMode(page)
  await setRawEditorContent(page, '# Work Home\n\nUnsaved draft before vault switch')

  await switchToPersonalVault(page)
  await expect(page.getByTestId('note-list-container').getByText('Personal Home', { exact: true })).toBeVisible()
  expect(await readSaveProbe(page)).not.toEqual(expect.arrayContaining([
    expect.objectContaining({
      activeVaultPath: data.personalVaultPath,
      path: `${data.workVaultPath}/work-home.md`,
    }),
  ]))

  await clearSaveProbe(page)
  await page.waitForTimeout(1_800)
  expect(await readSaveProbe(page)).toEqual([])
})
