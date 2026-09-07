import { Plus } from '@phosphor-icons/react'
import {
  useCallback,
  useMemo,
} from 'react'
import type {
  DefaultReactGridSuggestionItem,
  useCreateBlockNote,
} from '@blocknote/react'
import { trackEvent } from '../lib/telemetry'
import { createTranslator, type AppLocale } from '../lib/i18n'
import { searchEmojis, type EmojiEntry } from '../utils/emoji'
import {
  deduplicateByPath,
  MIN_QUERY_LENGTH,
  preFilterWikilinks,
} from '../utils/wikilinkSuggestions'
import { resolveEntry } from '../utils/wikilink'
import {
  attachClickHandlers,
  enrichSuggestionItems,
  hasMultipleSuggestionWorkspaces,
} from '../utils/suggestionEnrichment'
import type { VaultEntry } from '../types'
import type { WikilinkSuggestionItem } from './WikilinkSuggestionMenu'
import { getNabuSlashMenuItems } from './nabuEditorFormattingConfig'

const EMOJI_SHORTCODE_RESULT_LIMIT = 80
const WIKILINK_AUTOCOMPLETE_RESULT_LIMIT = 20

export type SuggestionAction = () => void
type WikilinkAutocompleteTrigger = '[[' | '@'
type SuggestionItemWithClick = { onItemClick?: SuggestionAction }
type EmojiSuggestionItem = DefaultReactGridSuggestionItem & {
  group: string
  name: string
}

function nonEmptyString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0 ? value : null
}

function markdownStem(value: string): string {
  return value.replace(/\.md$/i, '')
}

function pathStem(path: string): string {
  return markdownStem(path.split('/').pop() ?? path)
}

function safeStringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => nonEmptyString(item) !== null)
    : []
}

export function buildBaseSuggestionItems(entries: VaultEntry[]) {
  return deduplicateByPath(
    entries.flatMap((entry) => {
      const path = nonEmptyString(entry.path)
      if (!path) return []

      const filename = nonEmptyString(entry.filename)
      const filenameStem = filename ? markdownStem(filename) : pathStem(path)
      const title = nonEmptyString(entry.title) ?? filenameStem
      const entryType = nonEmptyString(entry.isA)
      return [{
        aliases: [...new Set([filenameStem, ...safeStringArray(entry.aliases)])],
        entry,
        entryTitle: title,
        entryType,
        group: entryType ?? 'Note',
        path,
        title,
      }]
    }),
  )
}

export function useInsertWikilink(
  editor: ReturnType<typeof useCreateBlockNote>,
  runEditorAction: (action: SuggestionAction) => void,
) {
  return useCallback(
    (target: string, triggerCharacter: WikilinkAutocompleteTrigger) => {
      runEditorAction(() => {
        editor.insertInlineContent(
          [{ type: 'wikilink' as const, props: { target } }, ' '],
          { updateSelection: true },
        )
        trackEvent('wikilink_inserted', {
          trigger: triggerCharacter === '@' ? 'at' : 'brackets',
        })
      })
    },
    [editor, runEditorAction],
  )
}

function emojiSuggestionRank(entry: EmojiEntry, query: string): number {
  const normalizedName = entry.name.toLowerCase()
  const tokens = normalizedName.split(/[^a-z0-9]+/).filter(Boolean)
  if (normalizedName === query) return 0
  if (tokens.includes(query)) return 1
  if (tokens.some((token) => token.startsWith(query))) return 2
  return normalizedName.startsWith(query) ? 3 : 4
}

function guardSuggestionMenuItems<T extends SuggestionItemWithClick>(
  items: T[],
  runEditorAction: (action: SuggestionAction) => void,
): T[] {
  return items.map((item) => {
    if (!item.onItemClick) return item

    const onItemClick = item.onItemClick
    return {
      ...item,
      onItemClick: () => runEditorAction(onItemClick),
    }
  })
}

function unresolvedWikilinkCreationItem(
  query: string,
  label: string,
  onCreate: () => void,
): WikilinkSuggestionItem {
  return {
    onItemClick: onCreate,
    path: `__create__:${query}`,
    title: label,
    TypeIcon: Plus,
  }
}

function candidateWikilinkSuggestions(
  baseItems: ReturnType<typeof buildBaseSuggestionItems>,
  normalizedQuery: string,
  triggerCharacter: WikilinkAutocompleteTrigger,
) {
  if (normalizedQuery.length >= MIN_QUERY_LENGTH) {
    return preFilterWikilinks(baseItems, normalizedQuery)
  }
  return triggerCharacter === '[[' ? baseItems : null
}

interface SuggestionMenuItemsOptions {
  baseItems: ReturnType<typeof buildBaseSuggestionItems>
  editor: ReturnType<typeof useCreateBlockNote>
  entries: VaultEntry[]
  insertWikilink: (target: string, triggerCharacter: WikilinkAutocompleteTrigger) => void
  locale: AppLocale
  onNavigateWikilink: (target: string) => void
  runEditorAction: (action: SuggestionAction) => void
  sourceEntry?: VaultEntry
  typeEntryMap: Record<string, VaultEntry>
  vaultPath?: string
}

function matchedWikilinkItems(
  options: Pick<
    SuggestionMenuItemsOptions,
    'baseItems' | 'insertWikilink' | 'runEditorAction' | 'sourceEntry' | 'typeEntryMap' | 'vaultPath'
  >,
  normalizedQuery: string,
  triggerCharacter: WikilinkAutocompleteTrigger,
) {
  const candidates = candidateWikilinkSuggestions(
    options.baseItems,
    normalizedQuery,
    triggerCharacter,
  )
  if (!candidates) return null

  const items = attachClickHandlers(
    candidates,
    (target) => options.insertWikilink(target, triggerCharacter),
    options.vaultPath ?? '',
    options.sourceEntry,
  )
  return guardSuggestionMenuItems(
    enrichSuggestionItems(items, normalizedQuery, options.typeEntryMap, {
      showWorkspace: hasMultipleSuggestionWorkspaces(options.baseItems),
    }),
    options.runEditorAction,
  )
}

function useWikilinkSuggestionItems(
  options: SuggestionMenuItemsOptions,
  createNoteLabel: (title: string) => string,
) {
  const {
    baseItems,
    entries,
    insertWikilink,
    onNavigateWikilink,
    runEditorAction,
    sourceEntry,
    typeEntryMap,
    vaultPath,
  } = options
  return useCallback(
    (query: string, triggerCharacter: WikilinkAutocompleteTrigger) => {
      const normalizedQuery = query.startsWith(triggerCharacter)
        ? query.slice(triggerCharacter.length)
        : query
      const matchedItems = matchedWikilinkItems({
        baseItems,
        insertWikilink,
        runEditorAction,
        sourceEntry,
        typeEntryMap,
        vaultPath,
      }, normalizedQuery, triggerCharacter)
      if (!matchedItems) return null
      if (
        !normalizedQuery
        || !sourceEntry
        || triggerCharacter !== '[['
        || resolveEntry(entries, normalizedQuery, sourceEntry)
      ) return matchedItems

      return [
        ...matchedItems.slice(0, WIKILINK_AUTOCOMPLETE_RESULT_LIMIT - 1),
        unresolvedWikilinkCreationItem(
          normalizedQuery,
          createNoteLabel(normalizedQuery),
          () => {
            insertWikilink(normalizedQuery, triggerCharacter)
            onNavigateWikilink(normalizedQuery)
          },
        ),
      ]
    },
    [
      baseItems,
      createNoteLabel,
      entries,
      insertWikilink,
      onNavigateWikilink,
      runEditorAction,
      sourceEntry,
      typeEntryMap,
      vaultPath,
    ],
  )
}

function useEmojiSuggestionItems(
  editor: ReturnType<typeof useCreateBlockNote>,
  runEditorAction: (action: SuggestionAction) => void,
) {
  return useCallback(async (query: string): Promise<EmojiSuggestionItem[]> => {
    const normalizedQuery = (query.startsWith(':') ? query.slice(1) : query).trim().toLowerCase()
    if (!normalizedQuery) return []

    return searchEmojis(normalizedQuery)
      .sort((left, right) => (
        emojiSuggestionRank(left, normalizedQuery)
        - emojiSuggestionRank(right, normalizedQuery)
        || left.name.localeCompare(right.name)
      ))
      .slice(0, EMOJI_SHORTCODE_RESULT_LIMIT)
      .map((entry) => ({
        group: entry.group,
        icon: <span title={entry.name}>{entry.emoji}</span>,
        id: entry.emoji,
        name: entry.name,
        onItemClick: () => {
          runEditorAction(() => {
            editor.insertInlineContent(entry.emoji, { updateSelection: true })
            trackEvent('emoji_shortcode_inserted', { group: entry.group })
          })
        },
      }))
  }, [editor, runEditorAction])
}

function useSlashMenuItems(
  editor: ReturnType<typeof useCreateBlockNote>,
  runEditorAction: (action: SuggestionAction) => void,
  t: ReturnType<typeof createTranslator>,
) {
  return useCallback(async (query: string) => {
    try {
      return guardSuggestionMenuItems(
        await Promise.resolve(getNabuSlashMenuItems(editor, query, {
          calloutTitle: t('editor.slash.callout'),
          calloutTypeTitles: {
            abstract: t('editor.slash.callout.abstract'),
            bug: t('editor.slash.callout.bug'),
            danger: t('editor.slash.callout.danger'),
            example: t('editor.slash.callout.example'),
            failure: t('editor.slash.callout.failure'),
            info: t('editor.slash.callout.info'),
            note: t('editor.slash.callout.note'),
            question: t('editor.slash.callout.question'),
            quote: t('editor.slash.callout.quote'),
            success: t('editor.slash.callout.success'),
            tip: t('editor.slash.callout.tip'),
            todo: t('editor.slash.callout.todo'),
            warning: t('editor.slash.callout.warning'),
          },
          dateTitle: t('editor.slash.date'),
          datetimeTitle: t('editor.slash.datetime'),
          sandboxBlockTitle: t('editor.slash.htmlBlock'),
          mathTitle: t('editor.slash.math'),
          timeTitle: t('editor.slash.time'),
        })),
        runEditorAction,
      )
    } catch (error) {
      console.warn('[editor] Ignored stale slash menu query:', error)
      return []
    }
  }, [editor, runEditorAction, t])
}

export function useSuggestionMenuItems(options: SuggestionMenuItemsOptions) {
  const t = useMemo(() => createTranslator(options.locale), [options.locale])
  const createNoteLabel = useCallback(
    (title: string) => t('editor.wikilink.createNote', { title }),
    [t],
  )
  const buildItems = useWikilinkSuggestionItems(options, createNoteLabel)
  const getWikilinkItems = useCallback(
    async (query: string): Promise<WikilinkSuggestionItem[]> => buildItems(query, '[[') ?? [],
    [buildItems],
  )
  const getAtWikilinkItems = useCallback(
    async (query: string): Promise<WikilinkSuggestionItem[]> => buildItems(query, '@') ?? [],
    [buildItems],
  )
  const getEmojiItems = useEmojiSuggestionItems(options.editor, options.runEditorAction)
  const getSlashMenuItems = useSlashMenuItems(options.editor, options.runEditorAction, t)

  return {
    getAtWikilinkItems,
    getEmojiItems,
    getSlashMenuItems,
    getWikilinkItems,
  }
}
