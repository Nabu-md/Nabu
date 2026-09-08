/**
 * IronCalc-powered sheet evaluation for the `evaluate_sheet` MCP tool.
 *
 * Uses the same `@ironcalc/wasm` engine the Nabu sheet editor uses in the
 * frontend, so agents see identical formula semantics — including financial
 * functions (NPV, IRR, PMT, PV, FV, RATE), aggregations (SUM, SUMIF, SUMIFS,
 * AVERAGE, COUNTIF), and lookups (VLOOKUP, HLOOKUP, INDEX, MATCH). The wasm
 * runtime is initialized lazily from the package's `wasm_bg.wasm` file.
 */

const SHEET_INDEX = 0
const MAX_SHEET_ROWS = 10_000
const MAX_SHEET_COLUMNS = 256
const MAX_EVALUATED_CELLS = 20_000

let modelFactory = null
let initPromise = null

function parseCsvRows(source) {
  const rows = []
  let row = []
  let field = ''
  let inQuotes = false
  const text = String(source ?? '')

  for (let index = 0; index < text.length; index += 1) {
    const character = text[index]

    if (inQuotes) {
      if (character === '"') {
        if (text[index + 1] === '"') {
          field += '"'
          index += 1
        } else {
          inQuotes = false
        }
      } else {
        field += character
      }
      continue
    }

    if (character === '"') {
      inQuotes = true
    } else if (character === ',') {
      row.push(field)
      field = ''
    } else if (character === '\n' || character === '\r') {
      if (character === '\r' && text[index + 1] === '\n') index += 1
      row.push(field)
      field = ''
      if (row.length > 1 || row[0] !== '') rows.push(row)
      row = []
    } else {
      field += character
    }
  }

  if (field !== '' || row.length > 0) {
    row.push(field)
    if (row.length > 1 || row[0] !== '') rows.push(row)
  }

  return rows
}

/**
 * Parse CSV or markdown-table content into a row grid. Markdown table rows
 * (`| a | b |`) and separator rows (`| --- | --- |`) are normalized first so
 * agents can paste tables straight out of notes.
 */
export function parseSheetGrid(csvContent) {
  const text = String(csvContent ?? '')
  if (!text.trim()) return []

  const looksLikeMarkdownTable = /^\s*\|.*\|\s*$/m.test(text)
  const normalized = looksLikeMarkdownTable
    ? text
        .split(/\r?\n/)
        .filter((line) => !/^\s*\|[\s:|-]+\|\s*$/.test(line))
        .map((line) => line.trim().replace(/^\|/, '').replace(/\|$/, '').split('|').map((cell) => cell.trim()).join(','))
        .join('\n')
    : text

  return parseCsvRows(normalized)
}

function columnLetterToIndex(letters) {
  let value = 0
  for (const character of letters) {
    const upper = character.toUpperCase()
    if (upper < 'A' || upper > 'Z') return null
    value = value * 26 + (upper.charCodeAt(0) - 64)
  }
  return value > 0 ? value : null
}

export function parseCellAddress(address) {
  const match = /^\$?([A-Za-z]{1,3})\$?([1-9]\d*)$/.exec(String(address ?? '').trim())
  if (!match) return null
  const column = columnLetterToIndex(match[1])
  const row = Number(match[2])
  if (!column || row > MAX_SHEET_ROWS) return null
  return { row, column }
}

async function loadModelFactory() {
  if (modelFactory) return modelFactory
  if (!initPromise) {
    initPromise = (async () => {
      const wasmModule = await import('@ironcalc/wasm/wasm.js')
      const { readFileSync } = await import('node:fs')
      const wasmPath = new URL('../node_modules/@ironcalc/wasm/wasm_bg.wasm', import.meta.url)
      const bytes = readFileSync(wasmPath)
      await wasmModule.default(bytes)
      modelFactory = wasmModule.Model
      return modelFactory
    })().catch((error) => {
      initPromise = null
      throw error
    })
  }
  return initPromise
}

function markdownCellText(value) {
  if (typeof value !== 'string') return String(value ?? '')
  if (value.startsWith('=')) return value
  const match = /^(\*\*\*|\*\*|__|_|~~)(.*)\1$/.exec(value.trim())
  if (match) {
    const inner = match[2].trim()
    if (!inner.startsWith('=')) return inner
  }
  return value
}

export async function evaluateSheet({ csvContent, cellOverrides } = {}) {
  if (typeof csvContent !== 'string' || !csvContent.trim()) {
    throw new Error('csvContent is required')
  }

  const Model = await loadModelFactory()
  const model = new Model('Nabu Sheet', 'en', 'UTC')
  try {
    const rows = parseSheetGrid(csvContent)
    const evaluated = new Map()

    for (let rowIndex = 0; rowIndex < Math.min(rows.length, MAX_SHEET_ROWS); rowIndex += 1) {
      const row = rows[rowIndex]
      for (let columnIndex = 0; columnIndex < Math.min(row.length, MAX_SHEET_COLUMNS); columnIndex += 1) {
        const text = markdownCellText(row[columnIndex])
        if (text === '') continue
        model.setUserInput(SHEET_INDEX, rowIndex + 1, columnIndex + 1, text)
        evaluated.set(`${rowIndex + 1}:${columnIndex + 1}`, text)
      }
    }

    const warnings = []
    if (cellOverrides && typeof cellOverrides === 'object') {
      for (const [address, rawValue] of Object.entries(cellOverrides)) {
        const indexes = parseCellAddress(address)
        if (!indexes) {
          warnings.push(`Ignored invalid cell address: ${address}`)
          continue
        }
        const value = markdownCellText(rawValue)
        model.setUserInput(SHEET_INDEX, indexes.row, indexes.column, value)
        evaluated.set(`${indexes.row}:${indexes.column}`, value)
      }
    }

    model.evaluate()

    const cells = {}
    for (const key of evaluated.keys()) {
      const [rowText, columnText] = key.split(':')
      const row = Number(rowText)
      const column = Number(columnText)
      if (Object.keys(cells).length >= MAX_EVALUATED_CELLS) break
      const address = `${columnAddress(column)}${row}`
      cells[address] = model.getFormattedCellValue(SHEET_INDEX, row, column)
    }

    return { cells, warnings }
  } finally {
    model.free?.()
  }
}

function columnAddress(column) {
  let name = ''
  let value = column
  while (value > 0) {
    const remainder = (value - 1) % 26
    name = String.fromCharCode(65 + remainder) + name
    value = Math.floor((value - 1) / 26)
  }
  return name
}
