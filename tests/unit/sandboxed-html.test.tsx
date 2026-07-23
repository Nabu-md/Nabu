import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { SandboxedHtml } from '@/shared/components/SandboxedHtml'

describe('SandboxedHtml - Tauri WebView readiness', () => {
  it('renders an iframe with strict sandbox flags and no Chromium shims', () => {
    render(<SandboxedHtml html="<p>App Block</p>" />)
    const iframe = screen.getByTitle('Sandboxed HTML app block')
    expect(iframe).toBeInstanceOf(HTMLIFrameElement)
    expect(iframe.getAttribute('sandbox')).toBe('allow-scripts')
    expect(iframe.hasAttribute('allow-same-origin')).toBe(false)
    expect(iframe.hasAttribute('allow-forms')).toBe(false)
    expect(iframe.hasAttribute('allow-popups')).toBe(false)
    expect(iframe.hasAttribute('allow-top-navigation')).toBe(false)
  })

  it('loads the bridge script for postMessage-based API access', () => {
    render(<SandboxedHtml html="<p>App Block</p>" />)
    const iframe = screen.getByTitle('Sandboxed HTML app block')
    const srcdoc = iframe.getAttribute('srcdoc') ?? ''
    expect(srcdoc).toContain('addEventListener(\'message\'')
    expect(srcdoc).toContain('NabuApi')
  })

  it('height measurement tolerates null-origin cross-origin access', () => {
    render(<SandboxedHtml html="<p>App Block</p>" maxHeight={200} />)
    const iframe = screen.getByTitle('Sandboxed HTML app block')
    expect(iframe.style.height).toBe('200px')
  })

  it('does not depend on Electron globals', () => {
    const src = require('fs').readFileSync(
      require('path').join(process.cwd(), 'src/renderer/src/shared/components/SandboxedHtml.tsx'),
      'utf8'
    )
    expect(src).not.toMatch(/require\('electron'\)/)
    expect(src).not.toMatch(/window\.electron/)
    expect(src).not.toMatch(/@electron-toolkit/)
  })
})
