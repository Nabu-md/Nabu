/**
 * Vault template helpers — `vault/templates/*.md` note templates.
 *
 * Templates are plain Markdown files (frontmatter + body skeleton) stored in
 * the vault's `templates/` folder. They can contain `{{placeholder}}` tags and
 * optional defaults: `{{placeholder|Default value}}`.
 */
import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'

const TEMPLATES_DIR = 'templates'
const TEMPLATE_PLACEHOLDER_PATTERN = /\{\{\s*([\w.-]+)\s*(?:\|\s*([^}]*?)\s*)?\}\}/g

function templatesDir(vaultPath) {
  return path.join(vaultPath, TEMPLATES_DIR)
}

/**
 * List the note templates available in a vault.
 * @param {string} vaultPath
 * @returns {Promise<Array<{name: string, path: string, title: string}>>}
 */
export async function listTemplates(vaultPath) {
  const dir = templatesDir(vaultPath)
  let entries
  try {
    entries = await readdir(dir)
  } catch (error) {
    if (error?.code === 'ENOENT') return []
    throw error
  }

  const templates = []
  for (const entry of entries) {
    if (!entry.isFile() || !entry.name.endsWith('.md')) continue
    const relativePath = path.join(TEMPLATES_DIR, entry.name)
    const name = entry.name.slice(0, -'.md'.length)
    templates.push({
      name,
      path: relativePath,
      title: (await readTemplateTitle(path.join(dir, entry.name))) ?? name,
    })
  }

  templates.sort((left, right) => left.name.localeCompare(right.name))
  return templates
}

/**
 * Apply a named template with `{{placeholder|default}}` substitution and
 * create the resulting note. Fails when the template does not exist or the
 * target note already exists (same contract as create_note).
 *
 * @param {string} vaultPath
 * @param {{name: string, path: string, params?: Record<string, string>, title?: string, type?: string}} args
 * @returns {Promise<{path: string, absolutePath: string}>}
 */
export async function applyTemplate(vaultPath, args = {}) {
  const name = typeof args.name === 'string' ? args.name.trim() : ''
  if (!name) throw new Error('Template name is required')
  if (!/^[\w.-]+$/.test(name) || name.includes('..')) {
    throw new Error('Invalid template name')
  }

  const templatePath = path.join(templatesDir(vaultPath), `${name}.md`)
  let raw
  try {
    raw = await readFile(templatePath, 'utf8')
  } catch (error) {
    if (error?.code === 'ENOENT') {
      throw new Error(`Template not found: ${name}. Use list_templates to see available templates.`)
    }
    throw error
  }

  const content = renderTemplate(raw, templateParams(args))
  const { createNote } = await import('./vault.js')
  const notePath = typeof args.path === 'string' && args.path.trim()
    ? args.path.trim()
    : defaultTemplateNotePath(vaultPath, name)
  return createNote(vaultPath, notePath, content)
}

function templateParams(args) {
  const params = {}
  if (args.params && typeof args.params === 'object') {
    for (const [key, value] of Object.entries(args.params)) {
      if (typeof value === 'string') params[key] = value
    }
  }
  if (typeof args.title === 'string' && args.title.length > 0) params.title = args.title
  if (typeof args.type === 'string' && args.type.length > 0) params.type = args.type
  return params
}

function renderTemplate(raw, params) {
  return raw.replace(TEMPLATE_PLACEHOLDER_PATTERN, (match, key, fallback) => {
    if (key in params) return params[key]
    return fallback ?? match
  })
}

function defaultTemplateNotePath(vaultPath, templateName) {
  const extension = templateName.endsWith('.md') ? '' : '.md'
  return path.join(TEMPLATES_DIR, `${templateName}.new${extension}`)
}

async function readTemplateTitle(filePath) {
  try {
    const raw = await readFile(filePath, 'utf8')
    const h1 = raw.match(/^#\s+(.+)$/m)
    if (h1) return h1[1].trim()
    const fmTitle = raw.match(/^title:\s*(.+)$/m)
    if (fmTitle) return fmTitle[1].trim()
  } catch {
    // Fall through to the filename-based title.
  }
  return null
}