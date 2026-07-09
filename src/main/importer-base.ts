/**
 * importer-base.ts
 *
 * Base types and registry for format importers.
 *
 * Requirements: 32.1, 32.2, 32.3, 32.4, 32.5, 32.6, 32.7, 32.8, 32.9
 */

export interface ImportResult {
  success: boolean
  path?: string
  error?: string
}

export interface ImportProgress {
  file: string
  status: 'pending' | 'processing' | 'complete' | 'error'
  message?: string
}

export type ImporterFormat = 'notion' | 'roam' | 'evernote' | 'pdf' | 'docx' | 'csv'

export interface Importer {
  format: ImporterFormat
  /** Parse source data and return markdown content with frontmatter */
  parse(data: unknown, sourcePath: string): Promise<string>
  /** Validate the source file format */
  validate(data: unknown): boolean
}

const importers = new Map<ImporterFormat, Importer>()

export function registerImporter(importer: Importer): void {
  importers.set(importer.format, importer)
}

export function getImporter(format: ImporterFormat): Importer | undefined {
  return importers.get(format)
}

export function getSupportedFormats(): ImporterFormat[] {
  return Array.from(importers.keys())
}