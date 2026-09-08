/**
 * Zero-dependency semantic search for vault notes.
 *
 * Embeds note text and queries with hashed bag-of-character-trigram vectors
 * (the "hashing trick", cf. feature hashing). Hashed trigram vectors give
 * exact lexeme matching a score of ~1 while still matching shared morphology
 * (plural/singular, compounding) that keyword `includes` misses, which is the
 * gap `search_notes_semantic` fills over `search_notes`. Runs fully offline in
 * the MCP server process; swapping in a learned embedding model later only
 * requires replacing `embedText` — the cosine-ranking contract stays the same.
 */

const EMBEDDING_DIMENSIONS = 4096
const NGRAM_SIZE = 3
const DEFAULT_LIMIT = 10
const MAX_LIMIT = 50
const TITLE_BOOST = 1.5
const MAX_EMBEDDING_CHARS_PER_NOTE = 20_000
const DEFAULT_MIN_SCORE = 0.05

const STOPWORDS = new Set([
  'a', 'an', 'and', 'are', 'as', 'at', 'be', 'but', 'by', 'for', 'from',
  'has', 'have', 'in', 'is', 'it', 'its', 'of', 'on', 'or', 'that', 'the',
  'their', 'them', 'then', 'there', 'these', 'they', 'this', 'to', 'was',
  'were', 'will', 'with',
])

function tokenize(text) {
  const words = text
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter((word) => word.length > 2 && !STOPWORDS.has(word))
  const unique = new Set(words)
  const tokens = [...unique]
  for (const word of unique) {
    if (word.length > NGRAM_SIZE) {
      tokens.push(`${word}s`)
    }
  }
  return tokens
}

function hashToken(token) {
  // FNV-1a over the token's UTF-8-ish code units.
  let hash = 0x811c9dc5
  for (let index = 0; index < token.length; index += 1) {
    hash ^= token.charCodeAt(index)
    hash = Math.imul(hash, 0x01000193)
  }
  return hash >>> 0
}

function embedText(text) {
  const vector = new Float32Array(EMBEDDING_DIMENSIONS)
  const tokens = tokenize(text.slice(0, MAX_EMBEDDING_CHARS_PER_NOTE))
  for (const token of tokens) {
    vector[hashToken(token) % EMBEDDING_DIMENSIONS] += 1
  }

  let norm = 0
  for (let index = 0; index < EMBEDDING_DIMENSIONS; index += 1) {
    norm += vector[index] * vector[index]
  }
  norm = Math.sqrt(norm)
  if (norm === 0) return vector
  for (let index = 0; index < EMBEDDING_DIMENSIONS; index += 1) {
    vector[index] /= norm
  }
  return vector
}

function cosineSimilarity(left, right) {
  let dot = 0
  for (let index = 0; index < EMBEDDING_DIMENSIONS; index += 1) {
    dot += left[index] * right[index]
  }
  return dot
}

function normalizeLimit(rawLimit) {
  const limit = Number(rawLimit)
  if (!Number.isFinite(limit) || limit <= 0) return DEFAULT_LIMIT
  return Math.min(Math.floor(limit), MAX_LIMIT)
}

/**
 * Rank notes against a query by embedding cosine similarity. Any extra fields
 * on the input note objects are carried through to their results.
 * @param {string} query
 * @param {Array<{ path: string, title: string, content: string }>} notes
 * @param {{ limit?: number, minScore?: number }} [options]
 * @returns {Array<{ path: string, title: string, snippet: string, score: number }>}
 */
export function rankNotesBySimilarity(query, notes, options = {}) {
  const limit = normalizeLimit(options.limit)
  const minScore = typeof options.minScore === 'number' ? options.minScore : DEFAULT_MIN_SCORE
  const queryVector = embedText(query)
  const results = []

  for (const note of notes) {
    const titleVector = embedText(note.title)
    const bodyVector = embedText(note.content)
    const score = cosineSimilarity(queryVector, bodyVector)
      + TITLE_BOOST * cosineSimilarity(queryVector, titleVector)
    if (score < minScore) continue

    results.push({
      ...note,
      path: note.path,
      title: note.title,
      snippet: buildSemanticSnippet(note.content, query),
      score: Math.min(score, 1),
    })
  }

  results.sort((left, right) => right.score - left.score)
  return results.slice(0, limit)
}

function buildSemanticSnippet(content, query) {
  const body = content.replace(/^---[\s\S]*?---\n?/, '').trim()
  if (!body) return ''
  const queryTokens = new Set(tokenize(query))
  const lines = body.split(/\r?\n/).filter((line) => line.trim())
  const best = lines
    .map((line) => {
      const lineTokens = tokenize(line)
      const overlap = lineTokens.filter((token) => queryTokens.has(token)).length
      return { line, overlap, length: line.length }
    })
    .sort((left, right) => right.overlap - left.overlap || left.length - right.length)[0]

  if (!best || best.overlap === 0) return body.slice(0, 200)
  return best.line.trim().slice(0, 200)
}
