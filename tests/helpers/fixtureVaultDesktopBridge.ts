export function installFixtureVaultDesktopBridgeInBrowser(): void {
  const dispatchBrowserMenuCommand =
    window.__laputaTest?.dispatchBrowserMenuCommand
    ?? (() => { throw new Error('Nabu test bridge is missing dispatchBrowserMenuCommand') })

  const specialHandlers: Record<string, (args?: Record<string, unknown>) => unknown> = {
    trigger_menu_command: (args) => {
      dispatchBrowserMenuCommand(String(args?.id ?? ''))
      return null
    },
  }

  const invoke = (command: string, args?: Record<string, unknown>) => {
    const handler = specialHandlers[command] ?? window.__mockHandlers?.[command]
    if (!handler) throw new Error(`Unhandled invoke: ${command}`)
    return handler(args)
  }

  Object.defineProperty(window, '__TAURI__', {
    configurable: true,
    value: {},
  })
  Object.defineProperty(window, '__TAURI_INTERNALS__', {
    configurable: true,
    value: { invoke },
  })
}
