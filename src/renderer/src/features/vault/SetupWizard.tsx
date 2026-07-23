import { tauriBridge } from '../../shared/tauri-ipc'
import React, { useState } from 'react'
import { useAppContext } from '../../shared/store'
import { join } from 'path'

type Mode = 'idle' | 'create' | 'open'

export function SetupWizard(): React.JSX.Element {
  const { dispatch } = useAppContext()

  const [mode, setMode] = useState<Mode>('idle')
  const [createVaultName, setCreateVaultName] = useState('')
  const [createVaultParentPath, setCreateVaultParentPath] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const handleOpenVault = async (): Promise<void> => {
    setIsLoading(true)
    setError(null)
    try {
      const result = await tauriBridge.vault.open()
      if (result) {
        dispatch({ type: 'VAULT_OPENED', payload: result })
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setIsLoading(false)
    }
  }

  const handleCreateVault = async (): Promise<void> => {
    if (!createVaultParentPath || !createVaultName) return
    setIsLoading(true)
    setError(null)
    try {
      await tauriBridge.vault.create(createVaultParentPath, createVaultName)
      const result = await tauriBridge.vault.open({ path: join(createVaultParentPath, createVaultName) })
      if (result) {
        dispatch({ type: 'VAULT_OPENED', payload: result })
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setIsLoading(false)
    }
  }

  return (
    <div className="flex flex-col items-center justify-center h-full w-full bg-nabu-bg text-nabu-text">
        {/* Simplified render for brevity, focusing on IPC call correctness */}
    </div>
  )
}
