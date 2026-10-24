// Complete local/publication assembly. Cargo and VitePress retain generation ownership.
import { spawnSync } from 'node:child_process'
import { access, cp, lstat, mkdir, realpath, rm, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { join, resolve } from 'node:path'

const docs = fileURLToPath(new URL('../', import.meta.url))
const root = resolve(fileURLToPath(new URL('../../', import.meta.url)))
const output = join(root, 'target/documentation')
const cargo = join(output, 'cargo')
const site = join(output, 'site')
const manifest = join(output, 'api-manifest.json')
const rustdoc = join(cargo, 'doc')
const rustdocFlags = '-D warnings -D rustdoc::broken_intra_doc_links'
let outputOwned = false

// Fixed generated destinations and every parent below the authorized repository.
// This is preflight, not protection against concurrent malicious replacement.
async function preflight() {
  outputOwned = false
  if (await realpath(root) !== root) throw new Error('API output root must be canonical')
  for (const [relative, directory] of [
    ['target', true], ['target/documentation', true],
    ['target/documentation/cargo', true], ['target/documentation/cargo/doc', true],
    ['target/documentation/site', true], ['target/documentation/site/api', true],
    ['target/documentation/site/api/index.html', false],
    ['target/documentation/site/api/LICENSE-MIT', false],
    ['target/documentation/site/api/LICENSE-APACHE', false],
    ['target/documentation/api-manifest.json', false], ['target/documentation/cache', true],
  ]) {
    const path = join(root, relative)
    let info
    try { info = await lstat(path) } catch (error) {
      if (error.code === 'ENOENT') continue
      throw error
    }
    if (info.isSymbolicLink() || (directory ? !info.isDirectory() : !info.isFile())
      || await realpath(path) !== path) {
      throw new Error(`API output preflight rejects redirected or invalid path: ${path}`)
    }
  }
  outputOwned = true
}

function run(command, args, options = {}) {
  console.log(`+ ${command} ${args.join(' ')}`)
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit', ...options })
  if (result.error) throw result.error
  if (result.status !== 0) throw new Error(`${command} failed (${result.status ?? result.signal})`)
  return result.stdout
}

try {
  console.log('+ validate API output ownership')
  await preflight()
  // These fixed paths belong solely to this generated preview, never target/doc.
  await rm(site, { recursive: true, force: true })
  await rm(manifest, { force: true })
  run(process.execPath, ['scripts/check-locales.mjs'], { cwd: docs })
  await preflight()
  await rm(rustdoc, { recursive: true, force: true })
  run('cargo', ['doc', '--workspace', '--no-deps', '--all-features', '--locked',
    '--target-dir', cargo], {
    env: { ...process.env, RUSTDOCFLAGS: rustdocFlags },
  })
  await preflight()
  const metadata = JSON.parse(run('cargo', ['metadata', '--all-features', '--format-version', '1',
    '--locked'], { stdio: ['ignore', 'pipe', 'inherit'], encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024 }))
  const crates = metadata.packages
    .filter(pkg => metadata.workspace_members.includes(pkg.id))
    .flatMap(pkg => pkg.targets
      .filter(target => target.doc && target.kind.includes('lib'))
      .map(target => ({ name: target.name, version: pkg.version,
        link: `/api/${target.name}/index.html` })))
    .sort((a, b) => a.name.localeCompare(b.name, 'en'))
  if (!crates.length) throw new Error('Cargo metadata contains no documentable workspace libraries')
  for (const crate of crates) {
    await access(join(cargo, 'doc', crate.name, 'index.html'))
  }
  await preflight()
  await mkdir(output, { recursive: true })
  await writeFile(manifest, JSON.stringify(crates, null, 2) + '\n')
  run(process.execPath, ['node_modules/vitepress/bin/vitepress.js', 'build', '.'], {
    cwd: docs, env: { ...process.env, KEDRA_DOCS_API: '1' },
  })
  await preflight()
  // Copy the whole rustdoc tree, including sources, search, fonts and legal files.
  await cp(rustdoc, join(site, 'api'), { recursive: true, errorOnExist: true,
    force: false })
  await preflight()
  const { adaptApiHtml } = await import('./adapt-api-html.mjs')
  await adaptApiHtml(metadata)
  await preflight()
  // Stable rustdoc has no workspace index option; help/settings link to this root.
  await writeFile(join(site, 'api/index.html'), '<!doctype html>\n<html lang="en">\n'
    + '<meta charset="utf-8"><title>Kedra Rust API reference</title>\n'
    + '<h1>Kedra Rust API reference</h1>\n'
    + '<p>English API contracts; choose a guide entry for the generated crate list:</p>\n'
    + '<ul><li><a href="/en/api/">English</a></li>'
    + '<li><a href="/ru/api/" lang="ru">Русский</a></li></ul>\n</html>\n')
  for (const license of ['LICENSE-MIT', 'LICENSE-APACHE']) {
    await cp(join(root, license), join(site, 'api', license))
  }
  console.log(`PASS: manual and ${crates.length} English crate references assembled in ${site}`)
} catch (error) {
  // A failed complete build must not leave a publishable, stale or partial preview.
  // Never retry cleanup when preflight could not establish destination ownership.
  if (outputOwned) {
    try {
      await preflight()
      await rm(site, { recursive: true, force: true })
      await rm(manifest, { force: true })
    } catch (cleanupError) {
      console.error(`FAIL: API preview cleanup refused or failed: ${cleanupError.message}`)
    }
  }
  console.error(`FAIL: API preview build: ${error.message}`)
  process.exitCode = 1
}
