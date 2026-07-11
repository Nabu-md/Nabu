/**
 * pdf-ipc.test.ts
 *
 * Unit tests for the PDF IPC channel Zod schemas (Req 40.1, 40.2).
 *
 * Requirements: 40.1, 40.2
 */

import { describe, it, expect } from 'vitest'
import {
  PDFOpenSchema,
  PDFOpenResultSchema,
  PDFRenderPageSchema,
  PDFRenderPageResultSchema
} from '../../src/shared/schemas'

describe('PDF IPC schemas', () => {
  describe('PDFOpenSchema', () => {
    it('accepts a path', () => {
      const result = PDFOpenSchema.safeParse({ path: '/vault/doc.pdf' })
      expect(result.success).toBe(true)
      if (result.success) {
        expect(result.data.path).toBe('/vault/doc.pdf')
      }
    })

    it('rejects a missing path', () => {
      const result = PDFOpenSchema.safeParse({})
      expect(result.success).toBe(false)
    })
  })

  describe('PDFOpenResultSchema', () => {
    it('accepts totalPages + metadata', () => {
      const result = PDFOpenResultSchema.safeParse({
        totalPages: 12,
        metadata: { title: 'Doc', author: 'Jane' }
      })
      expect(result.success).toBe(true)
    })

    it('allows an error field for failure reporting', () => {
      const result = PDFOpenResultSchema.safeParse({
        totalPages: 0,
        metadata: {},
        error: 'corrupt pdf'
      })
      expect(result.success).toBe(true)
    })
  })

  describe('PDFRenderPageSchema', () => {
    it('accepts path, pageNumber, scale', () => {
      const result = PDFRenderPageSchema.safeParse({
        path: '/vault/doc.pdf',
        pageNumber: 3,
        scale: 1.5
      })
      expect(result.success).toBe(true)
    })

    it('rejects a non-positive page number', () => {
      const result = PDFRenderPageSchema.safeParse({
        path: '/vault/doc.pdf',
        pageNumber: 0,
        scale: 1
      })
      expect(result.success).toBe(false)
    })
  })

  describe('PDFRenderPageResultSchema', () => {
    it('accepts a base64 data URI result', () => {
      const result = PDFRenderPageResultSchema.safeParse({
        pageNumber: 3,
        dataUri: 'data:image/png;base64,AAAA',
        width: 600,
        height: 800
      })
      expect(result.success).toBe(true)
    })

    it('allows an error field for failure reporting', () => {
      const result = PDFRenderPageResultSchema.safeParse({
        pageNumber: 3,
        dataUri: '',
        width: 0,
        height: 0,
        error: 'page out of range'
      })
      expect(result.success).toBe(true)
    })
  })
})
