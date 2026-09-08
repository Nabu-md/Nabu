import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { buildClarifyingForm, CLARIFYING_QUESTION_MODES } from './clarifying-questions.js'

describe('buildClarifyingForm', () => {
  it('builds a form with normalized options and a custom input', () => {
    const form = buildClarifyingForm({
      question: 'Which layout?',
      options: [
        { label: 'Grid', description: 'Dense grid view' },
        { label: 'List' },
        { id: 'custom-id', label: 'Timeline' },
      ],
    })

    assert.equal(form.question, 'Which layout?')
    assert.equal(form.options.length, 3)
    assert.equal(form.options[0].id, 'option-1')
    assert.equal(form.options[0].description, 'Dense grid view')
    assert.equal(form.options[2].id, 'custom-id')
    assert.equal(form.allow_custom, true)
    assert.equal(form.remember_key, null)
    assert.match(form.id, /^[\da-f-]{36}$/)
  })

  it('caps options at three', () => {
    const form = buildClarifyingForm({
      question: 'Pick one',
      options: [
        { label: 'A' },
        { label: 'B' },
        { label: 'C' },
        { label: 'D' },
      ],
    })
    assert.equal(form.options.length, 3)
  })

  it('keeps the remember key when provided', () => {
    const form = buildClarifyingForm({
      question: 'Units?',
      options: [{ label: 'Metric' }],
      rememberKey: 'preferred-units',
    })
    assert.equal(form.remember_key, 'preferred-units')
  })

  it('accepts gated modes', () => {
    for (const mode of CLARIFYING_QUESTION_MODES) {
      const form = buildClarifyingForm({ question: 'Q', options: [{ label: 'A' }], mode })
      assert.equal(form.mode, mode)
    }
  })

  it('rejects a missing question', () => {
    assert.throws(() => buildClarifyingForm({ options: [{ label: 'A' }] }), /question is required/)
  })

  it('rejects forms without any labeled option', () => {
    assert.throws(
      () => buildClarifyingForm({ question: 'Q', options: [{ label: '  ' }] }),
      /at least one labeled choice/,
    )
    assert.throws(() => buildClarifyingForm({ question: 'Q' }), /at least one labeled choice/)
  })

  it('rejects modes outside the gated allowlist', () => {
    assert.throws(
      () => buildClarifyingForm({ question: 'Q', options: [{ label: 'A' }], mode: 'safe' }),
      /not available in safe mode/,
    )
  })
})
