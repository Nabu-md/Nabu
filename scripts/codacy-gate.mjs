import { execFileSync, spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { addedLinesFromDiff, biomeGateFailures, eslintGateFailures } from './codacy-gate-lib.mjs'
import { sarifGateFailures } from './codacy-sarif.mjs'

const root = execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim()
const requestedBase = process.env.CODACY_BASE_REF || 'origin/main'
const verifiedBase = spawnSync('git', ['rev-parse', '--verify', `${requestedBase}^{commit}`], { cwd: root })
const base = verifiedBase.status === 0 ? requestedBase : 'HEAD^'
const diff = execFileSync('git', ['diff', '--unified=0', `${base}...HEAD`], {
  cwd: root,
  encoding: 'utf8',
  maxBuffer: 20 * 1024 * 1024,
})
const additions = addedLinesFromDiff(diff, root)

if (additions.size === 0) {
  console.log('Codacy gate: no added lines to analyze.')
  process.exit(0)
}

const cli = resolve(root, '.codacy/cli.sh')
if (!existsSync(cli)) {
  console.error('Codacy gate: .codacy/cli.sh is missing. Run the documented Codacy CLI setup.')
  process.exit(1)
}

const scannerEnvironment = {
  ...process.env,
  CODACY_CLI_V2_VERSION: '1.0.0-main.376.sha.799aab5',
  LANG: 'en_US.UTF-8',
  LC_ALL: 'en_US.UTF-8',
}
const install = spawnSync(cli, ['install'], { cwd: root, encoding: 'utf8', env: scannerEnvironment })
if (install.status !== 0) {
  console.error('Codacy gate: scanner installation failed.')
  process.exit(1)
}

const temporaryDirectory = mkdtempSync(join(tmpdir(), 'nabu-codacy-'))
const failures = []
try {
  const changedFiles = [...additions.keys()].map((path) => path.slice(root.length + 1)).filter((path) => existsSync(resolve(root, path)))
  const biome = spawnSync('pnpm', ['exec', 'biome', 'lint', ...changedFiles, '--reporter=json', '--max-diagnostics=none'], {
    cwd: root,
    encoding: 'utf8',
  })
  if (!biome.stdout) failures.push({ message: 'Biome did not produce an analysis report', tool: 'Biome' })
  else failures.push(...biomeGateFailures(JSON.parse(biome.stdout), additions, root))

  const eslintFiles = changedFiles.filter(path => /\.(?:[cm]?js|jsx|ts|tsx)$/u.test(path))
  if (eslintFiles.length > 0) {
    const eslint = spawnSync(process.execPath, [
      'node_modules/eslint8/bin/eslint.js',
      '--config',
      'eslint.codacy.config.js',
      '--format',
      'json',
      ...eslintFiles,
    ], { cwd: root, encoding: 'utf8' })
    if (!eslint.stdout || (eslint.status !== 0 && eslint.status !== 1)) {
      failures.push({ message: 'security ESLint scanner did not produce a valid report', tool: 'Codacy ESLint security rules' })
    } else {
      try {
        failures.push(...eslintGateFailures(JSON.parse(eslint.stdout), additions))
      } catch {
        failures.push({ message: 'security ESLint report was not valid JSON', tool: 'Codacy ESLint security rules' })
      }
    }
  }

  for (const tool of ['opengrep', 'trivy']) {
    const output = join(temporaryDirectory, `${tool}.sarif`)
    const result = spawnSync(cli, ['analyze', '.', '--tool', tool, '--format', 'sarif', '--output', output], {
      cwd: root,
      encoding: 'utf8',
      env: scannerEnvironment,
    })
    if (result.status !== 0 || !existsSync(output)) {
      failures.push({ message: `${tool} exited unsuccessfully`, tool })
      continue
    }
    failures.push(...sarifGateFailures(JSON.parse(readFileSync(output, 'utf8')), additions, root))
  }
} finally {
  rmSync(temporaryDirectory, { force: true, recursive: true })
}

if (failures.length > 0) {
  console.error(`Codacy gate failed with ${failures.length} new issue(s) or scanner failure(s):`)
  for (const failure of failures) {
    const location = failure.path ? `${failure.path}:${failure.line}` : failure.tool
    console.error(`- ${location} ${failure.rule ?? ''} ${failure.message}`.trim())
  }
  process.exit(1)
}

console.log('Codacy gate passed: no unreviewed new issues.')
