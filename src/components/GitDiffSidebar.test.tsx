import { describe, expect, it, vi } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { GitDiffSidebar } from './GitDiffSidebar'
import type { ModifiedFile } from '../types'

vi.mock('../lib/telemetry', () => ({
  trackEvent: vi.fn(),
}))

const CHANGED_FILES: ModifiedFile[] = [
  {
    path: '/vault/notes/alpha.md',
    relativePath: 'notes/alpha.md',
    status: 'modified',
    addedLines: 2,
    deletedLines: 1,
  },
  {
    path: '/vault/notes/beta.md',
    relativePath: 'notes/beta.md',
    status: 'added',
    addedLines: 10,
    deletedLines: 0,
  },
  {
    path: '/vault/journal/2026.md',
    relativePath: 'journal/2026.md',
    status: 'modified',
    addedLines: 1,
    deletedLines: 3,
  },
]

const DIFF_TEXT = [
  'diff --git a/notes/alpha.md b/notes/alpha.md',
  'index 111..222 100644',
  '--- a/notes/alpha.md',
  '+++ b/notes/alpha.md',
  '@@ -1,2 +1,3 @@',
  '-old line',
  '+new line',
].join('\n')

function setup(overrides: Partial<Parameters<typeof GitDiffSidebar>[0]> = {}) {
  const onFileSelect = vi.fn().mockResolvedValue(DIFF_TEXT)
  const onCommit = vi.fn().mockResolvedValue(undefined)
  const props = {
    vaultPath: '/vault',
    changedFiles: CHANGED_FILES,
    selectedFile: null as string | null,
    diff: null as string | null,
    diffLoading: false,
    onFileSelect,
    onCommit,
    remoteConfigured: true,
    locale: 'en' as const,
    ...overrides,
  }
  render(<GitDiffSidebar {...props} />)
  return { onFileSelect, onCommit }
}

describe('GitDiffSidebar', () => {
  it('shows only folders that contain changed files with per-folder counts', () => {
    setup()
    expect(screen.getByTestId('git-diff-sidebar')).toBeInTheDocument()
    expect(screen.getByText('notes')).toBeInTheDocument()
    expect(screen.getByText('journal')).toBeInTheDocument()
    expect(screen.getByText('alpha.md')).toBeInTheDocument()
    expect(screen.getByTestId('git-diff-file-count').textContent).toBe('3')
  })

  it('hides unchanged folders and files', () => {
    setup()
    expect(screen.queryByText('attachments')).not.toBeInTheDocument()
    expect(screen.queryByText('unrelated.md')).not.toBeInTheDocument()
  })

  it('loads the diff when a changed file is clicked', async () => {
    const { onFileSelect } = setup()
    fireEvent.click(screen.getByText('alpha.md'))
    await waitFor(() => {
      expect(onFileSelect).toHaveBeenCalledWith('/vault/notes/alpha.md')
    })
    expect(await screen.findByTestId('git-diff-view')).toBeInTheDocument()
  })

  it('disables commit when the message is empty', () => {
    const { onCommit } = setup()
    const commitButton = screen.getByTestId('git-diff-commit-btn') as HTMLButtonElement
    expect(commitButton).toBeDisabled()
    fireEvent.click(commitButton)
    expect(onCommit).not.toHaveBeenCalled()
  })

  it('commits with the entered message', async () => {
    const { onCommit } = setup()
    fireEvent.change(screen.getByTestId('git-diff-commit-message'), {
      target: { value: 'Update notes' },
    })
    fireEvent.click(screen.getByTestId('git-diff-commit-btn'))
    await waitFor(() => {
      expect(onCommit).toHaveBeenCalledWith('Update notes', false)
    })
  })

  it('offers commit and push when a remote is configured', () => {
    setup()
    expect(screen.getByTestId('git-diff-commit-push-btn')).toBeInTheDocument()
  })

  it('hides commit-and-push without a remote', () => {
    setup({ remoteConfigured: false })
    expect(screen.queryByTestId('git-diff-commit-push-btn')).not.toBeInTheDocument()
  })

  it('filters changed files with the search input', () => {
    setup()
    fireEvent.change(screen.getByTestId('git-diff-search'), { target: { value: 'alpha' } })
    expect(screen.getByText('alpha.md')).toBeInTheDocument()
    expect(screen.queryByText('beta.md')).not.toBeInTheDocument()
  })
})
