import { useLayoutEffect, useRef } from 'react'

const STARTUP_SHELL_FALLBACK_NODE_KEY = '__nabuStartupShellFallbackNode'

function startupShellFallbackNode(): Node | null {
  const capturedNode = Reflect.get(window, STARTUP_SHELL_FALLBACK_NODE_KEY)
  if (capturedNode instanceof Node) return capturedNode.cloneNode(true)

  return document.getElementById('nabu-boot-shell')?.cloneNode(true) ?? null
}

export function StartupShellFallback() {
  const containerRef = useRef<HTMLDivElement | null>(null)

  useLayoutEffect(() => {
    const container = containerRef.current
    const shell = startupShellFallbackNode()
    if (!(container && shell)) return
    container.replaceChildren(...Array.from(shell.childNodes, (child) => child.cloneNode(true)))
  }, [])

  return (
    <div
      ref={containerRef}
      className="startup-shell-fallback"
      data-testid="startup-shell-fallback"
      aria-hidden="true"
    />
  )
}
