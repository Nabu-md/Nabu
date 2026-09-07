import { defineConfig } from '@playwright/test'

const baseURL = process.env.BASE_URL || 'http://localhost:5201'
const claudeCodeOnboardingStorageState = {
  cookies: [],
  origins: [
    {
      origin: baseURL,
      localStorage: [
        { name: 'nabu:claude-code-onboarding-dismissed', value: '1' },
      ],
    },
  ],
}

export default defineConfig({
  testDir: './tests/smoke',
  timeout: 20_000,
  retries: 2,
  workers: 1,
  use: {
    baseURL,
    headless: true,
    storageState: claudeCodeOnboardingStorageState,
  },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
  webServer: {
    command: `pnpm dev --port ${process.env.BASE_URL?.match(/:(\d+)/)?.[1] || '5201'}`,
    url: baseURL,
    reuseExistingServer: true,
  },
})
