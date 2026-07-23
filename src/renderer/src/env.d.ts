import { ipc as bridge } from "@renderer-shared/ipc"

declare global {
  interface Window {
    ipc: typeof bridge
  }
}
