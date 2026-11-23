import { afterEach, beforeEach, expect, test } from 'bun:test'
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'

const hook = readFileSync(new URL('../.githooks/pre-commit', import.meta.url), 'utf8')
const catalog = readFileSync(new URL('./i18n/catalog.ts', import.meta.url), 'utf8')
const attributes = readFileSync(new URL('../.gitattributes', import.meta.url), 'utf8')
let sandbox: string
let repo: string
let bin: string

function run(args: string[], path = `${bin}:/usr/bin:/bin`) {
  return Bun.spawnSync(args, {
    cwd: repo,
    env: { ...process.env, PATH: path },
  })
}

function git(...args: string[]) {
  const result = run(['git', ...args])
  if (result.exitCode !== 0) throw new Error(result.stderr.toString())
  return result.stdout.toString()
}

function write(path: string, contents: string) {
  const fullPath = join(repo, path)
  mkdirSync(dirname(fullPath), { recursive: true })
  writeFileSync(fullPath, contents)
}

function executable(path: string, contents: string) {
  mkdirSync(dirname(path), { recursive: true })
  writeFileSync(path, contents)
  chmodSync(path, 0o755)
}

function formatter(tool: string) {
  executable(
    join(bin, tool),
    `#!/bin/sh
for file do
    case "$file" in
        *.go|*.rs|*.ts|*.kt|*.swift) printf '// formatted\\n' >> "$file" ;;
    esac
done
`,
  )
}

function checkHook(path?: string) {
  return run(['bash', '.githooks/pre-commit'], path)
}

function pathWithoutBun() {
  const isolatedBin = join(sandbox, 'no-bun')
  mkdirSync(isolatedBin)
  for (const tool of ['bash', 'git']) {
    const source = Bun.which(tool)
    if (!source) throw new Error(`Missing test prerequisite: ${tool}`)
    symlinkSync(source, join(isolatedBin, tool))
  }
  return isolatedBin
}

beforeEach(() => {
  sandbox = mkdtempSync(join(tmpdir(), 'meron-hook-test-'))
  repo = join(sandbox, 'repo')
  bin = join(sandbox, 'bin')
  mkdirSync(repo)
  mkdirSync(bin)
  git('init', '-q')
  git('config', 'user.name', 'Hook Test')
  git('config', 'user.email', 'hook@example.invalid')
  git('config', 'core.hooksPath', '.githooks')
  write('.githooks/pre-commit', hook)
  // Keep fixture setup commits from invoking the hook.
  git('add', '.')
  git('commit', '-qm', 'fixture')
  for (const tool of ['gofmt', 'rustfmt', 'ktlint', 'swiftformat']) formatter(tool)
})

afterEach(() => rmSync(sandbox, { recursive: true, force: true }))

test('formats and stages paths containing spaces, quotes, and newlines', () => {
  const path = "desktop/a 'quoted'\nfile.go"
  write(path, 'package main\n')
  git('add', '--', path)
  expect(checkHook().exitCode).toBe(0)
  expect(git('show', `:${path}`)).toBe('package main\n// formatted\n')
  expect(git('diff', '--', path)).toBe('')
})

test('preserves both versions of a partially staged file', () => {
  const path = 'desktop/main.go'
  write(path, 'package main\n')
  git('add', path)
  write(path, 'package main\n// unstaged\n')
  const result = checkHook()
  expect(result.exitCode).toBe(0)
  expect(result.stderr.toString()).toContain('unstaged changes')
  expect(git('show', `:${path}`)).toBe('package main\n')
  expect(readFileSync(join(repo, path), 'utf8')).toBe('package main\n// unstaged\n')
})

test('formats renamed files', () => {
  write('desktop/old.go', 'package main\n')
  git('add', '.')
  git('commit', '-qm', 'original')
  git('mv', 'desktop/old.go', 'desktop/new.go')
  expect(checkHook().exitCode).toBe(0)
  expect(git('show', ':desktop/new.go')).toContain('// formatted')
})

test('does not format symlink targets', () => {
  write('target', 'unchanged\n')
  expect(run(['ln', '-s', 'target', 'desktop-link.go']).exitCode).toBe(0)
  git('add', '.')
  expect(checkHook().exitCode).toBe(0)
  expect(readFileSync(join(repo, 'target'), 'utf8')).toBe('unchanged\n')
})

test('reports missing formatters', () => {
  write('desktop/frontend/src/test.ts', 'const x = 1\n')
  git('add', '.')
  const result = checkHook(pathWithoutBun())
  expect(result.exitCode).toBe(0)
  expect(result.stderr.toString()).toContain('skipping Frontend formatting')
})

test.each([
  ['gofmt', 'desktop/Test.go'],
  ['rustfmt', 'meron-core/src/test.rs'],
  ['bun', 'desktop/frontend/src/test.ts'],
  ['ktlint', 'mobile/Test.kt'],
  ['swiftformat', 'mobile/ios/Test.swift'],
])('stages partial formatter fixes and keeps blocking retries: %s', (tool, path) => {
  executable(
    join(bin, tool),
    `#!/bin/sh
for file do
    case "$file" in
        *.go|*.rs|*.ts|*.kt|*.swift) printf '// formatted\\n' >> "$file" ;;
    esac
done
exit 7
`,
  )
  write(path, '// original\n')
  git('add', '.')
  expect(checkHook().exitCode).toBe(7)
  expect(git('show', `:${path}`)).toBe('// original\n// formatted\n')
  expect(git('diff', '--', path)).toBe('')
  const retry = checkHook()
  expect(retry.exitCode).toBe(7)
  expect(retry.stderr.toString()).not.toContain('skipping')
  expect(git('show', `:${path}`)).toBe('// original\n// formatted\n// formatted\n')
  expect(git('diff', '--', path)).toBe('')
})

test('Rust formatting explicitly disables child traversal', () => {
  executable(join(bin, 'rustfmt'), '#!/bin/sh\n[ "$3" = "--config" ] && [ "$4" = "skip_children=true" ]\n')
  write('meron-core/src/lib.rs', 'mod child;\n')
  git('add', '.')
  expect(checkHook().exitCode).toBe(0)
})

test.each(['bad whitespace  \n', '<<<<<<< HEAD\n'])('rejects staged diff errors: %s', (contents) => {
  write('notes.txt', contents)
  git('add', '.')
  expect(checkHook().exitCode).not.toBe(0)
})

test('allows CRLF batch files while still rejecting trailing spaces', () => {
  write('.gitattributes', attributes)
  write('mobile/gradlew.bat', '@echo off\r\n')
  git('add', '.')
  expect(checkHook().exitCode).toBe(0)
  write('mobile/gradlew.bat', '@echo off  \r\n')
  git('add', '.')
  expect(checkHook().exitCode).not.toBe(0)
})

function localizationFixture() {
  write('scripts/i18n/catalog.ts', catalog)
  const localesDir = new URL('../locales/', import.meta.url)
  for (const path of new Bun.Glob('*.json').scanSync({ cwd: localesDir.pathname })) {
    write(`locales/${path}`, readFileSync(new URL(path, localesDir), 'utf8'))
  }
  executable(join(bin, 'bun'), `#!/bin/sh\nexec '${process.execPath.replaceAll("'", "'\\''")}' "$@"\n`)
  git('add', '.')
}

test('validates staged catalogs even when working catalogs are invalid', () => {
  localizationFixture()
  write('locales/en.json', '{}\n')
  expect(checkHook().exitCode).toBe(0)
  expect(readFileSync(join(repo, 'locales/en.json'), 'utf8')).toBe('{}\n')
})

test('rejects invalid staged catalogs even when fixed in the working tree', () => {
  localizationFixture()
  const original = readFileSync(join(repo, 'locales/en.json'), 'utf8')
  write('locales/en.json', '{}\n')
  git('add', 'locales/en.json')
  write('locales/en.json', original)
  expect(checkHook().exitCode).not.toBe(0)
  expect(git('show', ':locales/en.json')).toBe('{}\n')
})

test('rejects deleted catalogs', () => {
  localizationFixture()
  git('rm', '-f', 'locales/en.json')
  const result = checkHook()
  expect(result.exitCode).not.toBe(0)
  expect(result.stderr.toString()).toContain('Missing catalog: locales/en.json')
})

test('requires Bun only for localization changes', () => {
  const isolatedBin = pathWithoutBun()
  expect(checkHook(isolatedBin).exitCode).toBe(0)
  write('locales/en.json', '{}\n')
  git('add', '.')
  const result = checkHook(isolatedBin)
  expect(result.exitCode).toBe(1)
  expect(result.stderr.toString()).toContain('Bun is required')
})
