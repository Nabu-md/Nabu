import { useCallback, useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { isTauri, mockInvoke } from '../mock-tauri'

export interface BuzzMessage {
  author: string
  content: string
  created_at: number
}

export interface BuzzStatus {
  installed: boolean
  has_identity: boolean
  env_key_present: boolean
}

export type BuzzReadiness = 'checking' | 'not_installed' | 'no_identity' | 'ready'

export interface BuzzMultiplayerState {
  readiness: BuzzReadiness
  status: BuzzStatus | null
  teamMessages: BuzzMessage[]
  /** Most recent non-empty channel id used with the relay. */
  lastChannel: string | null
  refresh: (channel: string) => Promise<void>
  postUpdate: (channel: string, status: string) => Promise<void>
}

function buzzInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return isTauri() ? invoke<T>(command, args) : mockInvoke<T>(command, args)
}

/**
 * Buzz multiplayer state: detects the `buzz` CLI and identity keypair,
 * fetches recent team-channel messages for agent context, and posts agent
 * status updates to the relay. Channel ids never leave the local machine
 * except through the user-configured relay.
 */
export function useBuzzMultiplayer(): BuzzMultiplayerState {
  const [status, setStatus] = useState<BuzzStatus | null>(null)
  const [teamMessages, setTeamMessages] = useState<BuzzMessage[]>([])
  const [lastChannel, setLastChannel] = useState<string | null>(null)

  const readiness: BuzzReadiness = !status
    ? 'checking'
    : !status.installed
      ? 'not_installed'
      : !status.has_identity && !status.env_key_present
        ? 'no_identity'
        : 'ready'

  useEffect(() => {
    let cancelled = false
    buzzInvoke<BuzzStatus>('buzz_status')
      .then((next) => {
        if (!cancelled) setStatus(next)
      })
      .catch(() => {
        if (!cancelled)
          setStatus({ installed: false, has_identity: false, env_key_present: false })
      })
    return () => {
      cancelled = true
    }
  }, [])

  const refresh = useCallback(async (channel: string) => {
    const trimmed = channel.trim()
    if (!trimmed) return
    setLastChannel(trimmed)
    try {
      const messages = await buzzInvoke<BuzzMessage[]>('buzz_get_team_messages', {
        channel: trimmed,
        limit: 10,
      })
      setTeamMessages(messages)
    } catch {
      setTeamMessages([])
    }
  }, [])

  const postUpdate = useCallback(async (channel: string, message: string) => {
    await buzzInvoke('buzz_post_agent_update', { channel: channel.trim(), status: message })
  }, [])

  return { readiness, status, teamMessages, lastChannel, refresh, postUpdate }
}
