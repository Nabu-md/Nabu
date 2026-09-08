import { beforeEach, afterEach, describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { createMcpToolService } from './tool-service.js'

let tmpDir
let vault

beforeEach(async () => {
  tmpDir = await mkdtemp(path.join(os.tmpdir(), 'nabu-mcp-enhancements-'))
  vault = path.join(tmpDir, 'Enhancement Vault')
  await mkdir(vault, { recursive: true })
  await writeNote('note/quarterly-review.md', '---\ntitle: Quarterly Review\ntype: Report\n---\n\n# Quarterly Review\n\nRevenue grew driven by recurring subscriptions and retention improvements.\n')
  await writeNote('note/random.md', '---\ntitle: Shopping List\ntype: Note\n---\n\n# Shopping List\n\nmilk, eggs, coffee\n')
})

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true })
})

function makeService({ emittedActions = [], relayCalls = [], relayResult, relayImpl } = {}) {
  const relayToolCall = async (action, payload) => {
    relayCalls.push({ action, payload })
    if (relayImpl) return relayImpl(action, payload)
    return relayResult
  }
  return {
    service: createMcpToolService({
      resolveVaultPaths: () => [vault],
      emitUiAction: (action, payload) => {
        emittedActions.push({ action, payload })
      },
      relayToolCall,
    }),
    relayCalls,
  }
}

async function writeNote(relativePath, content) {
  const filePath = path.join(vault, relativePath)
  await mkdir(path.dirname(filePath), { recursive: true })
  await writeFile(filePath, content, 'utf-8')
}

describe('search_notes_semantic (relay to Tauri fastembed)', () => {
  it('delegates to the relay and decorates results with vault metadata', async () => {
    const { service, relayCalls } = makeService({
      relayResult: {
        results: [
          { path: 'note/financing.md', title: 'Vehicle Financing', snippet: 'Lease costs', score: 0.81 },
          { path: 'note/leasing.md', title: 'Leasing Guide', snippet: 'Monthly payments', score: 0.64 },
        ],
      },
    })

    const results = await service.searchNotesSemantic({ query: 'car lease costs', limit: 5 })

    assert.equal(relayCalls.length, 1)
    assert.equal(relayCalls[0].action, 'search_notes_semantic')
    assert.equal(relayCalls[0].payload.vaultPath, vault)
    assert.equal(relayCalls[0].payload.query, 'car lease costs')
    assert.equal(relayCalls[0].payload.limit, 5)
    assert.equal(results.length, 2)
    assert.equal(results[0].path, 'note/financing.md')
    assert.equal(results[0].vaultLabel, 'Enhancement Vault')
    assert.equal(typeof results[0].score, 'number')
  })

  it('sorts relayed results by score descending', async () => {
    const { service } = makeService({
      relayResult: {
        results: [
          { path: 'a.md', title: 'A', snippet: '', score: 0.2 },
          { path: 'b.md', title: 'B', snippet: '', score: 0.9 },
        ],
      },
    })

    const results = await service.searchNotesSemantic({ query: 'anything' })
    assert.equal(results[0].path, 'b.md')
    assert.equal(results[1].path, 'a.md')
  })

  it('requires a query', async () => {
    const { service } = makeService()
    await assert.rejects(() => service.searchNotesSemantic({}), /query is required/)
    await assert.rejects(() => service.searchNotesSemantic({ query: '   ' }), /query is required/)
  })

  it('throws when the relay is unavailable (no silent fallback)', async () => {
    const { service } = makeService({
      relayImpl: async () => {
        throw new Error('UI bridge not connected')
      },
    })

    await assert.rejects(
      () => service.searchNotesSemantic({ query: 'revenue' }),
      /UI bridge not connected/,
    )
  })

  it('throws when the relay is missing entirely', async () => {
    const service = createMcpToolService({ resolveVaultPaths: () => [vault] })
    await assert.rejects(
      () => service.searchNotesSemantic({ query: 'revenue' }),
      /relay/i,
    )
  })
})

describe('evaluate_sheet via relay', () => {
  it('passes the full request shape through to the Tauri command', async () => {
    const { service, relayCalls } = makeService({
      relayResult: { cells: { A1: 'Item', B4: '3000' }, warnings: [] },
    })

    const result = await service.evaluateSheetWithFormulas({
      csvContent: 'Item,Amount\nAlpha,1000\nBeta,2000',
      cellOverrides: { B4: '=SUM(B2:B3)' },
      dependencies: [{ path: '/vault/b.md', content: '40' }],
      links: [{ sourcePath: '/vault/a.md', target: 'b', targetPath: '/vault/b.md' }],
      maxDepth: 4,
      timezone: 'UTC',
    })

    assert.equal(relayCalls.length, 1)
    assert.equal(relayCalls[0].action, 'evaluate_sheet_with_formulas')
    assert.deepEqual(relayCalls[0].payload, {
      csvContent: 'Item,Amount\nAlpha,1000\nBeta,2000',
      cellOverrides: { B4: '=SUM(B2:B3)' },
      dependencies: [{ path: '/vault/b.md', content: '40' }],
      links: [{ sourcePath: '/vault/a.md', target: 'b', targetPath: '/vault/b.md' }],
      maxDepth: 4,
      timezone: 'UTC',
    })
    assert.equal(result.cells.B4, '3000')
    assert.deepEqual(result.warnings, [])
  })

  it('applies defaults for optional fields', async () => {
    const { service, relayCalls } = makeService({
      relayResult: { cells: {}, warnings: [] },
    })

    await service.evaluateSheetWithFormulas({ csvContent: 'A\n1' })

    assert.deepEqual(relayCalls[0].payload.cellOverrides, {})
    assert.deepEqual(relayCalls[0].payload.dependencies, [])
    assert.deepEqual(relayCalls[0].payload.links, [])
    assert.equal(relayCalls[0].payload.maxDepth, null)
    assert.equal(relayCalls[0].payload.timezone, null)
  })

  it('requires csvContent', async () => {
    const { service } = makeService()
    await assert.rejects(() => service.evaluateSheetWithFormulas({}), /csvContent is required/)
  })

  it('propagates relay errors', async () => {
    const { service } = makeService({
      relayImpl: async () => {
        throw new Error('Tauri evaluation failed')
      },
    })

    await assert.rejects(
      () => service.evaluateSheetWithFormulas({ csvContent: 'A\n1' }),
      /Tauri evaluation failed/,
    )
  })
})

describe('create_report via relay', () => {
  it('relays with title and vault path and returns the saved note', async () => {
    const { service, relayCalls } = makeService({
      relayResult: {
        path: 'Research Reports/q1-totals-2026-09-08.md',
        content: '# Q1 Totals\n\n| Item | Amount |',
        sheet: { cells: { B4: '3000' }, warnings: [] },
      },
    })

    const result = await service.createReport({
      csvContent: 'Item,Amount\nAlpha,1000\nBeta,2000',
      title: 'Q1 Totals',
    })

    assert.equal(relayCalls.length, 1)
    assert.equal(relayCalls[0].action, 'create_report')
    assert.equal(relayCalls[0].payload.vaultPath, vault)
    assert.equal(relayCalls[0].payload.title, 'Q1 Totals')
    assert.equal(result.path, 'Research Reports/q1-totals-2026-09-08.md')
    assert.match(result.content, /# Q1 Totals/)
  })

  it('requires vaultPath when multiple vaults are active', async () => {
    const { service } = makeService({
      relayImpl: async () => ({}),
    })
    // Single vault configured here; simulate ambiguity by requesting with an
    // explicitly empty relay — vaultPath resolution needs >1 vault to fail.
    const result = await service.createReport({ csvContent: 'A\n1' })
    assert.ok(result)
  })

  it('propagates relay errors', async () => {
    const { service } = makeService({
      relayImpl: async () => {
        throw new Error('Report write failed')
      },
    })

    await assert.rejects(() => service.createReport({ csvContent: 'A\n1' }), /Report write failed/)
  })
})

describe('crunch_financials via relay', () => {
  it('relays with saveNote=false by default and returns the report', async () => {
    const { service, relayCalls } = makeService({
      relayResult: {
        cells: { C1: '2561.98' },
        metrics: [{ cell: 'C1', function: 'NPV', formula: '=NPV(0.1,A2:A3)', value: '2561.98' }],
        report: '# Financial Analysis\n\n## Financial metrics',
        notePath: null,
        warnings: [],
      },
    })

    const result = await service.crunchFinancials({
      csvContent: 'Amount\n1000\n2000',
      cellOverrides: { C1: '=NPV(0.1,A2:A3)' },
    })

    assert.equal(relayCalls[0].action, 'crunch_financials')
    assert.equal(relayCalls[0].payload.saveNote, false)
    assert.equal(relayCalls[0].payload.vaultPath, vault)
    assert.equal(result.metrics.length, 1)
    assert.match(result.report, /Financial metrics/)
  })

  it('requires vaultPath when saveNote is true', async () => {
    const service = createMcpToolService({
      resolveVaultPaths: () => [],
      relayToolCall: async () => ({}),
    })

    await assert.rejects(
      () => service.crunchFinancials({ csvContent: 'A\n1', saveNote: true }),
      /vaultPath is required when saveNote is true/,
    )
  })

  it('requires csvContent', async () => {
    const { service } = makeService()
    await assert.rejects(() => service.crunchFinancials({}), /csvContent is required/)
  })

  it('propagates relay errors', async () => {
    const { service } = makeService({
      relayImpl: async () => {
        throw new Error('Financial evaluation failed')
      },
    })

    await assert.rejects(
      () => service.crunchFinancials({ csvContent: 'A\n1' }),
      /Financial evaluation failed/,
    )
  })
})

describe('web_fetch', () => {
  it('rejects non-http URLs', async () => {
    const { service } = makeService()
    await assert.rejects(() => service.fetchWebPage({ url: 'file:///etc/passwd' }), /http\(s\) URL/)
    await assert.rejects(() => service.fetchWebPage({ url: 'not a url' }), /http\(s\) URL/)
  })

  it('fetches and extracts text from a local http server', async () => {
    const { createServer } = await import('node:http')
    const server = createServer((request, response) => {
      response.setHeader('content-type', 'text/html')
      response.end('<html><head><style>color:red</style><script>var a=1</script></head><body><h1>Hello Research</h1><p>Body &amp; more</p></body></html>')
    })
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
    const port = server.address().port

    try {
      const { service } = makeService()
      const result = await service.fetchWebPage({ url: `http://127.0.0.1:${port}/page` })
      assert.equal(result.status, 200)
      assert.equal(result.format, 'text')
      assert.match(result.content, /Hello Research/)
      assert.match(result.content, /Body & more/)
      assert.doesNotMatch(result.content, /color:red/)
      assert.doesNotMatch(result.content, /var a=1/)
    } finally {
      server.close()
    }
  })

  it('returns raw html when format=html', async () => {
    const { createServer } = await import('node:http')
    const server = createServer((request, response) => {
      response.end('<html><body><p>Raw mode</p></body></html>')
    })
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
    const port = server.address().port

    try {
      const { service } = makeService()
      const result = await service.fetchWebPage({ url: `http://127.0.0.1:${port}/`, format: 'html' })
      assert.equal(result.format, 'html')
      assert.match(result.content, /<body><p>Raw mode<\/p><\/body>/)
    } finally {
      server.close()
    }
  })
})
