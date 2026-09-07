import { test, expect, type Page } from '@playwright/test'

interface StaleStarterPaths {
  gettingStartedPath: string
  personalVaultPath: string
  workVaultPath: string
}

function buildPaths(): StaleStarterPaths {
  return {
    gettingStartedPath: '/Users/mock/Documents/Getting Started',
    personalVaultPath: '/Users/mock/Personal',
    workVaultPath: '/Users/mock/Work',
  }
}

async function installStaleStarterMocks(page: Page) {
  const paths = buildPaths()

  await page.addInitScript((data: StaleStarterPaths) => {
    localStorage.clear()
    localStorage.setItem('nabu:claude-code-onboarding-dismissed', '1')

    let ref: Record<string, unknown> | null = null

    Object.defineProperty(window, '__mockHandlers', {
      configurable: true,
      set(value) {
        ref = value as Record<string, unknown>
        ref.load_vault_list = () => ({
          vaults: [
            { label: 'Getting Started', path: data.gettingStartedPath },
            { label: 'Work Vault', path: data.workVaultPath },
            { label: 'Personal Vault', path: data.personalVaultPath },
          ],
          active_vault: data.workVaultPath,
          hidden_defaults: [],
        })
        ref.get_default_vault_path = () => data.gettingStartedPath
        ref.check_vault_exists = (args: { path?: string }) =>
          args?.path === data.workVaultPath || args?.path === data.personalVaultPath
        ref.list_vault = (args: { path?: string }) => {
          if (args?.path === data.workVaultPath) {
            return [{
              path: `${data.workVaultPath}/work-home.md`,
              filename: 'work-home.md',
              title: 'Work Home',
              isA: 'Note',
              aliases: [],
              belongsTo: [],
              relatedTo: [],
              status: null,
              archived: false,
              modifiedAt: 1700000000,
              createdAt: null,
              fileSize: 256,
              snippet: 'Work Home snippet',
              wordCount: 12,
              relationships: {},
              outgoingLinks: [],
              properties: {},
              template: null,
              sort: null,
            }]
          }

          if (args?.path === data.personalVaultPath) {
            return [{
              path: `${data.personalVaultPath}/personal-home.md`,
              filename: 'personal-home.md',
              title: 'Personal Home',
              isA: 'Note',
              aliases: [],
              belongsTo: [],
              relatedTo: [],
              status: null,
              archived: false,
              modifiedAt: 1700000000,
              createdAt: null,
              fileSize: 256,
              snippet: 'Personal Home snippet',
              wordCount: 12,
              relationships: {},
              outgoingLinks: [],
              properties: {},
              template: null,
              sort: null,
            }]
          }

          return []
        }
        ref.list_vault_folders = () => []
        ref.list_views = () => []
        ref.get_all_content = () => ({
          [`${data.workVaultPath}/work-home.md`]: '# Work Home\n\nWork Home snippet',
          [`${data.personalVaultPath}/personal-home.md`]: '# Personal Home\n\nPersonal Home snippet',
        })
        ref.get_note_content = (args: { path?: string }) => {
          if (args?.path === `${data.workVaultPath}/work-home.md`) {
            return '# Work Home\n\nWork Home snippet'
          }

          if (args?.path === `${data.personalVaultPath}/personal-home.md`) {
            return '# Personal Home\n\nPersonal Home snippet'
          }

          return ''
        }
        ref.get_modified_files = () => []
        ref.get_file_history = () => []
      },
      get() {
        return ref
      },
    })
  }, paths)
}

test('stale persisted Getting Started entries stay hidden when the starter path is missing @smoke', async ({ page }) => {
  await installStaleStarterMocks(page)

  await page.goto('/', { waitUntil: 'domcontentloaded' })

  const trigger = page.getByTestId('status-vault-trigger')
  await expect(trigger).toContainText('Work Vault')

  await trigger.focus()
  await expect(trigger).toBeFocused()
  await page.keyboard.press('Enter')

  await expect(page.getByTestId('vault-menu-item-Getting Started')).toHaveCount(0)
  await expect(page.getByTestId('vault-menu-item-Work Vault')).toBeVisible()
  await expect(page.getByTestId('vault-menu-item-Personal Vault')).toBeVisible()
})
