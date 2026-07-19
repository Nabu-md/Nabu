/**
 * SandboxedHtml.tsx
 *
 * Renders user-authored HTML content inside a sandboxed iframe with a
 * postMessage bridge for safe communication with the host app.
 *
 * Security model:
 *   - sandbox="allow-scripts" — scripts run, but the frame sits in a null
 *     origin with no DOM access to the parent, no form submission, no popups,
 *     no top-level navigation.
 *   - No allow-same-origin — prevents the iframe from escaping the sandbox
 *     via Same-Origin Policy tricks.
 *   - No allow-forms — users can't submit data to arbitrary endpoints.
 *   - No allow-popups / allow-top-navigation — the iframe can't navigate away
 *     or open windows.
 *   - Communication via window.postMessage — the iframe calls a whitelisted
 *     set of APIs defined by the parent message handler.
 *
 * Requirements: 12.1, 12.2
 *
 * ponytail: height cap at 400px + internal scroll prevents notes from
 *           hijacking the viewport. Full-screen button for dense dashboards.
 *           Asset bridge uses postMessage → IPC → base64 data URI so the
 *           null-origin iframe can render local images.
 */

import { useRef, useMemo, useEffect, useCallback, useState } from 'react'

export interface SandboxedHtmlProps {
  /** Raw HTML content to render */
  html: string
  /** Max height in px before internal scroll. Default 400. */
  maxHeight?: number
  /** Optional class name for the wrapper. */
  className?: string
}

// ── postMessage protocol ────────────────────────────────────────────────────

type NabuApiMethod = 'readNote' | 'search' | 'getTheme' | 'getLocalAsset'

type NabuApiRequest = {
  id: string
  method: NabuApiMethod
  args?: unknown[]
}

type NabuApiResponse = {
  id: string
  result?: unknown
  error?: string
}

// ── Bridge script injected into every sandboxed document ────────────────────

const BRIDGE_SCRIPT = `
<script>
(function() {
  if (window.__nabuBridge) return;
  window.__nabuBridge = true;

  var pending = {};
  var nextId = 0;

  function call(method) {
    var args = Array.prototype.slice.call(arguments, 1);
    var id = ++nextId;
    return new Promise(function (resolve, reject) {
      pending[id] = { resolve: resolve, reject: reject };
      parent.postMessage({ id: id, method: method, args: args }, '*');
    });
  }

  window.nabu = {
    readNote:     function (p) { return call('readNote', p); },
    search:       function (q) { return call('search', q); },
    getTheme:     function ()  { return call('getTheme'); },
    getLocalAsset:function (p) { return call('getLocalAsset', p); },
  };

  parent.postMessage({ type: 'nabu-ready' }, '*');
})();
</script>`

// ── Internal styles injected into every sandboxed document ──────────────────

const INTERNAL_STYLES = `
<style>
  *, *::before, *::after { box-sizing: border-box; }
  html { height: 100%; }
  body {
    margin: 0;
    padding: 8px;
    overflow-y: auto;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto,
                 Oxygen, Ubuntu, Cantarell, sans-serif;
    line-height: 1.5;
    color-scheme: light dark;
  }
  img { max-width: 100%; height: auto; }
  pre { overflow-x: auto; white-space: pre-wrap; word-break: break-word; }
</style>`

// ── Helpers ─────────────────────────────────────────────────────────────────

const BODY_RE = /<body[^>]*>([\s\S]*)<\/body>/i
const HTML_RE = /<html[\s\S]*<\/html>/i

/** Inject the bridge + styles into an HTML document string. */
function injectProxies(doc: string): string {
  let result = doc
  // Inject before </head>
  if (/<\/head>/i.test(result)) {
    result = result.replace(/<\/head>/i, BRIDGE_SCRIPT + '\n' + INTERNAL_STYLES + '\n</head>')
  } else {
    // No head tag — prepend both
    result = BRIDGE_SCRIPT + '\n' + INTERNAL_STYLES + '\n' + result
  }
  return result
}

/** Wrap a content snippet in a minimal document with proxies. */
function wrapDocument(content: string): string {
  return `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
${BRIDGE_SCRIPT}
${INTERNAL_STYLES}
</head>
<body>${content}</body>
</html>`
}

// ── Component ───────────────────────────────────────────────────────────────

export function SandboxedHtml({ html, maxHeight = 400, className = '' }: SandboxedHtmlProps) {
  const iframeRef = useRef<HTMLIFrameElement>(null)
  const [expanded, setExpanded] = useState(false)

  // Build the srcdoc with bridge + styles injected
  const srcdoc = useMemo(() => {
    if (HTML_RE.test(html)) {
      return injectProxies(html)
    }
    const bodyMatch = html.match(BODY_RE)
    const content = bodyMatch ? bodyMatch[1] : html
    return wrapDocument(content)
  }, [html])

  // ── postMessage handler ─────────────────────────────────────────────────

  const handleMessage = useCallback((event: MessageEvent) => {
    const data = event.data as Record<string, unknown>
    if (!data || typeof data !== 'object') return
    if (data.type === 'nabu-ready') return // just a signal, ignore

    if (typeof data.method !== 'string') return

    const { id, method, args } = data as unknown as NabuApiRequest
    if (!id) return

    const respond = (result: unknown, error?: string) => {
      const response: NabuApiResponse = { id, result, error }
      try {
        if (iframeRef.current?.contentWindow) {
          iframeRef.current.contentWindow.postMessage(response, '*')
        }
      } catch {
        // iframe gone — ignore
      }
    }

    switch (method) {
      case 'readNote': {
        const notePath = args?.[0] as string | undefined
        if (!notePath) {
          respond(null, 'No path provided')
          return
        }
        window.electron.note
          .getRaw(notePath)
          .then((result: { content?: string; error?: string }) => {
            if (result.error) {
              respond(null, result.error)
            } else {
              respond(result.content ?? '')
            }
          })
          .catch((err: Error) => respond(null, err.message))
        break
      }

      case 'search': {
        const query = args?.[0] as string | undefined
        if (!query) {
          respond(null, 'No query provided')
          return
        }
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        window.electron.search.query(query).then((result: any) => {
          respond(result?.results ?? [])
        }).catch((err: Error) => respond(null, err.message))
        break
      }

      case 'getTheme':
        respond(document.documentElement.getAttribute('data-theme') ?? 'system')
        break

      case 'getLocalAsset': {
        const assetPath = args?.[0] as string | undefined
        if (!assetPath) {
          respond(null, 'No path provided')
          return
        }
        // Read the file via the Electron IPC bridge and return as a data URI
        window.electron.file
          .readAsset(assetPath)
          .then((result: { dataUri?: string; error?: string }) => {
            if (result.error) {
              respond(null, result.error)
            } else {
              respond(result.dataUri)
            }
          })
          .catch((err: Error) => respond(null, err.message))
        break
      }

      default:
        // Ignore unknown methods — safety
        break
    }
  }, [])

  useEffect(() => {
    window.addEventListener('message', handleMessage)
    return () => window.removeEventListener('message', handleMessage)
  }, [handleMessage])

  // ── Iframe auto-resize (capped unless expanded) ─────────────────────────

  const handleLoad = useCallback(() => {
    if (!iframeRef.current) return
    try {
      const body = iframeRef.current.contentDocument?.body
      const docEl = iframeRef.current.contentDocument?.documentElement
      const height = Math.max(body?.scrollHeight ?? 0, docEl?.scrollHeight ?? 0)
      if (height > 0) {
        const cap = expanded ? height : Math.min(height, maxHeight)
        iframeRef.current.style.height = `${cap}px`
      }
    } catch {
      // Cross-origin errors expected for sandboxed iframes — ignore
    }
  }, [maxHeight, expanded])

  // Re-size when expanded changes
  useEffect(() => {
    handleLoad()
  }, [expanded, handleLoad])

  // ── Render ──────────────────────────────────────────────────────────────

  const toggleExpanded = useCallback(() => setExpanded((v) => !v), [])

  return (
    <div
      className={`nabu-sandbox-html my-3 rounded-lg overflow-hidden border border-white/10 relative group ${className}`}
    >
      <iframe
        ref={iframeRef}
        sandbox="allow-scripts"
        srcDoc={srcdoc}
        title="Sandboxed HTML app block"
        className="w-full"
        style={{
          border: 'none',
          height: `${Math.min(200, maxHeight)}px`
        }}
        onLoad={handleLoad}
      />

      {/* Expand / collapse button — visible on hover */}
      <button
        onClick={toggleExpanded}
        className={
          'absolute top-1 right-1 px-2 py-0.5 text-xs rounded ' +
          'bg-white/10 hover:bg-white/20 text-white/60 hover:text-white/90 ' +
          'opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer'
        }
        title={expanded ? 'Collapse' : 'Expand to full height'}
      >
        {expanded ? '▾ Collapse' : '▴ Expand'}
      </button>
    </div>
  )
}
