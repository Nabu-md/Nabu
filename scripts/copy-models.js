#!/usr/bin/env node
/**
 * copy-models.js
 *
 * Copies the BGE-micro-v2 ONNX model files from node_modules/@xenova/transformers
 * into resources/models/bge-micro-v2/ so electron-builder can bundle them as
 * extraResources in the app package.
 *
 * Usage: node scripts/copy-models.js
 *
 * Requirements: 12.4
 */

import { cpSync, existsSync, mkdirSync, readdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');

// Source: @xenova/transformers stores models under assets/ or cache/
// The model can be at several locations depending on how it was fetched.
const POSSIBLE_SOURCES = [
  join(ROOT, 'node_modules', '@xenova', 'transformers', 'assets', 'Xenova', 'bge-micro-v2'),
  join(ROOT, 'node_modules', '@xenova', 'transformers', 'assets', 'bge-micro-v2'),
  // Hugging Face cache populated by transformers on first run
  join(ROOT, 'node_modules', '@xenova', 'transformers', 'dist', 'models', 'bge-micro-v2'),
];

const DEST = join(ROOT, 'resources', 'models', 'bge-micro-v2');

function findSource() {
  for (const candidate of POSSIBLE_SOURCES) {
    if (existsSync(candidate)) {
      return candidate;
    }
  }
  return null;
}

const src = findSource();

if (!src) {
  console.warn(
    '[copy-models] WARNING: BGE-micro-v2 model files not found in node_modules.\n' +
      '  The model will be downloaded at runtime on first launch.\n' +
      '  To bundle the model, run the app once in dev mode to cache it, then re-run this script.\n' +
      '  Searched:\n' +
      POSSIBLE_SOURCES.map((p) => '    ' + p).join('\n'),
  );
  // Exit 0 — missing model is a soft warning; the app handles it gracefully.
  process.exit(0);
}

// Ensure destination directory exists
mkdirSync(DEST, { recursive: true });

console.log(`[copy-models] Copying BGE-micro-v2 from:\n  ${src}\n  → ${DEST}`);

try {
  // Count files for logging
  const files = readdirSync(src, { recursive: true });
  cpSync(src, DEST, { recursive: true });
  console.log(`[copy-models] Copied ${files.length} file(s) successfully.`);
} catch (err) {
  console.error('[copy-models] ERROR: Failed to copy model files:', err.message);
  process.exit(1);
}
