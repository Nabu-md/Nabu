import type { ContextSearchResult } from '@shared/schemas'

// Reveal the type of ContextSearchResult
const x: ContextSearchResult = null as any
// @ts-expect-error reveal
const reveal: { __impossible: true } = x
