import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { formatRetrievalResults } from './semantic-search-format.js'

describe('formatRetrievalResults', () => {
  const results = [
    {
      path: 'note/financing.md',
      title: 'Vehicle Financing',
      snippet: 'Lease costs and monthly payments',
      score: 0.812,
      vaultLabel: 'Work Vault',
      vaultPath: '/vaults/work',
    },
    {
      path: 'ideas/leasing.md',
      title: 'Leasing Guide',
      snippet: 'Monthly payments depend on residual value',
      score: 0.641,
      vaultLabel: 'Personal Vault',
      vaultPath: '/vaults/personal',
    },
  ]

  it('cites the vault label for every result', () => {
    const text = formatRetrievalResults(results, { vaultCount: 2 })

    assert.match(text, /Vault: Work Vault/)
    assert.match(text, /Vault: Personal Vault/)
    assert.match(text, /note\/financing\.md/)
    assert.match(text, /ideas\/leasing\.md/)
  })

  it('includes a multi-vault scope header when several vaults were searched', () => {
    const text = formatRetrievalResults(results, { vaultCount: 2 })

    assert.match(text, /across 2 vaults/)
    assert.match(text, /ranked by relevance/)
    assert.match(text, /cites its source vault/)
  })

  it('omits the scope header for single-vault searches', () => {
    const text = formatRetrievalResults(results.slice(0, 1), { vaultCount: 1 })

    assert.doesNotMatch(text, /across \d+ vaults/)
    assert.match(text, /Vault: Work Vault/)
  })

  it('numbers results in rank order', () => {
    const text = formatRetrievalResults(results, { vaultCount: 2 })

    const firstIndex = text.indexOf('1. **Vehicle Financing**')
    const secondIndex = text.indexOf('2. **Leasing Guide**')
    assert.ok(firstIndex !== -1, 'result 1 present')
    assert.ok(secondIndex !== -1, 'result 2 present')
    assert.ok(firstIndex < secondIndex, 'results appear in rank order')
  })

  it('includes the score and snippet for each result', () => {
    const text = formatRetrievalResults(results, { vaultCount: 2 })

    assert.match(text, /Score: 0\.812/)
    assert.match(text, /Score: 0\.641/)
    assert.match(text, /> Lease costs and monthly payments/)
  })

  it('omits the score line when results carry no score (search_notes)', () => {
    const keywordResults = [
      { path: 'a.md', title: 'A', snippet: 'alpha body', vaultLabel: 'Solo Vault', vaultPath: '/v' },
    ]
    const text = formatRetrievalResults(keywordResults, { vaultCount: 1 })

    assert.doesNotMatch(text, /Score:/)
    assert.match(text, /Vault: Solo Vault/)
    assert.match(text, /> alpha body/)
  })

  it('falls back to Unknown vault when the label is missing', () => {
    const text = formatRetrievalResults(
      [{ path: 'a.md', title: 'A', snippet: '', score: 0.5 }],
      { vaultCount: 1 },
    )

    assert.match(text, /Vault: Unknown vault/)
  })

  it('uses the provided empty message when there are no results', () => {
    const semantic = formatRetrievalResults([], {
      vaultCount: 3,
      emptyMessage: 'No semantically matching notes found. Try search_notes for exact keyword matches.',
    })
    const keyword = formatRetrievalResults([], {
      vaultCount: 3,
      emptyMessage: 'No matching notes found.',
    })

    assert.match(semantic, /No semantically matching notes found/)
    assert.match(semantic, /search_notes/)
    assert.match(keyword, /^No matching notes found\.$/)
  })

  it('handles a non-array payload defensively', () => {
    const text = formatRetrievalResults(null, {})
    assert.match(text, /No matching notes found/)
  })
})
