/**
 * validation/index.ts
 *
 * Reusable validation helpers for the shared contracts layer.
 *
 * Responsibilities (Phase 1.4):
 *  - payload validation
 *  - schema validation
 *  - structured error generation
 *  - reusable validation utilities
 *
 * Rules:
 *  - Deterministic and side-effect free.
 *  - Independent of Electron and React.
 *  - No application behavior — only validation primitives.
 */

import { z } from 'zod'

// ---------------------------------------------------------------------------
// Structured error contract
// ---------------------------------------------------------------------------

/**
 * A structured validation error. Prefer this over arbitrary strings so that
 * consumers (main + renderer) can programmatically inspect failures.
 */
export interface ValidationError {
  /** Machine-readable error code. */
  code: string
  /** Human-readable message. */
  message: string
  /** Path to the offending field, e.g. ["payload", "path"]. */
  path: PropertyKey[]
}

/** Result wrapper for a validation attempt. */
export type ValidationResult<T> =
  { success: true; data: T } | { success: false; errors: ValidationError[] }

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Convert a ZodError into a list of structured ValidationError objects.
 * Deterministic: same input always yields the same output.
 */
export function zodErrorToValidationErrors(error: z.ZodError): ValidationError[] {
  return error.issues.map((issue) => ({
    code: issue.code,
    message: issue.message,
    path: issue.path
  }))
}

/**
 * Format a ZodError into a short, readable string suitable for logging or an
 * activity:log message. Pure function — no I/O.
 */
export function formatZodError(error: z.ZodError): string {
  return error.issues.map((e) => `${e.path.join('.')}: ${e.message}`).join('; ')
}

/**
 * Validate an unknown value against a Zod schema, returning a structured
 * ValidationResult instead of throwing. Side-effect free.
 */
export function validatePayload<T>(schema: z.ZodType<T>, value: unknown): ValidationResult<T> {
  const result = schema.safeParse(value)
  if (result.success) {
    return { success: true, data: result.data }
  }
  return { success: false, errors: zodErrorToValidationErrors(result.error) }
}

/**
 * Build a single structured ValidationError. Pure helper used by callers that
 * need to construct errors without a schema (e.g. semantic checks).
 */
export function makeValidationError(
  code: string,
  message: string,
  path: PropertyKey[] = []
): ValidationError {
  return { code, message, path }
}

/**
 * Type guard: is the given value a successful ValidationResult?
 */
export function isValidationSuccess<T>(
  result: ValidationResult<T>
): result is { success: true; data: T } {
  return result.success
}
