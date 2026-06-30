/**
 * Property-based tests for vector index error handling (Task 23.4)
 *
 * Property 47: Missing ONNX Model Graceful Degradation
 *   - When BGE-micro ONNX model files are absent, embeddings are disabled
 *   - An activity:log error is emitted
 *   - embedFile() is silently skipped
 *
 * Property 48: Vector Index Corruption Recovery
 *   - When the Vectra index fails to load/parse, a rebuild is triggered
 *   - A warning is emitted on activity:log with the reason
 *
 * Validates: Requirements 9.8, 9.9
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import * as fc from 'fast-check';
import { mkdtemp, rm } from 'fs/promises';
import { tmpdir } from 'os';
import { join } from 'path';

// ---------------------------------------------------------------------------
// Shared state that the vectra mock reads at call time
// ---------------------------------------------------------------------------

// Controls per-test vectra behaviour. Reset in beforeEach.
const vectraState = {
  isIndexCreated: true as boolean,
  getIndexStatsError: null as Error | null,
  createIndexError: null as Error | null,
  deleteIndexCalls: 0,
  createIndexCalls: 0,
  upsertItemCalls: 0,
};

// ── vectra mock (class constructor) ─────────────────────────────────────
vi.mock('vectra', () => {
  class LocalIndex {
    async isIndexCreated() { return vectraState.isIndexCreated; }
    async getIndexStats() {
      if (vectraState.getIndexStatsError) throw vectraState.getIndexStatsError;
      return { items: 0 };
    }
    async createIndex(_opts: unknown) {
      vectraState.createIndexCalls++;
      if (vectraState.createIndexError) throw vectraState.createIndexError;
    }
    async deleteIndex() { vectraState.deleteIndexCalls++; }
    async upsertItem(_item: unknown) { vectraState.upsertItemCalls++; }
    async queryItems(_vec: unknown, _text: unknown, _limit: unknown) { return []; }
  }
  return { LocalIndex };
});

// ── @xenova/transformers mock ─────────────────────────────────────────────
// Controls whether pipeline() succeeds or fails.
const xenovaState = {
  pipelineError: null as Error | null,
};

vi.mock('@xenova/transformers', () => {
  const env = { localModelPath: '', allowRemoteModels: true };
  async function pipeline(_task: unknown, _model: unknown) {
    if (xenovaState.pipelineError) throw xenovaState.pipelineError;
    // Return a mock embedder function
    return async (_text: unknown, _opts: unknown) => ({
      data: new Float32Array(384).fill(0.1),
    });
  }
  return { pipeline, env };
});

// ---------------------------------------------------------------------------
// Module under test (imported after mocks are registered)
// ---------------------------------------------------------------------------
import { VectorManager } from '@main/vector';

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), 'nemo-vector-'));
  // Reset all shared state
  Object.assign(vectraState, {
    isIndexCreated: true,
    getIndexStatsError: null,
    createIndexError: null,
    deleteIndexCalls: 0,
    createIndexCalls: 0,
    upsertItemCalls: 0,
  });
  xenovaState.pipelineError = null;
});

afterEach(async () => {
  try {
    await rm(tmpDir, { recursive: true, force: true });
  } catch {
    // best-effort
  }
});

/** Initialize a VectorManager and capture all log calls. */
async function initVMWithLogs(overrides: { modelPath?: string; indexPath?: string } = {}) {
  const logs: { level: string; message: string }[] = [];
  const vm = new VectorManager();
  vm.setLogCallback((level, message) => logs.push({ level, message }));
  await vm.initialize({
    indexPath: overrides.indexPath ?? tmpDir,
    modelPath: overrides.modelPath ?? join(tmpDir, 'models'),
  });
  return { vm, logs };
}

// ---------------------------------------------------------------------------
// Property 47 — Missing ONNX Model Graceful Degradation (Requirement 9.8)
// ---------------------------------------------------------------------------
/**Validates: Requirements 9.8 */
describe('Property 47 — Missing ONNX Model Graceful Degradation (Req 9.8)', () => {
  it('embeddings are disabled and activity:log error is emitted when model load fails', async () => {
    await fc.assert(
      fc.asyncProperty(
        fc.oneof(
          fc.constant('ENOENT: no such file or directory'),
          fc.constant('Model not found at path'),
          fc.constant('Failed to load ONNX model'),
          fc.constant('Cannot read properties of undefined'),
        ),
        async (errorMsg) => {
          xenovaState.pipelineError = new Error(errorMsg);

          const { vm, logs } = await initVMWithLogs();

          // Must emit an error-level log referencing model/embedding/onnx
          const errorLog = logs.find(
            (l) =>
              l.level === 'error' &&
              (l.message.toLowerCase().includes('model') ||
                l.message.toLowerCase().includes('embed') ||
                l.message.toLowerCase().includes('onnx')),
          );
          expect(errorLog).toBeDefined();

          // embedFile() must be a no-op (not throw)
          expect(() => vm.embedFile('/vault/note.md', 'Some text')).not.toThrow();

          // search() must return empty array, not throw
          const results = await vm.search('query text', 5);
          expect(results).toEqual([]);

          // Reset for next fc run
          xenovaState.pipelineError = null;
        },
      ),
      { numRuns: 4, seed: 42 },
    );
  });

  it('embedFile is a no-op when embeddings are disabled (model missing)', async () => {
    xenovaState.pipelineError = new Error('ENOENT: bge-micro-v2 not found');
    const upsertBefore = vectraState.upsertItemCalls;

    const { logs } = await initVMWithLogs();

    // Confirm embeddings disabled via error log
    expect(logs.some((l) => l.level === 'error')).toBe(true);

    // upsertItem must never be called since embedFile is skipped
    // We need to wait for any async queue drain
    await new Promise((r) => setTimeout(r, 20));
    expect(vectraState.upsertItemCalls).toBe(upsertBefore);
  });

  it('activity:log error message references model unavailability (property over error types)', async () => {
    await fc.assert(
      fc.asyncProperty(
        fc.oneof(
          fc.constant('ENOENT: no such file'),
          fc.constant('Failed to fetch'),
          fc.constant('Cannot resolve module'),
          fc.constant('model files not found'),
        ),
        async (errorMsg) => {
          xenovaState.pipelineError = new Error(errorMsg);

          const { logs } = await initVMWithLogs();

          const errorLogs = logs.filter((l) => l.level === 'error');
          expect(errorLogs.length).toBeGreaterThan(0);

          const disabledLog = errorLogs.find(
            (l) =>
              l.message.toLowerCase().includes('embed') ||
              l.message.toLowerCase().includes('model') ||
              l.message.toLowerCase().includes('disabled'),
          );
          expect(disabledLog).toBeDefined();

          xenovaState.pipelineError = null;
        },
      ),
      { numRuns: 4, seed: 99 },
    );
  });
});

// ---------------------------------------------------------------------------
// Property 48 — Vector Index Corruption Recovery (Requirement 9.9)
// ---------------------------------------------------------------------------
/**Validates: Requirements 9.9 */
describe('Property 48 — Vector Index Corruption Recovery (Req 9.9)', () => {
  it('rebuild is triggered and activity:log warning emitted when index is corrupted', async () => {
    await fc.assert(
      fc.asyncProperty(
        fc.oneof(
          fc.constant('Unexpected token in JSON'),
          fc.constant('ENOENT: index file missing'),
          fc.constant('Invalid index format'),
          fc.constant('Checksum mismatch'),
          fc.constant('Cannot read property of null'),
        ),
        async (corruptionMsg) => {
          vectraState.isIndexCreated = true;
          vectraState.getIndexStatsError = new Error(corruptionMsg);
          vectraState.deleteIndexCalls = 0;
          vectraState.createIndexCalls = 0;

          const { logs } = await initVMWithLogs();

          // Allow background rebuild to run
          await new Promise((r) => setTimeout(r, 20));

          // Must emit a warn-level log mentioning corruption/rebuild/index
          const warnLog = logs.find(
            (l) =>
              l.level === 'warn' &&
              (l.message.toLowerCase().includes('corrupt') ||
                l.message.toLowerCase().includes('rebuild') ||
                l.message.toLowerCase().includes('index')),
          );
          expect(warnLog).toBeDefined();

          // deleteIndex + createIndex must have been called (rebuild happened)
          expect(vectraState.deleteIndexCalls).toBeGreaterThan(0);
          expect(vectraState.createIndexCalls).toBeGreaterThanOrEqual(1);

          // Reset
          vectraState.getIndexStatsError = null;
          vectraState.deleteIndexCalls = 0;
          vectraState.createIndexCalls = 0;
        },
      ),
      { numRuns: 5, seed: 42 },
    );
  });

  it('warning message includes the reason for the rebuild', async () => {
    const corruptionReasons = [
      'SyntaxError: Unexpected end of JSON input',
      'ENOENT: index.json not found',
      'Failed to parse index header',
    ];

    for (const reason of corruptionReasons) {
      vectraState.getIndexStatsError = new Error(reason);
      vectraState.deleteIndexCalls = 0;
      vectraState.createIndexCalls = 0;

      const { logs } = await initVMWithLogs();
      await new Promise((r) => setTimeout(r, 20));

      const warnLog = logs.find((l) => l.level === 'warn');
      expect(warnLog).toBeDefined();

      // The corruption reason keyword should appear in the warning
      const reasonKeyword = reason.split(':')[0]; // e.g. "SyntaxError"
      expect(warnLog!.message).toContain(reasonKeyword);

      vectraState.getIndexStatsError = null;
    }
  });

  it('index rebuild runs as background task (initialize() resolves without waiting for rebuild)', async () => {
    // Simulate slow deleteIndex to confirm non-blocking behaviour
    let deleteResolved = false;
    vectraState.isIndexCreated = true;
    vectraState.getIndexStatsError = new Error('corrupted');

    // Override deleteIndex to be slow — patch after mock setup
    const { LocalIndex } = await import('vectra');
    const proto = LocalIndex.prototype as Record<string, unknown>;
    const originalDelete = proto['deleteIndex'] as () => Promise<void>;
    proto['deleteIndex'] = async function () {
      await new Promise<void>((resolve) =>
        setTimeout(() => {
          deleteResolved = true;
          resolve();
        }, 80),
      );
      vectraState.deleteIndexCalls++;
    };

    const logs: { level: string; message: string }[] = [];
    const vm = new VectorManager();
    vm.setLogCallback((level, message) => logs.push({ level, message }));

    await vm.initialize({ indexPath: tmpDir, modelPath: join(tmpDir, 'models') });

    // initialize() must have returned (the rebuild runs in background)
    const warnLog = logs.find((l) => l.level === 'warn');
    expect(warnLog).toBeDefined();

    // Allow the background rebuild to finish
    await new Promise((r) => setTimeout(r, 120));
    expect(deleteResolved).toBe(true);

    // Restore
    proto['deleteIndex'] = originalDelete;
    vectraState.getIndexStatsError = null;
  });

  it('corruption recovery always emits warn log (property over arbitrary corruption details)', async () => {
    await fc.assert(
      fc.asyncProperty(
        fc.string({ minLength: 5, maxLength: 80 }).filter((s) => s.trim().length > 4),
        async (corruptionDetail) => {
          vectraState.getIndexStatsError = new Error(corruptionDetail);
          vectraState.deleteIndexCalls = 0;
          vectraState.createIndexCalls = 0;

          const { logs } = await initVMWithLogs();
          await new Promise((r) => setTimeout(r, 20));

          // Always emits a warn log on corruption
          const warnLogs = logs.filter((l) => l.level === 'warn');
          expect(warnLogs.length).toBeGreaterThan(0);

          // Always attempts to rebuild
          expect(vectraState.deleteIndexCalls).toBeGreaterThan(0);

          vectraState.getIndexStatsError = null;
        },
      ),
      { numRuns: 10, seed: 17 },
    );
  });
});
