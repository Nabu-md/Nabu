import { test, expect, type Page } from '@playwright/test'
import { findCommand, openCommandPalette, sendShortcut } from './helpers'

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

interface VaultSeed {
  label: string
  path: string
  noteTitle?: string
}

interface EntryOptions {
  fileName?: string
  isA?: string
  snippet?: string
}

function untitledNoteRow(page: Page) {
  return page.getByText(/^Untitled Note(?: \d+)?$/i).first()
}

async function expectFreshVaultSeedEntries(page: Page) {
  await expect(page.getByText('AGENTS.md — Nabu Vault', { exact: true })).toBeVisible()
  await expect(page.getByText('CLAUDE.md', { exact: true })).toBeVisible()
  await expect(page.getByText('Config', { exact: true })).toHaveCount(0)
}

async function installEmptyVaultMocks(
  page: Page,
  config: {
    createdVaultPath: string
    initialVaults: VaultSeed[]
    activeVault: string | null
  },
) {
  await page.addInitScript((mockConfig) => {
    const gettingStartedPath = '/Users/mock/Documents/Getting Started'

    function buildEntry(vaultPath: string, title: string, options: EntryOptions = {}): MockEntry {
      const defaultStem = title.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/(^-|-$)/g, '')
      const filename = options.fileName ?? `${defaultStem}.md`
      return {
        path: `${vaultPath}/${filename}`,
        filename,
        title,
        isA: options.isA ?? 'Note',
        aliases: [],
        belongsTo: [],
        relatedTo: [],
        status: null,
        archived: false,
        modifiedAt: 1700000000,
        createdAt: null,
        fileSize: 256,
        snippet: options.snippet ?? `${title} snippet`,
        wordCount: 12,
        relationships: {},
        outgoingLinks: [],
        properties: {},
        template: null,
        sort: null,
      }
    }

    function buildFreshVaultSeedEntries(vaultPath: string): MockEntry[] {
      return [
        buildEntry(vaultPath, 'AGENTS.md — Nabu Vault', { fileName: 'AGENTS.md' }),
        buildEntry(vaultPath, 'CLAUDE.md', { fileName: 'CLAUDE.md' }),
        buildEntry(vaultPath, 'Type', { fileName: 'type.md', isA: 'Type' }),
        buildEntry(vaultPath, 'Note', { fileName: 'note.md', isA: 'Type' }),
      ]
    }

    function syncEntryContent(entry: MockEntry) {
      allContent[entry.path] = `# ${entry.title}\n\n${entry.snippet}`
    }

    let savedVaults = mockConfig.initialVaults.map(({ label, path }) => ({ label, path }))
    let activeVault = mockConfig.activeVault
    let hiddenDefaults: string[] = []

    const entriesByVault = Object.fromEntries(
      mockConfig.initialVaults.map((vault) => [
        vault.path,
        vault.noteTitle ? [buildEntry(vault.path, vault.noteTitle)] : [],
      ]),
    ) satisfies Record<string, MockEntry[]>

    const allContent = Object.fromEntries(
      Object.values(entriesByVault)
        .flat()
        .map((entry) => [entry.path, `# ${entry.title}\n\n${entry.snippet}`]),
    )

    localStorage.clear()
    localStorage.setItem('nabu:claude-code-onboarding-dismissed', '1')

    Object.defineProperty(window, 'prompt', {
      configurable: true,
      value: () => mockConfig.createdVaultPath,
    })

    let ref: Record<string, unknown> | null = null

    Object.defineProperty(window, '__mockHandlers', {
      configurable: true,
      set(value) {
        ref = value as Record<string, unknown>
        ref.load_vault_list = () => ({
          vaults: [...savedVaults],
          active_vault: activeVault,
          hidden_defaults: [...hiddenDefaults],
        })
        ref.save_vault_list = (args: {
          list: {
            vaults: Array<{ label: string; path: string }>
            active_vault: string | null
            hidden_defaults?: string[]
          }
        }) => {
          savedVaults = [...args.list.vaults]
          activeVault = args.list.active_vault
          hiddenDefaults = [...(args.list.hidden_defaults ?? [])]
          return null
        }
        ref.get_default_vault_path = () => gettingStartedPath
        ref.check_vault_exists = (args: { path?: string }) =>
          savedVaults.some((vault) => vault.path === args.path)
          || args.path === mockConfig.createdVaultPath
        ref.create_empty_vault = (args: { targetPath?: string | null }) => {
          if (args.targetPath !== mockConfig.createdVaultPath) {
            throw new Error(`Unexpected empty vault target: ${args.targetPath}`)
          }
          entriesByVault[mockConfig.createdVaultPath] = buildFreshVaultSeedEntries(mockConfig.createdVaultPath)
          entriesByVault[mockConfig.createdVaultPath].forEach(syncEntryContent)
          return mockConfig.createdVaultPath
        }
        ref.list_vault = (args: { path?: string }) => entriesByVault[args.path ?? activeVault ?? ''] ?? []
        ref.list_vault_folders = () => []
        ref.list_views = () => []
        ref.get_all_content = () => allContent
        ref.get_note_content = (args: { path?: string }) => allContent[args.path ?? ''] ?? ''
        ref.get_modified_files = () => []
        ref.get_file_history = () => []
      },
      get() {
        return ref
      },
    })
  }, config)
}

test('keyboard onboarding can create an empty vault and the first note', async ({ page }) => {
  await installEmptyVaultMocks(page, {
    createdVaultPath: '/Users/mock/Documents/Fresh Vault',
    initialVaults: [],
    activeVault: null,
  })

  await page.goto('/', { waitUntil: 'domcontentloaded' })

  await expect(page.getByTestId('welcome-screen')).toBeVisible()
  await expect(page.getByTestId('welcome-create-new')).toContainText('Create empty vault')
  await expect(page.getByTestId('welcome-create-vault')).toBeFocused()

  await page.keyboard.press('Tab')
  await expect(page.getByTestId('welcome-create-new')).toBeFocused()
  await page.keyboard.press('Enter')

  await expect(page.getByTestId('note-list-container')).toBeVisible({ timeout: 5_000 })
  await expectFreshVaultSeedEntries(page)
  await sendShortcut(page, 'n', ['Control'])
  await expect(untitledNoteRow(page)).toBeVisible({ timeout: 5_000 })
})

test('command palette and bottom bar expose empty-vault creation from the active app shell', async ({ page }) => {
  await installEmptyVaultMocks(page, {
    createdVaultPath: '/Users/mock/Documents/Client Vault',
    initialVaults: [{ label: 'Work Vault', path: '/Users/mock/Work', noteTitle: 'Work Home' }],
    activeVault: '/Users/mock/Work',
  })

  await page.goto('/', { waitUntil: 'domcontentloaded' })

  await expect(page.getByTestId('note-list-container')).toBeVisible({ timeout: 5_000 })
  await expect(page.getByTestId('status-vault-trigger')).toContainText('Work Vault')

  await openCommandPalette(page)
  expect(await findCommand(page, 'Create Empty Vault')).toBe(true)
  await page.keyboard.press('Escape')

  const trigger = page.getByTestId('status-vault-trigger')
  await trigger.focus()
  await expect(trigger).toBeFocused()
  await page.keyboard.press('Enter')

  const createEmptyItem = page.getByTestId('vault-menu-create-empty')
  await createEmptyItem.focus()
  await expect(createEmptyItem).toBeFocused()
  await page.keyboard.press('Enter')

  await expect(trigger).toContainText('Client Vault')
  await expectFreshVaultSeedEntries(page)
  await sendShortcut(page, 'n', ['Control'])
  await expect(untitledNoteRow(page)).toBeVisible({ timeout: 5_000 })
})
