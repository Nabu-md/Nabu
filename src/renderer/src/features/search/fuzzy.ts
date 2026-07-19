/**
 * fuzzy.ts
 *
 * Shared fuzzy-ranking utility for the Quick Switcher and Command Palette.
 * Matches a query string against item names, paths, and aliases with
 * weighted scoring: name > path > alias.
 *
 * All functions are pure and deterministic — same inputs always produce
 * the same ordering.
 *
 * Requirements: 4.2, 5.7
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** An item that can be fuzzy-matched. */
export interface FuzzyItem {
  /** Primary display name (e.g. file name without extension). */
  name: string
  /** Full or relative path (e.g. "folder/subfolder/note"). */
  path: string
  /** Optional frontmatter aliases. */
  aliases?: string[]
  /** Optional extra keywords for Command Palette items. */
  keywords?: string[]
}

/** A single contiguous character range for highlighting. */
export interface FuzzyRange {
  start: number
  end: number // exclusive
}

/** A fuzzy-match result against a single field (name, path, or alias). */
export interface FieldMatch {
  score: number
  ranges: FuzzyRange[]
}

/** A ranked fuzzy-match result. */
export interface FuzzyMatch<T extends FuzzyItem = FuzzyItem> {
  item: T
  /** Aggregate score (0–1). Higher is better. */
  score: number
  /** Character ranges to highlight in the best-matching field. */
  ranges: FuzzyRange[]
  /** Which field produced the best match: 'name' | 'path' | 'alias' | 'keyword'. */
  matchField: 'name' | 'path' | 'alias' | 'keyword'
}

export interface FuzzyOptions {
  /** Maximum results to return (0 = unlimited). Default: 0. */
  maxResults?: number
  /** Minimum score threshold (0–1). Results below this are excluded. Default: 0. */
  threshold?: number
}

// ---------------------------------------------------------------------------
// Constants — field weights (name > path > alias > keyword)
// ---------------------------------------------------------------------------

const NAME_WEIGHT = 3
const PATH_WEIGHT = 1
const ALIAS_WEIGHT = 0.8
const KEYWORD_WEIGHT = 0.6

// ---------------------------------------------------------------------------
// Core matching algorithm
// ---------------------------------------------------------------------------

/**
 * Compute a fuzzy match score and character ranges for `query` against
 * `target`. Returns `null` when the query characters do not appear in
 * `target` in order.
 *
 * Scoring factors (all normalised to 0–1, then summed):
 *   - **Sequential bonus**: consecutive matched characters score higher.
 *   - **Start-of-word bonus**: matches after a separator (`/`, `-`, `_`,
 *     `.`, ` `) or an uppercase letter within a camelCase word get a bonus.
 *   - **Leading bonus**: matches near the start of `target` score higher.
 *
 * The returned score is clamped to [0, 1].
 */
export function matchScore(query: string, target: string): FieldMatch | null {
  if (!query || !target) return null

  const q = query.toLowerCase()
  const t = target.toLowerCase()

  // Every query character must appear in order.
  let ti = 0
  const matchedIndices: number[] = []

  for (let qi = 0; qi < q.length; qi++) {
    const ch = q[qi]

    while (true) {
      if (ti >= t.length) return null // character not found
      if (t[ti] === ch) {
        matchedIndices.push(ti)
        ti++
        break
      }
      ti++
    }
  }

  // Compute score based on match quality.
  let score = 0
  const qLen = q.length

  // Constant for proximity normalisation — one "word unit" length.
  // Using a constant instead of target.length ensures shorter targets
  // are not penalised relative to longer ones for the same absolute index.
  const PROXIMITY_LENGTH = 40

  for (let i = 0; i < matchedIndices.length; i++) {
    const idx = matchedIndices[i]

    // Leading bonus: matches at the very start.
    if (idx === 0) {
      score += 0.3
    } else {
      // Proximity bonus: closer to the start is better.
      // Uses a fixed denominator so target length doesn't distort scores.
      score += 0.05 * Math.max(0, 1 - idx / PROXIMITY_LENGTH)
    }

    // Sequential bonus: this match is consecutive with the previous.
    if (i > 0 && idx === matchedIndices[i - 1] + 1) {
      score += 0.15
    }

    // Word-boundary bonus: preceded by a separator or uppercase letter.
    if (idx > 0) {
      const prevChar = target[idx - 1]
      if (isSeparator(prevChar)) {
        score += 0.2
      } else if (isUppercase(target[idx]) && isLowercase(prevChar)) {
        // camelCase boundary
        score += 0.15
      }
    }
  }

  // Normalise score by query length and clamp to [0, 1].
  score = Math.min(1, score / (qLen * 0.5 + 0.5))

  // Build match ranges from consecutive matched indices.
  const ranges: FuzzyRange[] = []
  let rangeStart = matchedIndices[0]
  let prev = matchedIndices[0]

  for (let i = 1; i < matchedIndices.length; i++) {
    if (matchedIndices[i] === prev + 1) {
      prev = matchedIndices[i]
    } else {
      ranges.push({ start: rangeStart, end: prev + 1 })
      rangeStart = matchedIndices[i]
      prev = matchedIndices[i]
    }
  }
  ranges.push({ start: rangeStart, end: prev + 1 })

  return { score, ranges }
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

/** Standard word-boundary separators. */
const SEPARATORS = new Set(['/', '-', '_', '.', ' ', '\\', '#', '[', ']', '(', ')'])

function isSeparator(ch: string): boolean {
  return SEPARATORS.has(ch)
}

function isUppercase(ch: string): boolean {
  return ch >= 'A' && ch <= 'Z'
}

function isLowercase(ch: string): boolean {
  return ch >= 'a' && ch <= 'z'
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Perform a fuzzy search across all items.
 *
 * For each item, fuzzy-matches the query against the item's `name`,
 * `path`, `aliases`, and `keywords` fields independently. The best score
 * across all fields is used as the item's final score, weighted by field
 * type (name > path > alias > keyword).
 *
 * Results are sorted by score descending. Ties are broken by name
 * alphabetically for deterministic ordering.
 *
 * @param query  The user-typed search string (case-insensitive).
 * @param items  The items to search through.
 * @param opts   Optional `maxResults` and `threshold`.
 * @returns Ranked results, best match first.
 */
export function fuzzySearch<T extends FuzzyItem>(
  query: string,
  items: T[],
  opts?: FuzzyOptions
): FuzzyMatch<T>[] {
  if (!query.trim()) return []

  const threshold = opts?.threshold ?? 0
  const maxResults = opts?.maxResults ?? 0

  const scored: FuzzyMatch<T>[] = []

  for (const item of items) {
    let best: FieldMatch | null = null
    let matchField: 'name' | 'path' | 'alias' | 'keyword' = 'name'

    // Helper to get current weighted score
    const getCurrentWeighted = (): number =>
      best !== null ? best.score * getFieldWeight(matchField) : -1

    // Check name (highest weight).
    const nameMatch = matchScore(query, item.name)
    if (nameMatch) {
      const weighted = nameMatch.score * NAME_WEIGHT
      if (weighted > getCurrentWeighted()) {
        best = nameMatch
        matchField = 'name'
      }
    }

    // Check path.
    const pathMatch = matchScore(query, item.path)
    if (pathMatch) {
      const weighted = pathMatch.score * PATH_WEIGHT
      if (weighted > getCurrentWeighted()) {
        best = pathMatch
        matchField = 'path'
      }
    }

    // Check aliases.
    if (item.aliases) {
      for (const alias of item.aliases) {
        const aliasMatch = matchScore(query, alias)
        if (aliasMatch) {
          const weighted = aliasMatch.score * ALIAS_WEIGHT
          if (weighted > getCurrentWeighted()) {
            best = aliasMatch
            matchField = 'alias'
          }
        }
      }
    }

    // Check keywords.
    if (item.keywords) {
      for (const kw of item.keywords) {
        const kwMatch = matchScore(query, kw)
        if (kwMatch) {
          const weighted = kwMatch.score * KEYWORD_WEIGHT
          if (weighted > getCurrentWeighted()) {
            best = kwMatch
            matchField = 'keyword'
          }
        }
      }
    }

    if (best !== null) {
      const finalScore = best.score * getFieldWeight(matchField)
      if (finalScore >= threshold) {
        scored.push({
          item,
          score: finalScore,
          ranges: best.ranges,
          matchField
        })
      }
    }
  }

  // Sort by score descending, then by name ascending for determinism.
  scored.sort((a, b) => {
    if (a.score !== b.score) return b.score - a.score
    return a.item.name.localeCompare(b.item.name)
  })

  if (maxResults > 0 && scored.length > maxResults) {
    return scored.slice(0, maxResults)
  }

  return scored
}

function getFieldWeight(field: FuzzyMatch['matchField']): number {
  switch (field) {
    case 'name':
      return NAME_WEIGHT
    case 'path':
      return PATH_WEIGHT
    case 'alias':
      return ALIAS_WEIGHT
    case 'keyword':
      return KEYWORD_WEIGHT
  }
}
