import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'

const root = fileURLToPath(new URL('../', import.meta.url))
const target = fileURLToPath(new URL('../../target/website/', import.meta.url))
const vite = fileURLToPath(new URL('../node_modules/vite/bin/vite.js', import.meta.url))

function build(output, docsURL) {
  return spawnSync(process.execPath, [vite, 'build', '--outDir', output], {
    cwd: root,
    env: { ...process.env, KEDRA_DOCS_URL: docsURL },
    encoding: 'utf8',
    timeout: 30_000,
  })
}

test('landing artifacts retain locale routes, configured manual links and licensed source assets',
  async () => {
    await mkdir(target, { recursive: true })
    const output = await mkdtemp(join(target, 'site-contract-'))
    try {
      const result = build(output, 'https://manual.example.test/guide')
      assert.equal(result.status, 0, result.stdout + result.stderr)
      for (const [locale, page] of [['en', 'index.html'], ['ru', 'ru/index.html']]) {
        const html = await readFile(join(output, page), 'utf8')
        assert.ok(html.includes(`<html lang="${locale}">`))
        assert.ok(html.includes('hreflang="en" href="/"'))
        assert.ok(html.includes('hreflang="ru" href="/ru/"'))
        assert.ok(html.includes(`href="https://manual.example.test/guide/${locale}/"`))
        assert.ok(html.includes(`href="https://manual.example.test/guide/${locale}/contributing/"`))
        assert.ok(html.includes('aria-current="page"'))
        assert.ok(html.includes('aria-label="RU — Русский"'))
        assert.ok(html.includes('aria-label="EN — English"'))
        assert.ok(html.includes('src="/brand/kedra.svg"'))
        assert.doesNotMatch(html, /%KEDRA_DOCS_|figma\.com\/api\/mcp|<script\b/)
      }

      assert.deepEqual(await readFile(join(output, 'brand/kedra.svg')),
        await readFile(join(root, 'public/brand/kedra.svg')))
      for (const [name, source] of [
        ['Inter-OFL.txt', '../../assets/editor/fonts/Inter-OFL.txt'],
        ['LICENSE-MIT', '../../LICENSE-MIT'],
        ['LICENSE-APACHE', '../../LICENSE-APACHE'],
      ]) {
        assert.deepEqual(await readFile(join(output, 'licenses', name)),
          await readFile(new URL(source, import.meta.url)))
      }

      const assets = await readdir(join(output, 'assets'))
      for (const weight of ['Regular', 'Medium', 'SemiBold']) {
        const bundled = assets.find(name => name.startsWith(`Inter-${weight}-`))
        assert.ok(bundled, `Missing Inter ${weight} source`)
        assert.deepEqual(await readFile(join(output, 'assets', bundled)),
          await readFile(new URL(`../../assets/editor/fonts/Inter-${weight}.ttf`, import.meta.url)))
      }
    } finally {
      await rm(output, { recursive: true, force: true })
    }
  })

test('invalid documentation roots fail before replacing an existing output', async () => {
  await mkdir(target, { recursive: true })
  const output = await mkdtemp(join(target, 'rejected-config-'))
  const sentinel = join(output, 'existing-output.txt')
  await writeFile(sentinel, 'Retain the existing preview on configuration rejection.\n')
  try {
    for (const docsURL of [
      'javascript:alert(1)',
      'https://user:password@example.test/',
      'https://example.test/?token=value',
      'https://example.test/#fragment',
      '/relative/manual/',
    ]) {
      const result = build(output, docsURL)
      assert.notEqual(result.status, 0, 'Invalid root unexpectedly built the landing')
      assert.match(result.stderr, /KEDRA_DOCS_URL/)
      assert.equal(await readFile(sentinel, 'utf8'),
        'Retain the existing preview on configuration rejection.\n')
    }
  } finally {
    await rm(output, { recursive: true, force: true })
  }
})
