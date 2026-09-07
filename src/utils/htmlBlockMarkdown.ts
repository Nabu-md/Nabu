import {
  type BlockLike,
  type DurableBlockCodec,
  type DurableFencePayloadInput,
  injectDurableMarkdownBlocks,
  preProcessDurableMarkdownBlocks,
  readCodeBlockLanguage,
  readInlineText,
} from './durableMarkdownBlocks'

const BLOCK_TYPE = 'htmlBlock'
const BLOCK_DEFAULT_HEIGHT = '320'
const BLOCK_MIN_HEIGHT = 180
const BLOCK_MAX_HEIGHT = 960
const BLOCK_SCRIPTS_BLOCKED = 'blocked'
const BLOCK_SCRIPTS_SANDBOXED = 'sandboxed'

export {
  BLOCK_DEFAULT_HEIGHT as HTML_BLOCK_DEFAULT_HEIGHT,
  BLOCK_MAX_HEIGHT as HTML_BLOCK_MAX_HEIGHT,
  BLOCK_MIN_HEIGHT as HTML_BLOCK_MIN_HEIGHT,
  BLOCK_SCRIPTS_BLOCKED as HTML_BLOCK_SCRIPTS_BLOCKED,
  BLOCK_SCRIPTS_SANDBOXED as HTML_BLOCK_SCRIPTS_SANDBOXED,
  BLOCK_TYPE as HTML_BLOCK_TYPE,
}

const TOKEN_PREFIX = '@@NABU_HTML_BLOCK:'
const TOKEN_SUFFIX = '@@'

export type HtmlBlockScripts = typeof BLOCK_SCRIPTS_BLOCKED | typeof BLOCK_SCRIPTS_SANDBOXED

interface HtmlBlockPayload {
  height: string
  html: string
  scripts: HtmlBlockScripts
}

class StoredPayload implements HtmlBlockPayload {
  readonly height: string
  readonly html: string
  readonly scripts: HtmlBlockScripts

  constructor(height: string, sourceHtml: string, scripts: HtmlBlockScripts) {
    this.height = height
    this.html = sourceHtml
    this.scripts = scripts
  }
}

class StoredProps {
  readonly [key: string]: string
  readonly height: string
  readonly html: string
  readonly scripts: HtmlBlockScripts

  constructor(current: Record<string, string> | undefined, payload: HtmlBlockPayload) {
    Object.assign(this, current)
    this.height = payload.height
    this.html = payload.html
    this.scripts = payload.scripts
  }
}

interface HtmlFenceSource {
  height: string
  html: string
  scripts?: unknown
}

interface FenceAttributeRequest {
  info: string
  name: 'height' | 'scripts'
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function readFenceAttribute({ info, name }: FenceAttributeRequest): string {
  for (const match of info.matchAll(/\b([A-Za-z][\w-]*)=(?:"([^"]+)"|'([^']+)'|([^\s]+))/gu)) {
    if (match.at(1) === name) return match.at(2) ?? match.at(3) ?? match.at(4) ?? ''
  }
  return ''
}

function normalizeBlockHeight(value: unknown): string {
  if (typeof value !== 'string' && typeof value !== 'number') return BLOCK_DEFAULT_HEIGHT

  const parsed = Number.parseInt(String(value), 10)
  if (!Number.isFinite(parsed)) return BLOCK_DEFAULT_HEIGHT
  if (parsed < BLOCK_MIN_HEIGHT || parsed > BLOCK_MAX_HEIGHT) return BLOCK_DEFAULT_HEIGHT
  return String(parsed)
}

function clampBlockHeight(value: number): string {
  if (!Number.isFinite(value)) return BLOCK_DEFAULT_HEIGHT
  return String(Math.min(BLOCK_MAX_HEIGHT, Math.max(BLOCK_MIN_HEIGHT, Math.round(value))))
}

function normalizeBlockScripts(value: unknown): HtmlBlockScripts {
  return value === BLOCK_SCRIPTS_SANDBOXED ? BLOCK_SCRIPTS_SANDBOXED : BLOCK_SCRIPTS_BLOCKED
}

export {
  clampBlockHeight as clampHtmlBlockHeight,
  normalizeBlockHeight as normalizeHtmlBlockHeight,
  normalizeBlockScripts as normalizeHtmlBlockScripts,
}

function decodeBlockPayload(payload: unknown): HtmlBlockPayload | null {
  if (!isRecord(payload)) return null
  const markup = Reflect.get(payload, 'html')
  if (typeof markup !== 'string') return null
  return new StoredPayload(
    normalizeBlockHeight(payload.height),
    markup,
    normalizeBlockScripts(payload.scripts),
  )
}

function readFenceMetadata(info: string): Pick<HtmlBlockPayload, 'height' | 'scripts'> | null {
  const [language = '', ...infoParts] = info.trim().split(/\s+/u)
  if (language.toLowerCase() !== 'html') return null
  const attributeInfo = infoParts.join(' ')

  return {
    height: normalizeBlockHeight(readFenceAttribute({
      info: attributeInfo,
      name: 'height',
    })),
    scripts: normalizeBlockScripts(readFenceAttribute({
      info: attributeInfo,
      name: 'scripts',
    })),
  }
}

function buildBlockPayload({ lines, start, end, metadata }: DurableFencePayloadInput): HtmlBlockPayload {
  const markup = lines.slice(start + 1, end).join('')
  return new StoredPayload(
    String(Reflect.get(metadata as object, 'height')),
    markup,
    normalizeBlockScripts(Reflect.get(metadata as object, 'scripts')),
  )
}

function buildBlock(block: BlockLike, rawPayload: unknown): BlockLike {
  const payload = rawPayload as StoredPayload
  return {
    ...block,
    type: BLOCK_TYPE,
    props: new StoredProps(block.props, payload),
    content: undefined,
    children: [],
  }
}

function readSourceCodeBlock(block: BlockLike): HtmlBlockPayload | null {
  if (block.type !== 'codeBlock') return null
  if (readCodeBlockLanguage({ block }) !== 'html') return null

  const markup = readInlineText(block.content)
  if (markup === null) return null
  return new StoredPayload(BLOCK_DEFAULT_HEIGHT, markup, BLOCK_SCRIPTS_BLOCKED)
}

function fenceLengthForMarkup(markup: string): number {
  const longestRun = Math.max(0, ...Array.from(markup.matchAll(/`+/gu), match => match[0].length))
  return Math.max(3, longestRun + 1)
}

function escapeFenceAttribute(value: string): string {
  return value.replace(/"/gu, '&quot;')
}

function fenceSource(source: HtmlFenceSource): string {
  const { height, scripts: requestedScripts } = source
  const markup = Reflect.get(source, 'html') as string
  const normalizedHeight = normalizeBlockHeight(height)
  const scripts = normalizeBlockScripts(requestedScripts)
  const scriptAttribute = scripts === BLOCK_SCRIPTS_SANDBOXED ? ' scripts="sandboxed"' : ''
  const fence = '`'.repeat(fenceLengthForMarkup(markup))
  const markupBody = markup.endsWith('\n') ? markup : `${markup}\n`
  return `${fence}html height="${escapeFenceAttribute(normalizedHeight)}"${scriptAttribute}\n${markupBody}${fence}`
}

function isBlock(block: BlockLike): boolean {
  return block.type === BLOCK_TYPE
    && typeof Reflect.get(block.props ?? {}, 'html') === 'string'
    && typeof block.props?.height === 'string'
}

function serializeBlock(block: BlockLike): string {
  return fenceSource({
    height: block.props?.height ?? BLOCK_DEFAULT_HEIGHT,
    html: Reflect.get(block.props ?? {}, 'html') ?? '',
    scripts: block.props?.scripts,
  })
}

const durableCodec: DurableBlockCodec = {
  tokenPrefix: TOKEN_PREFIX,
  tokenSuffix: TOKEN_SUFFIX,
  readFenceMetadata,
  buildPayload: buildBlockPayload,
  decodePayload: decodeBlockPayload,
  buildBlock: buildBlock.bind(null),
  readCodeBlock: readSourceCodeBlock,
  isBlock,
  serializeBlock,
}

function preProcessBlockMarkdown({ markdown }: { markdown: string }): string {
  return preProcessDurableMarkdownBlocks({ markdown, codecs: [durableCodec] })
}

function injectBlockInBlocks(blocks: unknown[]): unknown[] {
  return injectDurableMarkdownBlocks({ blocks, codecs: [durableCodec] })
}

export {
  durableCodec as htmlBlockMarkdownCodec,
  fenceSource as htmlFenceSource,
  injectBlockInBlocks as injectHtmlBlockInBlocks,
  preProcessBlockMarkdown as preProcessHtmlBlockMarkdown,
  serializeBlock as htmlBlockMarkdown,
}
