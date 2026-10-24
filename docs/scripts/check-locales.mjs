// Kedra policy: every published manual page has an EN/RU counterpart.
// Node's glob owns discovery; VitePress owns Markdown parsing and rendering.
import { glob } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../site/', import.meta.url))
const pages = {}
for (const locale of ['en', 'ru']) {
  pages[locale] = new Set()
  for await (const path of glob('**/*.md', { cwd: `${root}${locale}` })) {
    pages[locale].add(path.replaceAll('\\', '/'))
  }
}

const missing = []
for (const [locale, other] of [['en', 'ru'], ['ru', 'en']]) {
  for (const page of pages[locale]) {
    if (!pages[other].has(page)) missing.push(`${other}/${page}`)
  }
}
if (!pages.en.has('index.md') || !pages.ru.has('index.md')) {
  missing.push('Both locale index.md entry points are required')
}
if (missing.length) {
  console.error('FAIL: missing documentation translations:\n' + missing.sort().join('\n'))
  process.exitCode = 1
} else {
  console.log(`PASS: ${pages.en.size} documentation page pair(s) have matching EN/RU paths`)
}
