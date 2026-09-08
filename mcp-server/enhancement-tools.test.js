import { beforeEach, afterEach, describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { createMcpToolService } from './tool-service.js'
import { parseSheetGrid, parseCellAddress } from './sheet-eval.js'

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

function makeService({ emittedActions = [] } = {}) {
  return createMcpToolService({
    resolveVaultPaths: () => [vault],
    emitUiAction: (action, payload) => {
      emittedActions.push({ action, payload })
    },
  })
}

async function writeNote(relativePath, content) {
  const filePath = path.join(vault, relativePath)
  await mkdir(path.dirname(filePath), { recursive: true })
  await writeFile(filePath, content, 'utf-8')
}

describe('search_notes_semantic', () => {
  it('ranks conceptually related notes above unrelated ones and includes vault metadata', async () => {
    const service = makeService()

    const results = await service.searchNotesSemantic({ query: 'subscription revenue growth', limit: 5 })

    assert.ok(results.length >= 1)
    assert.equal(results[0].path, 'note/quarterly-review.md')
    assert.equal(results[0].vaultLabel, 'Enhancement Vault')
    assert.equal(typeof results[0].score, 'number')
    assert.ok(results[0].score > 0)
    assert.ok(!results.some((result) => result.path === 'note/random.md'))
  })

  it('requires a query', async () => {
    const service = makeService()
    await assert.rejects(() => service.searchNotesSemantic({}), /query is required/)
    await assert.rejects(() => service.searchNotesSemantic({ query: '   ' }), /query is required/)
  })

  it('respects the limit', async () => {
    const service = makeService()
    const results = await service.searchNotesSemantic({ query: 'revenue', limit: 1 })
    assert.ok(results.length <= 1)
  })
})

describe('web_fetch', () => {
  it('rejects non-http URLs', async () => {
    const service = makeService()
    await assert.rejects(() => service.fetchWebPage({ url: 'file:///etc/passwd' }), /http\(s\) URL/)
    await assert.rejects(() => service.fetchWebPage({ url: 'not a url' }), /http\(s\) URL/)
  })

  it('fetches and extracts text from a local http server', async () => {
    const { createServer } = await import('node:http')
    const server = createServer((request, response) => {
      response.setHeader('content-type', 'text/html')
      response.end('<html><head><style>.x{color:red}</style><script>var a=1;</script></head><body><h1>Hello&nbsp;Research</h1><p>Body &amp; more</p></body></html>')
    })
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
    const port = server.address().port

    try {
      const service = makeService()
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
      const service = makeService()
      const result = await service.fetchWebPage({ url: `http://127.0.0.1:${port}/`, format: 'html' })
      assert.equal(result.format, 'html')
      assert.match(result.content, /<body><p>Raw mode<\/p><\/body>/)
    } finally {
      server.close()
    }
  })
})

describe('evaluate_sheet via tool-service', () => {
  it('evaluates csv with formulas and returns the formatted grid', async () => {
    const service = makeService()

    const result = await service.evaluateSheetWithFormulas({
      csvContent: 'Item,Amount\nAlpha,1000\nBeta,2000\nTotal,=SUM(B2:B3)',
      cellOverrides: { D2: '=NPV(0.1,B2:B3)' },
    })

    assert.equal(result.cells.A1, 'Item')
    assert.equal(result.cells.B2, '1000')
    assert.equal(result.cells.B4, '3000')
    assert.match(result.cells.D2, /2,?561\.98|2561\.98/)
    assert.deepEqual(result.warnings, [])
  })

  it('warns on invalid override addresses', async () => {
    const service = makeService()

    const result = await service.evaluateSheetWithFormulas({
      csvContent: 'A,B\n1,2',
      cellOverrides: { 'nope': '=1+1' },
    })

    assert.equal(result.warnings.length, 1)
    assert.match(result.warnings[0], /invalid cell address/)
  })

  it('requires csvContent', async () => {
    const service = makeService()
    await assert.rejects(() => service.evaluateSheetWithFormulas({}), /csvContent is required/)
  })
})

describe('parseSheetGrid', () => {
  it('parses csv rows', () => {
    assert.deepEqual(parseSheetGrid('a,b\n1,2'), [['a', 'b'], ['1', '2']])
  })

  it('normalizes markdown tables and drops separator rows', () => {
    const grid = parseSheetGrid('| Name | Value |\n| --- | --- |\n| Alpha | 1 |')
    assert.deepEqual(grid, [['Name', 'Value'], ['Alpha', '1']])
  })

  it('handles quoted csv fields', () => {
    const grid = parseSheetGrid('"say, hi",2')
    assert.deepEqual(grid, [['say, hi', '2']])
  })
})

describe('parseCellAddress', () => {
  it('parses absolute and relative addresses', () => {
    assert.deepEqual(parseCellAddress('B2'), { row: 2, column: 2 })
    assert.deepEqual(parseCellAddress('$D$10'), { row: 10, column: 4 })
    assert.equal(parseCellAddress('invalid'), null)
    assert.equal(parseCellAddress('0'), null)
  })
})
