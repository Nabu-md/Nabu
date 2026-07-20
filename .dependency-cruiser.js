/**
 * dependency-cruiser configuration for Nabu architecture enforcement
 *
 * This configuration enforces the layer ownership rules defined in ADR-005:
 *   - Renderer may only import from shared (types/schemas/contracts)
 *   - Preload may only import from shared
 *   - Main may import from services and shared
 *   - Services may import from shared and other services
 *   - Shared may NOT import from main, renderer, or electron
 */

module.exports = {
  forbidden: [
    {
      name: 'no-renderer-to-main',
      from: { path: '^src/renderer' },
      to: { path: '^src/main', depth: 0 },
      severity: 'error'
    },
    {
      name: 'no-renderer-to-electron',
      from: { path: '^src/renderer' },
      to: { path: '^electron', depth: 0 },
      severity: 'error'
    },
    {
      name: 'no-renderer-to-node-api',
      from: { path: '^src/renderer' },
      to: {
        path: '^(fs|path|os|child_process|net|http|https|crypto|stream|buffer|util|events|worker_threads)$',
        depth: 0
      },
      severity: 'error'
    },
    {
      name: 'no-preload-to-main-logic',
      from: { path: '^src/preload' },
      to: { path: '^src/main/(?!index\\.d\\.ts)', depth: 0 },
      severity: 'error'
    },
    {
      name: 'no-shared-to-electron',
      from: { path: '^src/shared' },
      to: { path: '^electron', depth: 0 },
      severity: 'error'
    },
    {
      name: 'no-shared-to-react',
      from: { path: '^src/shared' },
      to: { path: '^react', depth: 0 },
      severity: 'error'
    },
    {
      name: 'no-services-to-renderer',
      from: { path: '^src/main/services' },
      to: { path: '^src/renderer', depth: 0 },
      severity: 'error'
    },
    {
      name: 'no-ipc-to-renderer',
      from: { path: '^src/main/ipc' },
      to: { path: '^src/renderer', depth: 0 },
      severity: 'error'
    }
  ],
  options: {
    tsConfig: {
      fileName: 'tsconfig.json'
    },
    doNotFollow: {
      path: ['^node_modules']
    },
    exclude: '^(node_modules|\\.git|dist|out)'
  }
}
