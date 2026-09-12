import { fireEvent, render, screen } from '@testing-library/react'
import type { ComponentProps } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { TooltipProvider } from '@/components/ui/tooltip'
import { SidebarTitleBar } from './SidebarSections'

function renderTitleBar(overrides: Partial<ComponentProps<typeof SidebarTitleBar>> = {}) {
  return render(<SidebarTitleBar {...overrides} />, { wrapper: TooltipProvider })
}

describe('SidebarTitleBar', () => {
  it('renders sidebar collapse and search controls', () => {
    const onCollapse = vi.fn()
    const onSearchChange = vi.fn()

    renderTitleBar({
      onCollapse,
      onSearchChange,
      search: 'test',
    })

    const collapse = screen.getByRole('button', { name: 'Collapse sidebar' })
    expect(collapse).toHaveAttribute('title', expect.stringMatching(/^Collapse sidebar \((⌘|Ctrl\+)2\)$/))

    const searchBtn = screen.getByRole('button', { name: 'Search notes' })
    expect(searchBtn).toBeInTheDocument()

    fireEvent.click(collapse)
    fireEvent.click(searchBtn)

    expect(onCollapse).toHaveBeenCalledTimes(1)
    expect(onSearchChange).not.toHaveBeenCalled()
  })

  it('hides search input when search is cleared', () => {
    const onSearchChange = vi.fn()

    renderTitleBar({
      onSearchChange,
      search: 'test',
    })

    const searchBtn = screen.getByRole('button', { name: 'Search notes' })
    fireEvent.click(searchBtn)

    const input = screen.getByPlaceholderText('Search notes...')
    expect(input).toHaveValue('test')

    const clearSearch = screen.getByRole('button', { name: 'Clear search' })
    fireEvent.click(clearSearch)

    expect(onSearchChange).toHaveBeenCalledWith('')
  })

  it('omits controls when sidebar callbacks are absent', () => {
    renderTitleBar()

    expect(screen.queryByRole('button', { name: 'Collapse sidebar' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Go Back' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Go Forward' })).not.toBeInTheDocument()
  })

  it('uses the fullscreen-aware macOS traffic-light inset', () => {
    const { container } = renderTitleBar()

    expect(container.firstElementChild).toHaveStyle({
      paddingLeft: 'var(--nabu-macos-traffic-light-padding, 90px)',
    })
  })
})
