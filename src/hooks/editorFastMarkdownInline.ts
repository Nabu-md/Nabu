export interface FastMarkdownTextStyles {
  bold?: boolean
  code?: boolean
  italic?: boolean
  strike?: boolean
}

export interface FastMarkdownInlineItem {
  type: 'link' | 'text'
  href?: string
  text?: string
  content?: FastMarkdownInlineItem[]
  styles?: FastMarkdownTextStyles
}

interface InlineLink {
  end: number
  href: string
  label: string
}

type StyleKey = keyof FastMarkdownTextStyles
type InlineMarkdownText = string
type MarkdownHref = string
type MarkdownToken = string
type TextIndex = number

interface InlineParserState {
  index: TextIndex
  items: FastMarkdownInlineItem[]
  styles: FastMarkdownTextStyles
  text: InlineMarkdownText
}

const DURABLE_MARKDOWN_TOKEN_PREFIX = '@@NABU_'
const DURABLE_MARKDOWN_TOKEN_RE = /^@@NABU_[A-Z_]+:[^@]+@@$/u
const TEXT_STYLE_KEYS: StyleKey[] = ['bold', 'code', 'italic', 'strike']

export function fastMarkdownTextItem(
  text: InlineMarkdownText,
  styles: FastMarkdownTextStyles = {},
): FastMarkdownInlineItem {
  return { type: 'text', text, styles }
}

function stylesEqual(left?: FastMarkdownTextStyles, right?: FastMarkdownTextStyles): boolean {
  return TEXT_STYLE_KEYS.every(style => Boolean(Reflect.get(left ?? {}, style)) === Boolean(Reflect.get(right ?? {}, style)))
}

function appendText(items: FastMarkdownInlineItem[], text: InlineMarkdownText, styles: FastMarkdownTextStyles): void {
  if (!text) return
  const previous = items.at(-1)
  if (previous?.type === 'text' && stylesEqual(previous.styles, styles)) {
    previous.text = `${previous.text ?? ''}${text}`
    return
  }
  items.push(fastMarkdownTextItem(text, { ...styles }))
}

function isEscaped(text: InlineMarkdownText, index: TextIndex): boolean {
  let slashCount = 0
  for (let cursor = index - 1; cursor >= 0 && text.charAt(cursor) === '\\'; cursor -= 1) {
    slashCount += 1
  }
  return slashCount % 2 === 1
}

function findUnescaped(text: InlineMarkdownText, needle: MarkdownToken, from: TextIndex): TextIndex {
  let index = text.indexOf(needle, from)
  while (index !== -1 && isEscaped(text, index)) index = text.indexOf(needle, index + needle.length)
  return index
}

function markdownLinkBounds(text: InlineMarkdownText, index: TextIndex): { hrefEnd: TextIndex; labelEnd: TextIndex } | null {
  if (text.charAt(index) !== '[' || isEscaped(text, index)) return null
  const labelEnd = findUnescaped(text, ']', index + 1)
  if (labelEnd === -1 || text.charAt(labelEnd + 1) !== '(') return null
  const hrefEnd = findUnescaped(text, ')', labelEnd + 2)
  return hrefEnd === -1 ? null : { hrefEnd, labelEnd }
}

function validMarkdownLinkHref(href: MarkdownHref): boolean {
  if (!href) return false
  if (!/\s/u.test(href)) return true
  return href.startsWith('<') && href.endsWith('>')
}

function normalizedMarkdownLinkHref(href: MarkdownHref): MarkdownHref {
  return href.startsWith('<') && href.endsWith('>') ? href.slice(1, -1) : href
}

function readLinkAt(text: InlineMarkdownText, index: TextIndex): InlineLink | null {
  const bounds = markdownLinkBounds(text, index)
  if (!bounds) return null

  const href = text.slice(bounds.labelEnd + 2, bounds.hrefEnd).trim()
  if (!validMarkdownLinkHref(href)) return null
  return {
    end: bounds.hrefEnd + 1,
    href: normalizedMarkdownLinkHref(href),
    label: text.slice(index + 1, bounds.labelEnd),
  }
}

function isDurableMarkdownToken(text: InlineMarkdownText): boolean {
  return DURABLE_MARKDOWN_TOKEN_RE.test(text)
}

function readDurableMarkdownTokenAt(text: InlineMarkdownText, index: TextIndex): MarkdownToken | null {
  if (!text.startsWith(DURABLE_MARKDOWN_TOKEN_PREFIX, index)) return null
  const tokenEnd = text.indexOf('@@', index + DURABLE_MARKDOWN_TOKEN_PREFIX.length)
  if (tokenEnd === -1) return null
  const token = text.slice(index, tokenEnd + 2)
  return isDurableMarkdownToken(token) ? token : null
}

function nextStyleMarker(text: InlineMarkdownText, index: TextIndex): { style: StyleKey; token: MarkdownToken } | null {
  if (isEscaped(text, index)) return null
  if (text.startsWith('**', index)) return { style: 'bold', token: '**' }
  if (text.startsWith('__', index)) return { style: 'bold', token: '__' }
  if (text.startsWith('~~', index)) return { style: 'strike', token: '~~' }
  if (text.charAt(index) === '*') return { style: 'italic', token: '*' }
  if (text.charAt(index) === '_') return { style: 'italic', token: '_' }
  return null
}

function appendDurableToken(state: InlineParserState): boolean {
  const token = readDurableMarkdownTokenAt(state.text, state.index)
  if (!token) return false
  appendText(state.items, token, state.styles)
  state.index += token.length
  return true
}

function appendLink(state: InlineParserState): boolean {
  const link = readLinkAt(state.text, state.index)
  if (!link) return false
  state.items.push({
    type: 'link',
    href: link.href,
    content: parseFastMarkdownInline(link.label, state.styles),
  })
  state.index = link.end
  return true
}

function appendCode(state: InlineParserState): boolean {
  if (state.text.charAt(state.index) !== '`' || isEscaped(state.text, state.index)) return false
  const end = findUnescaped(state.text, '`', state.index + 1)
  if (end === -1) return false
  appendText(state.items, state.text.slice(state.index + 1, end), { ...state.styles, code: true })
  state.index = end + 1
  return true
}

function appendStyledText(state: InlineParserState): boolean {
  const marker = nextStyleMarker(state.text, state.index)
  if (!marker) return false
  const end = findUnescaped(state.text, marker.token, state.index + marker.token.length)
  if (end === -1) return false
  const inner = state.text.slice(state.index + marker.token.length, end)
  state.items.push(...parseFastMarkdownInline(inner, { ...state.styles, [marker.style]: true }))
  state.index = end + marker.token.length
  return true
}

export function parseFastMarkdownInline(
  text: InlineMarkdownText,
  styles: FastMarkdownTextStyles = {},
): FastMarkdownInlineItem[] {
  if (isDurableMarkdownToken(text)) return [fastMarkdownTextItem(text, styles)]
  const state: InlineParserState = { index: 0, items: [], styles, text }
  while (state.index < state.text.length) {
    if (appendDurableToken(state)) continue
    if (appendLink(state)) continue
    if (appendCode(state)) continue
    if (appendStyledText(state)) continue
    appendLiteralText(state)
  }
  return state.items
}

function appendLiteralText(state: InlineParserState): void {
  const current = state.text.charAt(state.index)
  if (current !== '\\') {
    appendText(state.items, current, state.styles)
    state.index += 1
    return
  }

  const hasEscapedCharacter = state.index + 1 < state.text.length
  appendText(state.items, hasEscapedCharacter ? state.text.charAt(state.index + 1) : '\\', state.styles)
  state.index += hasEscapedCharacter ? 2 : 1
}
