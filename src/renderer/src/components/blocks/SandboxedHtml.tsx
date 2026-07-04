/**
 * SandboxedHtml.tsx
 *
 * Renders user-authored HTML content inside a sandboxed iframe.
 *
 * Security model:
 *   - iframe sandbox="allow-scripts" — scripts run, but the frame is in a
 *     null origin with no DOM access to the parent, no form submission, no
 *     popups, no top-level navigation, no same-origin access.
 *   - No allow-same-origin — prevents the iframe from escaping the sandbox
 *     via Same-Origin Policy tricks.
 *   - No allow-forms — users can't submit data to arbitrary endpoints.
 *   - No allow-popups / allow-top-navigation — the iframe can't navigate away
 *     or open windows.
 *   - communication via window.postMessage — the iframe can call a safe,
 *     curated set of APIs defined in the parent listener.
 *
 * Requirements: 12.1 (sandbox isolation), 12.2 (postMessage bridge)
 */

import { useRef, useMemo, useEffect, useCallback } from 'react'

export interface SandboxedHtmlProps {
  /** Raw HTML content to render */
  html: string
  /** Optional max height before scroll. Defaults to 400px. */
  maxHeight?: number
  /** Optional className for the wrapper */
  className?: string
}

// Safe API surface exposed to sandboxed iframes via postMessage
type NemoApiRequest = {
  id: string
  method: 'readNote' | 'search' | 'getTheme'
  args?: unknown[]
}

type NemoApiResponse = {
  id: string
  result?: unknown
  error?: string
}

export function SandboxedHtml({
  html,
  maxHeight = 400,
  className = '',
}: SandboxedHtmlProps) {
  const iframeRef = useRef<HTMLIFrameElement>(null)

  // Build the srcdoc: inject a small bridge script that exposes a safe
  // `window.nemo` API using postMessage.
  const srcdoc = useMemo(() => {
    const bridgeScript = `
<script>
  // Secure bridge — exposes a controlled API to sandboxed HTML apps
  window.nemo = {
    _pending: {},
    _id: 0,
    _call(method, ...args) {
      const id = ++this._id;
      return new Promise((resolve, reject) => {
        this._pending[id] = { resolve, reject };
        parent.postMessage({ id, method, args }, '*');
      });
    },
    readNote(path) { return this._call('readNote', path); },
    search(query)  { return this._call('search', query); },
    getTheme()     { return this._call('getTheme'); },
  };
  // Signal readiness
  parent.postMessage({ type: 'nemo-ready' }, '*');
</script>`
    // Extract content between <body> tags if present, otherwise wrap the whole
    // thing in a minimal document so scripts execute properly.
    const bodyMatch = html.match(/<body[^>]*>([\s\S]*)<\/body>/i)
    const content = bodyMatch ? bodyMatch[1] : html

    // If the user already provided a full document, inject the bridge before
    // </head>. Otherwise wrap it in a minimal document.
    if (/<html[\s\S]*<\/html>/i.test(html)) {
      return html.replace('</head>', bridgeScript + '\n</head>')
    }

    return `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<style>
  body {
    margin: 0;
    padding: 8px;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  }
</style>
${bridgeScript}
</head>
<body>${content}</body>
</html>`
  }, [html])

  // Listen for postMessage from the iframe
  const handleMessage = useCallback((event: MessageEvent) => {
    const data = event.data as Record<string, unknown>
    if (!data || typeof data !== 'object') return

    // Handle API calls from the sandboxed iframe
    if (data.method === 'readNote' || data.method === 'search' || data.method === 'getTheme') {
      const { id, method, args } = data as unknown as NemoApiRequest

      // Forward to the Electron main process via IPC
      const respond = (result: unknown, error?: string) => {
        const response: NemoApiResponse = { id, result, error }
        if (iframeRef.current?.contentWindow) {
          iframeRef.current.contentWindow.postMessage(response, '*')
        }
      }

      // Map to Electron IPC calls
      switch (method) {
        case 'readNote':
          // TODO: Implement readNote API when the mini-app ecosystem requires it
          respond(null, 'Not yet implemented')
          break
        case 'search':
          // TODO: Implement search API
          respond(null, 'Not yet implemented')
          break
        case 'getTheme':
          // Return whether the app is in dark mode
          respond(document.documentElement.getAttribute('data-theme') ?? 'system')
          break
      }
    }
  }, [])

  useEffect(() => {
    window.addEventListener('message', handleMessage)
    return () => window.removeEventListener('message', handleMessage)
  }, [handleMessage])

  // Auto-resize iframe to content height (within maxHeight bounds)
  const handleLoad = useCallback(() => {
    if (!iframeRef.current) return
    try {
      const height = iframeRef.current.contentDocument?.documentElement?.scrollHeight
      if (height && height > 0) {
        iframeRef.current.style.height = `${Math.min(height, maxHeight)}px`
      }
    } catch {
      // Cross-origin errors are expected if the iframe navigates — ignore
    }
  }, [maxHeight])

  return (
    <div className={`nemo-sandbox-html my-3 rounded-lg overflow-hidden border border-white/10 ${className}`}>
      <iframe
        ref={iframeRef}
        sandbox="allow-scripts"
        srcDoc={srcdoc}
        title="Sandboxed HTML app block"
        className="w-full"
        style={{
          border: 'none',
          height: '200px', // will be resized on load
        }}
        onLoad={handleLoad}
      />
    </div>
  )
}
