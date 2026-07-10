/**
 * vector.ts
 *
 * VectorManager — manages the Vectra vector index and BGE-micro ONNX embeddings
 * for semantic similarity search across vault files.
 *
 * Requirements: 9.1, 9.2, 9.3, 9.4, 9.5, 9.8, 9.9
 */

import path from 'path'
import fs from 'fs/promises'
import { pipeline, env } from '@xenova/transformers'
import { LocalIndex } from 'vectra'
import type { SearchResult } from '../shared/types'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface VectorConfig {
  /** Absolute path to the `.nabu/` directory */
  indexPath: string
  /** Absolute path to the bundled ONNX model directory */
  modelPath: string
}

interface VaultFileRef {
  path: string
  text: string
}

type EmbedTask = VaultFileRef

// Metadata stored alongside each vector in the index.
// The index signature satisfies vectra's `Record<string, MetadataTypes>` constraint.
interface FileMetadata extends Record<string, import('vectra').MetadataTypes> {
  path: string
  name: string
  mtime: number
  charCount: number
}

// ---------------------------------------------------------------------------
// Async serial queue
// ---------------------------------------------------------------------------

/**
 * A lightweight serial async queue that processes tasks one at a time,
 * preventing concurrent embedding operations from exhausting memory.
 *
 * Requirements: 9.2, 9.3
 */
class AsyncQueue<T> {
  private queue: T[] = []
  private running = false

  constructor(private readonly worker: (item: T) => Promise<void>) {}

  enqueue(item: T): void {
    this.queue.push(item)
    if (!this.running) {
      void this.drain()
    }
  }

  private async drain(): Promise<void> {
    this.running = true
    while (this.queue.length > 0) {
      const item = this.queue.shift()!
      try {
        await this.worker(item)
      } catch (err) {
        console.error('[VectorManager] Queue worker error:', err)
      }
    }
    this.running = false
  }
}

// ---------------------------------------------------------------------------
// VectorManager
// ---------------------------------------------------------------------------

export class VectorManager {
  private index: LocalIndex<FileMetadata> | null = null
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private embedder: ((text: string, options: object) => Promise<any>) | null = null
  private embeddingsDisabled = false
  private disabledReason: string | null = null
  private queue!: AsyncQueue<EmbedTask>

  /** Callback used to send activity:log messages to the renderer. */
  private logFn: (level: 'info' | 'warn' | 'error', message: string) => void = () => {}

  // -------------------------------------------------------------------------
  // Public API
  // -------------------------------------------------------------------------

  /**
   * Register a logging callback so VectorManager can emit `activity:log` events.
   */
  setLogCallback(fn: (level: 'info' | 'warn' | 'error', message: string) => void): void {
    this.logFn = fn
  }

  /**
   * Initialise the vector index and BGE-micro embedding model.
   *
   * 1. Load (or create) the Vectra index from `.nabu/`
   * 2. Load the BGE-micro ONNX model from the app bundle
   * 3. If model files are missing, log error and disable embeddings (Req 9.8)
   * 4. If the index is corrupted, rebuild from vault as a background task (Req 9.9)
   *
   * Requirements: 9.1, 9.8, 9.9
   */
  async initialize(config: VectorConfig): Promise<void> {
    // Set up the serial embed queue
    this.queue = new AsyncQueue<EmbedTask>((task) => this.processEmbedTask(task))

    // ---- Step 1: Initialise Vectra index ----
    const indexDir = path.join(config.indexPath, 'vectra')
    this.index = new LocalIndex<FileMetadata>(indexDir)

    const indexExists = await this.index.isIndexCreated()
    if (!indexExists) {
      try {
        await this.index.createIndex({ version: 1 })
        this.log('info', 'Vector index created at ' + indexDir)
      } catch (err) {
        this.log('error', `Failed to create vector index: ${String(err)}`)
        this.index = null
      }
    } else {
      // Verify the index is not corrupted by reading its stats
      try {
        await this.index.getIndexStats()
      } catch (err) {
        this.log('warn', `Vector index corrupted — rebuilding in background. ${String(err)}`)
        // Rebuild non-blocking (caller can pass vault files via embedFile later)
        void this.rebuildIndex()
      }
    }

    // ---- Step 2: Load BGE-micro model ----
    await this.loadModel(config.modelPath)
  }

  /**
   * Queue a file for embedding. The queue processes one file at a time to
   * limit memory pressure. Returns immediately (non-blocking).
   *
   * Skips files whose text is empty (e.g. frontmatter-only notes) to avoid
   * storing degenerate zero-content vectors (Requirement 1.8).
   *
   * Requirements: 1.8, 9.2, 9.3
   */
  embedFile(filePath: string, text: string): void {
    if (this.embeddingsDisabled || !this.index) return
    if (!text || text.trim().length === 0) return // Requirement 1.8
    this.queue.enqueue({ path: filePath, text })
  }

  /**
   * Remove a file's vector from the Vectra index.
   *
   * Called by the watcher when a file is deleted from the vault. Does not
   * throw — failures are logged and silently ignored so deletion is never
   * blocked by a vector-index error.
   *
   * Requirements: 1.3
   */
  async removeFile(filePath: string): Promise<void> {
    if (!this.index || this.embeddingsDisabled) return
    try {
      await this.index.deleteItem(filePath)
    } catch (err) {
      this.log('error', `Failed to remove vector for "${filePath}": ${String(err)}`)
    }
  }

  /**
   * Return the current status of the vector index.
   *
   * When embeddings are disabled (model did not load), returns `disabled: true`
   * with a human-readable reason. Also reports the number of items in the index
   * so callers can detect an empty index (Requirement 1.7).
   *
   * Requirements: 1.5, 1.6, 1.7
   */
  async getStatus(): Promise<{ disabled: boolean; reason: string | null; items: number }> {
    let items = 0
    if (this.index && !this.embeddingsDisabled) {
      try {
        const stats = await this.index.getIndexStats()
        items = stats.items
      } catch {
        // Index stats unavailable — leave items as 0
      }
    }
    return {
      disabled: this.embeddingsDisabled || !this.index,
      reason: this.disabledReason,
      items
    }
  }

  /**
   * Enqueue all vault files for re-embedding.
   *
   * Reads each file from disk and enqueues it through the AsyncQueue.
   * Returns the number of non-empty files processed.
   *
   * Requirements: 1.5, 1.6
   */
  async reindexAll(files: import('../shared/types').FileEntry[]): Promise<number> {
    if (this.embeddingsDisabled || !this.index) return 0

    let processed = 0
    for (const file of files) {
      try {
        const content = await fs.readFile(file.path, 'utf-8')
        if (content && content.trim().length > 0) {
          this.queue.enqueue({ path: file.path, text: content })
          processed++
        }
      } catch (err) {
        this.log('error', `Failed to read file for reindex "${file.path}": ${String(err)}`)
      }
    }
    this.log('info', `Reindex complete: ${processed}/${files.length} files enqueued`)
    return processed
  }

  /**
   * Search the vector index for notes semantically similar to `queryText`.
   *
   * Returns up to `limit` results, excluding `excludePath` if provided.
   * Each result includes the file path, cosine-similarity score (0.0–1.0),
   * and approximate token count (charCount / 4).
   *
   * Requirements: 9.4, 9.5
   */
  async search(queryText: string, limit: number, excludePath?: string): Promise<SearchResult[]> {
    if (this.embeddingsDisabled || !this.index) return []

    let queryEmbedding: number[]
    try {
      queryEmbedding = await this.generateEmbedding(queryText)
    } catch (err) {
      this.log('error', `Failed to generate query embedding: ${String(err)}`)
      return []
    }

    let results
    try {
      // Query with limit + 1 so we can drop the excluded path and still
      // return up to `limit` results.
      results = await this.index.queryItems(queryEmbedding, queryText, limit + 1)
    } catch (err) {
      this.log('error', `Vector index query failed: ${String(err)}`)
      return []
    }

    return results
      .filter((r) => r.item.metadata.path !== excludePath)
      .slice(0, limit)
      .map((r) => ({
        path: r.item.metadata.path,
        score: Math.round(r.score * 100) / 100, // 2 decimal places (Req 9.5)
        tokenCount: Math.round((r.item.metadata.charCount ?? 0) / 4) // Req 9.5
      }))
  }

  /**
   * Generate a 384-dim BGE-micro embedding for the given text.
   *
   * Requirements: 9.2
   */
  async generateEmbedding(text: string): Promise<number[]> {
    if (!this.embedder) {
      throw new Error('Embedder not available')
    }

    const output = await this.embedder(text, {
      pooling: 'mean',
      normalize: true
    })

    // @xenova/transformers returns a Tensor; extract the flat Float32Array data
    const data: number[] = Array.from(output.data as Float32Array)
    return data
  }

  // -------------------------------------------------------------------------
  // Private helpers
  // -------------------------------------------------------------------------

  /**
   * Load the BGE-micro ONNX pipeline from the bundled model directory.
   * If the files are not present, logs an error and disables embeddings.
   *
   * Requirements: 9.8
   */
  private async loadModel(modelPath: string): Promise<void> {
    try {
      // Point @xenova/transformers to the local bundled model directory so it
      // never attempts a network download at runtime.
      env.localModelPath = modelPath
      // Disable remote model fetching — app bundle must contain all files.
      env.allowRemoteModels = false

      // The pipeline call loads the ONNX model; 'Xenova/bge-micro-v2' must be
      // present under `modelPath` as a subdirectory named `bge-micro-v2`.
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      this.embedder = (await pipeline('feature-extraction', 'Xenova/bge-micro-v2')) as any
      this.log('info', 'BGE-micro-v2 embedding model loaded')
    } catch (err) {
      // Model files missing or corrupt — disable embeddings (Req 9.8)
      this.embeddingsDisabled = true
      this.disabledReason = `BGE-micro ONNX model failed to load: ${String(err)}`
      this.embedder = null
      this.log(
        'error',
        `BGE-micro ONNX model not found or failed to load — embeddings disabled. ${String(err)}`
      )
    }
  }

  /**
   * Worker called by the serial queue to embed a single file and upsert it
   * into the Vectra index.
   *
   * Requirements: 9.2, 9.3
   */
  private async processEmbedTask(task: EmbedTask): Promise<void> {
    if (!this.index || this.embeddingsDisabled) return

    try {
      const vector = await this.generateEmbedding(task.text)
      const metadata: FileMetadata = {
        path: task.path,
        name: path.basename(task.path, '.md'),
        mtime: Date.now(),
        charCount: task.text.length
      }

      await this.index.upsertItem({
        id: task.path, // use absolute path as stable ID
        vector,
        metadata
      })
    } catch (err) {
      this.log('error', `Failed to embed file "${task.path}": ${String(err)}`)
    }
  }

  /**
   * Rebuild the Vectra index from scratch.
   * Called as a non-blocking background task when corruption is detected.
   *
   * Requirements: 9.9
   */
  private async rebuildIndex(): Promise<void> {
    if (!this.index) return

    try {
      // Delete and recreate the index
      await this.index.deleteIndex()
      await this.index.createIndex({ version: 1 })
      this.log('warn', 'Vector index rebuilt. Re-embedding will begin when files are opened.')
    } catch (err) {
      this.log('error', `Failed to rebuild vector index: ${String(err)}`)
      this.index = null
    }
  }

  /** Emit a log message via the registered callback. */
  private log(level: 'info' | 'warn' | 'error', message: string): void {
    console[level](`[VectorManager] ${message}`)
    this.logFn(level, message)
  }
}

// ---------------------------------------------------------------------------
// Singleton export
// ---------------------------------------------------------------------------

/** Singleton VectorManager instance used by the main process. */
export const vectorManager = new VectorManager()
