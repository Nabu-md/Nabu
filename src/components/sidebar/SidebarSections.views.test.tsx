import { act, render } from '@testing-library/react'
import type { ReactNode } from 'react'
import { describe, expect, it, vi } from 'vitest'
import type { ViewFile } from '../../types'
import { ViewsSection } from './SidebarSections'

let dragEnd: ((event: { active: { id: string }; over: { id: string } }) => void) | undefined
let sortableItems: string[] = []

vi.mock('@dnd-kit/core', () => ({
  closestCenter: vi.fn(),
  DndContext: ({ children, onDragEnd }: { children: ReactNode; onDragEnd: typeof dragEnd }) => {
    dragEnd = onDragEnd
    return children
  },
  useSensors: vi.fn(() => []),
}))

vi.mock('@dnd-kit/sortable', () => ({
  SortableContext: ({ children, items }: { children: ReactNode; items: string[] }) => {
    sortableItems = items
    return children
  },
  sortableKeyboardCoordinates: vi.fn(),
  useSortable: vi.fn(() => ({
    attributes: {},
    isDragging: false,
    listeners: {},
    setNodeRef: vi.fn(),
    transform: null,
    transition: undefined,
  })),
  verticalListSortingStrategy: vi.fn(),
}))

const views: ViewFile[] = [
  {
    filename: 'first.yml',
    definition: {
      name: 'First',
      icon: null,
      color: null,
      order: 0,
      sort: null,
      filters: { all: [] },
    },
  },
  {
    filename: 'second.yml',
    definition: {
      name: 'Second',
      icon: null,
      color: null,
      order: 1,
      sort: null,
      filters: { all: [] },
    },
  },
]

describe('ViewsSection ordering', () => {
  it('reports plain filenames after a saved View is dropped', () => {
    const onReorderViews = vi.fn()
    render(
      <ViewsSection
        views={views}
        selection={{ kind: 'filter', filter: 'all' }}
        onSelect={vi.fn()}
        collapsed={false}
        onToggle={vi.fn()}
        onReorderViews={onReorderViews}
        sensors={[]}
        entries={[]}
      />,
    )

    act(() => {
      dragEnd?.({ active: { id: sortableItems[0] }, over: { id: sortableItems[1] } })
    })

    expect(onReorderViews).toHaveBeenCalledWith(['second.yml', 'first.yml'])
  })
})
