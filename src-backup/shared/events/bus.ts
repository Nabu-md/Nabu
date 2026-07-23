/**
 * bus.ts
 *
 * A lightweight, strongly typed event bus for asynchronous *internal*
 * workflows. It is intentionally independent of Electron, React, and any UI
 * component so it can be consumed by the main process, services, and (if ever
 * needed) the renderer without creating layer coupling.
 *
 * Design goals (Phase 1.5 — Event Bus & Layer Enforcement):
 *  - Strongly typed event names (keys of the `Events` map).
 *  - Strongly typed payloads (value type per event key).
 *  - Minimal surface: `publish`, `subscribe`, `unsubscribe`, `once`.
 *  - No dependency on Node `events`, Electron, or React.
 *  - Synchronous dispatch (callers control async via `Promise`/`async`).
 *
 * The event bus is NOT a replacement for ordinary function calls or for the
 * typed IPC layer. It is used only for background, decoupled notifications
 * between services on the main process.
 */

/** A listener invoked with the strongly typed payload of event `K`. */
export type EventListener<K extends keyof Events, Events> = (payload: Events[K]) => void

/**
 * The contract every event map must satisfy: a record from event name to
 * payload type. A plain object type (not requiring an index signature) is
 * used so concrete event maps declared as `interface` — such as `AppEvents` —
 * still satisfy the constraint without forcing an index signature.
 */
export type EventMap = { [K: string]: unknown }

/**
 * A strongly typed publish/subscribe event bus.
 *
 * @typeParam Events - A map of event name → payload type.
 */
export class EventBus<Events extends EventMap> {
  private readonly listeners = new Map<keyof Events, Set<EventListener<keyof Events, Events>>>()

  /**
   * Publish an event to all current subscribers.
   *
   * Listeners are invoked synchronously in registration order. A throwing
   * listener does not prevent other listeners from running; the error is
   * re-thrown after all listeners have been notified so the publisher is
   * still aware of the failure.
   */
  publish<K extends keyof Events>(event: K, payload: Events[K]): void {
    const set = this.listeners.get(event)
    if (!set || set.size === 0) return

    let firstError: unknown
    for (const listener of set) {
      try {
        ;(listener as EventListener<K, Events>)(payload)
      } catch (err) {
        if (firstError === undefined) firstError = err
      }
    }
    if (firstError !== undefined) throw firstError
  }

  /**
   * Subscribe to an event. Returns an unsubscribe function for convenience.
   */
  subscribe<K extends keyof Events>(event: K, listener: EventListener<K, Events>): () => void {
    let set = this.listeners.get(event)
    if (!set) {
      set = new Set()
      this.listeners.set(event, set)
    }
    set.add(listener as EventListener<keyof Events, Events>)
    return () => this.unsubscribe(event, listener)
  }

  /**
   * Remove a previously registered listener. Safe to call with a listener
   * that was never registered.
   */
  unsubscribe<K extends keyof Events>(event: K, listener: EventListener<K, Events>): void {
    const set = this.listeners.get(event)
    if (!set) return
    set.delete(listener as EventListener<keyof Events, Events>)
    if (set.size === 0) this.listeners.delete(event)
  }

  /**
   * Subscribe to an event exactly once. The listener is removed after the
   * first invocation.
   */
  once<K extends keyof Events>(event: K, listener: EventListener<K, Events>): () => void {
    const wrapper: EventListener<K, Events> = (payload) => {
      this.unsubscribe(event, wrapper)
      listener(payload)
    }
    return this.subscribe(event, wrapper)
  }

  /**
   * Remove all listeners for a given event, or every listener if no event is
   * supplied. Primarily useful for tests and process teardown.
   */
  clear(event?: keyof Events): void {
    if (event === undefined) this.listeners.clear()
    else this.listeners.delete(event)
  }
}
