import { useState } from 'react'
import { CaretRight, CheckCircle, Globe, MagnifyingGlass, StopCircle } from '@phosphor-icons/react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { useDragRegion } from '../hooks/useDragRegion'
import { trackEvent } from '../lib/telemetry'
import { openExternalUrl } from '../utils/url'
import { useDeepResearch } from '../hooks/useDeepResearch'
import { useSettings } from '../hooks/useSettings'
import { AI_AGENT_DEFINITIONS, resolveDefaultAiAgent, type AiAgentId } from '../lib/aiAgents'
import type { AiAgentPermissionMode } from '../lib/aiAgentPermissionMode'
import { MarkdownContent } from './MarkdownContent'

interface DeepResearchPanelProps {
  vaultPath: string
  vaultPaths?: string[]
  permissionMode?: AiAgentPermissionMode
  model?: string
}

const MIN_DEPTH = 1
const MAX_DEPTH = 5

/**
 * Multi-step AI deep research panel (Feature 3). Runs the Claude CLI research
 * loop through the `start_deep_research` IPC command and renders live
 * progress: iterations, scraped sources, interim summaries, and the final
 * markdown report with source citations.
 */
export function DeepResearchPanel({
  vaultPath,
  vaultPaths,
  permissionMode = 'safe',
  model,
}: DeepResearchPanelProps) {
  const [query, setQuery] = useState('')
  const [depth, setDepth] = useState(3)
  const [selectedAgent, setSelectedAgent] = useState<AiAgentId | null>(null)
  const { settings } = useSettings()
  const { state, run, abort, reset } = useDeepResearch()
  const running = state.status === 'running'
  const { dragRegionRef } = useDragRegion<HTMLDivElement>()
  // Default the research loop to the app's configured default agent (e.g. Pi)
  // until the user picks a different one for this session.
  const agent = selectedAgent ?? resolveDefaultAiAgent(settings.default_ai_agent)
  const agentLabel = AI_AGENT_DEFINITIONS.find((definition) => definition.id === agent)?.label ?? agent

  const handleStart = () => {
    const trimmed = query.trim()
    if (!trimmed || running) return
    trackEvent('deep_research_started', { depth, permission_mode: permissionMode, agent })
    void run({
      query: trimmed,
      vaultPath,
      vaultPaths,
      depth,
      model,
      agent,
      permissionMode,
    })
  }

  const handleStop = () => {
    trackEvent('deep_research_stopped')
    abort()
  }

  const handleReset = () => {
    reset()
    setQuery('')
    setDepth(3)
  }

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col" data-testid="deep-research-panel">
      <div ref={dragRegionRef} className="flex flex-col gap-3 border-b border-border p-4">
        <div className="flex items-center gap-2">
          <MagnifyingGlass size={16} className="shrink-0 text-muted-foreground" aria-hidden />
          <h2 className="text-[14px] font-semibold">Deep Research</h2>
        </div>
        <div className="flex gap-2">
          <Input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Research query, e.g. impact of climate change on Mediterranean agriculture"
            onKeyDown={(event) => {
              if (event.key === 'Enter') handleStart()
            }}
            disabled={running}
            data-testid="deep-research-query"
          />
          {running ? (
            <Button type="button" variant="outline" onClick={handleStop} data-testid="deep-research-stop">
              <StopCircle size={15} aria-hidden /> Stop
            </Button>
          ) : (
            <Button type="button" onClick={handleStart} disabled={!query.trim()} data-testid="deep-research-start">
              <CaretRight size={15} aria-hidden /> Research
            </Button>
          )}
        </div>
        <div className="flex items-center gap-3">
          <label className="flex shrink-0 items-center gap-2 text-[12px] text-muted-foreground">
            <span className="shrink-0">Agent</span>
            <Select
              value={agent}
              onValueChange={(value) => setSelectedAgent(value as AiAgentId)}
              disabled={running}
            >
              <SelectTrigger size="sm" className="h-8 w-[140px] text-[12px]" data-testid="deep-research-agent">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {AI_AGENT_DEFINITIONS.map((definition) => (
                  <SelectItem key={definition.id} value={definition.id}>
                    {definition.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </label>
          <label className="flex min-w-0 flex-1 items-center gap-2 text-[12px] text-muted-foreground">
            <span className="shrink-0">Depth</span>
            <input
              type="range"
              min={MIN_DEPTH}
              max={MAX_DEPTH}
              step={1}
              value={depth}
              onChange={(event) => setDepth(Number(event.target.value))}
              disabled={running}
              className="min-w-0 flex-1 accent-[var(--accent-blue)]"
              aria-label="Research depth"
              data-testid="deep-research-depth"
            />
            <span className="w-6 shrink-0 text-right font-medium text-foreground">{depth}</span>
          </label>
          {state.status === 'done' && (
            <Button type="button" variant="ghost" size="xs" onClick={handleReset} data-testid="deep-research-reset">
              New research
            </Button>
          )}
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-4">
        {state.status === 'idle' && (
          <p className="text-[13px] leading-relaxed text-muted-foreground">
            Run a multi-step research session: {agentLabel} scrapes the web, identifies gaps, and
            synthesizes a cited report. Uses the {agentLabel} CLI with Bash/web tools.
          </p>
        )}

        {running && (
          <div className="mb-3 flex items-center gap-2 text-[12px] text-muted-foreground">
            <span className="h-2 w-2 animate-pulse rounded-full bg-[var(--accent-blue)]" />
            {state.iterations.length === 0
              ? 'Starting research…'
              : `Iteration ${state.iterations.length} — gathering sources`}
          </div>
        )}

        {state.iterations.length > 0 && (
          <section className="mb-4" aria-label="Research iterations">
            <h3 className="mb-1.5 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
              Iterations
            </h3>
            <ol className="space-y-1">
              {state.iterations.map((iteration) => (
                <li key={iteration.number} className="flex items-start gap-2 text-[12px]">
                  <CheckCircle size={13} className="mt-0.5 shrink-0 text-[var(--accent-blue)]" aria-hidden />
                  <span className="min-w-0">
                    <span className="font-medium text-foreground">Iteration {iteration.number}</span>
                    <span className="block truncate text-muted-foreground" title={iteration.goal}>
                      {iteration.goal}
                    </span>
                  </span>
                </li>
              ))}
            </ol>
          </section>
        )}

        {state.sources.length > 0 && (
          <section className="mb-4" aria-label="Research sources">
            <h3 className="mb-1.5 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
              Sources ({state.sources.length})
            </h3>
            <ul className="space-y-0.5" data-testid="deep-research-sources">
              {state.sources.map((source, index) => (
                <li key={`${source.url}-${index}`}>
                  <button
                    type="button"
                    className="flex w-full items-center gap-2 rounded-md px-2 py-1 text-left text-[12px] transition-colors hover:bg-[var(--hover)]"
                    title={source.url}
                    onClick={() => void openExternalUrl(source.url)}
                    data-testid={`deep-research-source-${index}`}
                  >
                    <Globe size={13} className="shrink-0 text-muted-foreground" aria-hidden />
                    <span className="min-w-0 flex-1 truncate">{source.title}</span>
                    <span className="shrink-0 text-[11px] text-muted-foreground">open</span>
                  </button>
                </li>
              ))}
            </ul>
          </section>
        )}

        {state.interimSummaries.length > 0 && (
          <section className="mb-4" aria-label="Interim summaries">
            <h3 className="mb-1.5 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
              Interim summaries
            </h3>
            <div className="space-y-2">
              {state.interimSummaries.map((summary, index) => (
                <p key={index} className="rounded-lg bg-[var(--hover)] px-3 py-2 text-[12px] leading-relaxed">
                  {summary}
                </p>
              ))}
            </div>
          </section>
        )}

        {state.report && (
          <section aria-label="Research report" data-testid="deep-research-report">
            <h3 className="mb-1.5 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
              Report
            </h3>
            <div className="rounded-lg border border-border p-4">
              <MarkdownContent content={state.report} />
            </div>
          </section>
        )}

        {state.error && (
          <p className="rounded-lg border border-red-500/30 bg-red-500/5 px-3 py-2 text-[12px] text-red-500">
            {state.error}
          </p>
        )}
      </div>
    </div>
  )
}