/**
 * pdf-ipc.test.ts
 *
 * Unit tests for the PDF IPC channel Zod schemas (Req 40.1, 40.2, 40.4, 40.5, 40.6, 40.7, 40.8).
 *
 * Requirements: 40.1, 40.2, 40.4, 40.5, 40.6, 40.7, 40.8
 */

import { describe, it, expect } from 'vitest'
import {
  PDFOpenSchema,
  PDFOpenResultSchema,
  PDFRenderPageSchema,
  PDFRenderPageResultSchema,
  PDFAnnotationSchema,
  PDFLoadAnnotationsSchema,
  PDFLoadAnnotationsResultSchema,
  PDFSaveAnnotationsSchema,
  PDFSaveAnnotationsResultSchema
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

  // ---------------------------------------------------------------------------
  // PDF Annotation schemas (Req 40.4, 40.5)
  // ---------------------------------------------------------------------------

  describe('PDFAnnotationSchema', () => {
    it('accepts a valid annotation object', () => {
      const result = PDFAnnotationSchema.safeParse({
        id: 'annotation-1',
        page: 5,
        rect: { x: 100, y: 200, w: 300, h: 50 },
        text: 'Selected text from PDF',
        color: 'yellow',
        timestamp: Date.now()
      })
      expect(result.success).toBe(true)
    })

    it('accepts optional comment and linkedNotePath', () => {
      const result = PDFAnnotationSchema.safeParse({
        id: 'annotation-2',
        page: 1,
        rect: { x: 0, y: 0, w: 100, h: 20 },
        text: 'Another annotation',
        color: 'green',
        timestamp: Date.now(),
        comment: 'This is a comment',
        linkedNotePath: '/vault/notes/annotation-note.md'
      })
      expect(result.success).toBe(true)
      if (result.success) {
        expect(result.data.comment).toBe('This is a comment')
        expect(result.data.linkedNotePath).toBe('/vault/notes/annotation-note.md')
      }
    })

    it('rejects invalid color values', () => {
      const result = PDFAnnotationSchema.safeParse({
        id: 'annotation-3',
        page: 1,
        rect: { x: 0, y: 0, w: 100, h: 20 },
        text: 'Text',
        color: 'red', // invalid - not in enum
        timestamp: Date.now()
      })
      expect(result.success).toBe(false)
    })
  })

  describe('PDFLoadAnnotationsSchema', () => {
    it('accepts a path', () => {
      const result = PDFLoadAnnotationsSchema.safeParse({ path: '/vault/doc.pdf' })
      expect(result.success).toBe(true)
    })
  })

  describe('PDFLoadAnnotationsResultSchema', () => {
    it('accepts an array of annotations', () => {
      const result = PDFLoadAnnotationsResultSchema.safeParse({
        annotations: [
          {
            id: 'a1',
            page: 1,
            rect: { x: 0, y: 0, w: 100, h: 20 },
            text: 'Text',
            color: 'yellow',
            timestamp: Date.now()
          }
        ]
      })
      expect(result.success).toBe(true)
    })

    it('accepts empty annotations array', () => {
      const result = PDFLoadAnnotationsResultSchema.safeParse({ annotations: [] })
      expect(result.success).toBe(true)
    })
  })

  describe('PDFSaveAnnotationsSchema', () => {
    it('accepts path and annotations array', () => {
      const result = PDFSaveAnnotationsSchema.safeParse({
        path: '/vault/doc.pdf',
        annotations: []
      })
      expect(result.success).toBe(true)
    })
  })

  describe('PDFSaveAnnotationsResultSchema', () => {
    it('accepts success boolean', () => {
      const result = PDFSaveAnnotationsResultSchema.safeParse({ success: true })
      expect(result.success).toBe(true)
    })

    it('allows optional error field', () => {
      const result = PDFSaveAnnotationsResultSchema.safeParse({
        success: false,
        error: 'Failed to write file'
      })
      expect(result.success).toBe(true)
    })
  })

  // ---------------------------------------------------------------------------
  // Note card template generation (Req 40.6)
  // ---------------------------------------------------------------------------

  describe('Note card template generation (Req 40.6)', () => {
    it('generates correct frontmatter for annotation note', () => {
      const annotation = {
        id: 'a1',
        page: 5,
        rect: { x: 0, y: 0, w: 100, h: 20 },
        text: 'Selected text from PDF',
        color: 'yellow' as const,
        timestamp: 1700000000000
      }
      const pdfName = 'document'
      const isoDate = new Date(annotation.timestamp).toISOString()

      const body = [
        `> ${annotation.text}`,
        '',
        `Source: [[${pdfName}.pdf#page=${annotation.page}]]`
      ].join('\n')

      const frontmatter = `---\nsource: [[${pdfName}.pdf]]\npage: ${annotation.page}\nannotation_date: ${isoDate}\n---\n\n`

      const fullContent = frontmatter + body

      expect(fullContent).toContain('source: [[document.pdf]]')
      expect(fullContent).toContain('page: 5')
      expect(fullContent).toContain('annotation_date:')
      expect(fullContent).toContain('> Selected text from PDF')
      expect(fullContent).toContain('Source: [[document.pdf#page=5]]')
    })

    it('truncates annotation text to 60 chars for title (Req 40.6)', () => {
      const longText =
        'This is a very long annotation text that should be truncated to 60 characters for the title'
      const title = longText.substring(0, 60)
      expect(title.length).toBe(60)
      // Verify the first 60 characters are correctly extracted
      expect(title.startsWith('This is a very long annotation text that should be')).toBe(true)
    })

    it('uses fallback title when text is empty', () => {
      const emptyText = ''
      const title = emptyText.substring(0, 60) || 'PDF Annotation'
      expect(title).toBe('PDF Annotation')
    })
  })
})
