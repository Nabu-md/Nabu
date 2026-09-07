import type { ComponentType } from 'react'
import type { IconProps } from '@phosphor-icons/react'
import { cn } from '../lib/utils'

/** Label + icon metadata for a sidebar group of entries sharing a type. */
export interface SectionGroup {
  label: string
  type: string
  Icon: ComponentType<IconProps>
  customColor?: string | null
}

const SIDEBAR_COUNT_PILL_STYLE = {
  borderRadius: 9999,
  padding: '0 6px',
  fontSize: 10,
  fontVariantNumeric: 'tabular-nums',
} as const

export function SidebarCountPill({
  count,
  className,
  style,
  compact,
  testId = 'sidebar-count-chip',
}: {
  count: number
  className?: string
  style?: React.CSSProperties
  compact?: boolean
  testId?: string
}) {
  return (
    <span
      data-testid={testId}
      className={cn('flex items-center justify-center', className)}
      style={{
        height: compact ? 18 : 20,
        ...SIDEBAR_COUNT_PILL_STYLE,
        ...style,
      }}
    >
      {count}
    </span>
  )
}
