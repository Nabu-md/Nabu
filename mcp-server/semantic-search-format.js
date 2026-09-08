/**
 * Formatting for agent-visible retrieval output (search_notes and
 * search_notes_semantic / query_vault_rag).
 *
 * Every result cites its source vault by label so multi-vault answers can
 * attribute findings to the vault they came from, not just the note path.
 * When a query spans multiple vaults, a scope header states that explicitly.
 */

function vaultLine(result) {
  const label = typeof result.vaultLabel === 'string' && result.vaultLabel
    ? result.vaultLabel
    : 'Unknown vault'
  return `   Vault: ${label}`
}

/**
 * Render ranked results as markdown the agent can quote from. Both retrieval
 * tools share this format so agents see one consistent citation style.
 * @param {Array<{ path: string, title: string, snippet: string, score?: number, vaultLabel?: string, vaultPath?: string }>} results
 * @param {{ vaultCount?: number, emptyMessage?: string }} [options]
 *   vaultCount: number of vaults the search covered (header shown when > 1)
 *   emptyMessage: override for the zero-results text
 * @returns {string}
 */
export function formatRetrievalResults(results, options = {}) {
  if (!Array.isArray(results) || results.length === 0) {
    return options.emptyMessage ?? 'No matching notes found.'
  }

  const vaultCount = Number.isFinite(options.vaultCount) ? options.vaultCount : null
  const header = vaultCount !== null && vaultCount > 1
    ? `Matches across ${vaultCount} vaults, ranked by relevance. Each result cites its source vault.\n\n`
    : ''

  const body = results
    .map((result, index) => {
      const lines = [
        `${index + 1}. **${result.title}**`,
        vaultLine(result),
        `   Note: ${result.path}`,
      ]
      if (typeof result.score === 'number') {
        lines.push(`   Score: ${result.score.toFixed(3)}`)
      }
      if (result.snippet) {
        lines.push(`   > ${result.snippet.replace(/\s+/g, ' ').trim()}`)
      }
      return lines.join('\n')
    })
    .join('\n\n')

  return `${header}${body}`
}

/**
 * Back-compat alias for the semantic search formatter.
 */
export const formatSemanticSearchResults = formatRetrievalResults
