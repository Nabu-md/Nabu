import { describe, it, expect } from 'vitest'

describe('Bookmarks logic', () => {
  it('adds a file to a new list', () => {
    const collection: Record<string, string[]> = {}
    const listName = 'Reading List'
    const filePath = '/vault/note.md'

    if (!collection[listName] || !collection[listName].includes(filePath)) {
      collection[listName] = [filePath]
    }
    expect(collection).toEqual({ 'Reading List': ['/vault/note.md'] })
  })

  it('removes a file and deletes empty list', () => {
    const collection: Record<string, string[]> = { 'Reading List': ['/vault/note.md'] }
    const listName = 'Reading List'
    const filePath = '/vault/note.md'

    if (collection[listName]) {
      const index = collection[listName].indexOf(filePath)
      if (index >= 0) {
        collection[listName].splice(index, 1)
      }
      if (collection[listName].length === 0) {
        delete collection[listName]
      }
    }
    expect(collection).toEqual({})
  })

  it('removes a file from all bookmark lists', () => {
    const collection: Record<string, string[]> = {
      'Reading List': ['/vault/note1.md', '/vault/note2.md'],
      'Ideas': ['/vault/note2.md']
    }
    const filePath = '/vault/note2.md'

    for (const listName of Object.keys(collection)) {
      const list = collection[listName]
      const index = list.indexOf(filePath)
      if (index >= 0) {
        list.splice(index, 1)
      }
      if (list.length === 0) {
        delete collection[listName]
      }
    }
    expect(collection).toEqual({ 'Reading List': ['/vault/note1.md'] })
  })
})