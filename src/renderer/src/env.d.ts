import { ipc as bridge } from './shared/ipc'

declare global {
  interface Window {
    ipc: typeof bridge
  }
}
