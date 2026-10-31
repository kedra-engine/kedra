// Rust 1.98.0 output compatibility, not a general HTML rewriter. Vue's existing
// locked HTML parser owns tokenization/offsets; original bytes are otherwise kept.
import { NodeTypes, parse, transform } from '@vue/compiler-dom'
import { glob, readFile, realpath, writeFile } from 'node:fs/promises'
import { join, resolve, sep } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const root = resolve(fileURLToPath(new URL('../../target/documentation/site/api/',
  import.meta.url)))
const versions = { tracing: '0.1.44', yansi: '1.0.1', bevy_reflect: '0.20.0',
  bevy_state: '0.20.0', 'futures-util': '0.3.34', 'futures-core': '0.3.34' }
// Provenance: pinned crates' instrument.rs, macros.rs, type_path.rs, reflect.rs,
// info/typed.rs, state/state_set.rs and future/future/mod.rs (futures-util).
// See documentation-infrastructure.md.
const links = new Map([
  ['dispatcher#setting-the-default-subscriber', {
    crate: 'tracing', path: 'dispatcher/index.html#setting-the-default-subscriber',
    trait: 'WithSubscriber', members: ['method.with_current_subscriber'], text: 'default',
  }],
  ['crate#per-style', {
    crate: 'yansi', path: 'index.html#per-style', trait: 'Paint',
    members: ['method.whenever'], text: 'crate level docs',
  }],
  ['TypePath#anonymity', {
    crate: 'bevy_reflect', path: 'trait.TypePath.html#anonymity', trait: 'TypePath',
    members: ['method.type_ident', 'method.crate_name', 'method.module_path'], text: 'anonymous',
  }],
  ['Reflect', {
    crate: 'bevy_reflect', path: 'trait.Reflect.html', trait: 'PartialReflect',
    members: ['method.try_into_reflect', 'method.try_as_reflect', 'method.try_as_reflect_mut'],
    text: 'fully-reflected',
  }],
  ['TypeInfo', {
    crate: 'bevy_reflect', path: 'enum.TypeInfo.html', trait: 'Typed',
    members: ['method.type_info'], text: 'info',
  }],
  ['%60States::DEPENDENCY_DEPTH%60', {
    crate: 'bevy_state', path: 'state/trait.States.html#associatedconstant.DEPENDENCY_DEPTH',
    trait: 'StateSet', members: ['associatedconstant.SET_DEPENDENCY_DEPTH'],
    text: 'DEPENDENCY_DEPTH',
  }],
  ['futures_core::future::TryFuture', {
    crate: 'futures-core', module: 'futures_core', path: 'future/trait.TryFuture.html',
    trait: 'FutureExt', members: ['method.unit_error', 'method.never_error'],
    memberText: {
      'method.unit_error': 'TryFuture<Ok = T, Error = ()>',
      'method.never_error': 'TryFuture<Ok = T, Error = Never>',
    },
  }],
])

const attribute = (node, name) => node.props.find(prop =>
  prop.type === NodeTypes.ATTRIBUTE && prop.name === name)
const value = (node, name) => attribute(node, name)?.value?.content
const child = (node, tag) => node?.children.find(item =>
  item.type === NodeTypes.ELEMENT && item.tag === tag)
const sectionId = node => {
  const section = child(child(node, 'summary'), 'section')
  return section && value(section, 'id')
}

function elements(html, visit) {
  const ancestors = []
  transform(parse(html, { parseMode: 'html', delimiters: ['\u0000', '\u0001'] }), {
    nodeTransforms: [node => {
      if (node.type !== NodeTypes.ELEMENT) return
      visit(node, ancestors)
      ancestors.push(node)
      return () => { ancestors.pop() }
    }],
  })
}

async function contained(path) {
  if (!path.startsWith(root + sep) || await realpath(path) !== path) {
    throw new Error(`API adapter rejects escaped or symlinked path: ${path}`)
  }
}

function apply(html, edits) {
  let boundary = html.length
  for (const { start, end, text } of edits.sort((a, b) => b.start - a.start)) {
    if (start < 0 || end < start || end > boundary) {
      throw new Error('API adapter rejects overlapping or invalid HTML offsets')
    }
    html = html.slice(0, start) + text + html.slice(end)
    boundary = start
  }
  return html
}

function checkToolchain(html, path) {
  if (!html.includes('data-rustdoc-version="1.98.0 (88d9e12ae 2026-08-18)"')) {
    throw new Error(`API adapter requires reviewed Rust 1.98.0 output: ${path}`)
  }
}

/** Adapt only the copied preview after validating all edits, never Cargo's tree. */
export async function adaptApiHtml(metadata) {
  if (await realpath(root) !== root) throw new Error('API output root must not be a symlink')
  for (const [name, version] of Object.entries(versions)) {
    const packages = metadata.packages.filter(pkg => pkg.name === name)
    if (packages.length !== 1 || packages[0].version !== version
      || packages[0].source !== 'registry+https://github.com/rust-lang/crates.io-index') {
      throw new Error(`API adapter requires reviewed ${name} ${version} registry source`)
    }
  }
  const planned = new Map()
  const ranges = new Map()
  let repaired = 0
  for await (const relative of glob('**/*.html', { cwd: root })) {
    const path = join(root, relative)
    await contained(path)
    const html = await readFile(path, 'utf8')
    const edits = []
    elements(html, (node, ancestors) => {
      if (node.tag !== 'a') return
      const href = value(node, 'href')
      if (!href) return
      const rule = links.get(href)
      if (rule) {
        checkToolchain(html, path)
        const member = ancestors.findLast(item => item.tag === 'details')
        const implementation = ancestors.findLast(item => item.tag === 'details'
          && value(item, 'class')?.split(' ').includes('implementors-toggle'))
        const text = node.children.map(item => item.type === NodeTypes.TEXT ? item.content
          : item.tag === 'code' && item.children.length === 1
            && item.children[0].type === NodeTypes.TEXT ? item.children[0].content : '').join('')
        if (!rule.members.includes(sectionId(member))
          || !sectionId(implementation)?.startsWith(`impl-${rule.trait}-for-`)
          || text !== (rule.memberText?.[sectionId(member)] ?? rule.text)) {
          throw new Error(`API adapter rejects unrecognized inherited ${href} shape in ${path}`)
        }
        const prop = attribute(node, 'href')
        const module = rule.module ?? rule.crate
        const url = `https://docs.rs/${rule.crate}/${versions[rule.crate]}/${module}/${rule.path}`
        edits.push({ start: prop.loc.start.offset, end: prop.loc.end.offset,
          text: `href="${url}"` })
        repaired++
      }
      if (/^[a-z][a-z\d+.-]*:/i.test(href) || href.startsWith('//')) return
      const url = new URL(href, pathToFileURL(path))
      if (!/^#[0-9]+-[0-9]+$/.test(url.hash)) return
      checkToolchain(html, path)
      const target = fileURLToPath(url)
      if (url.search || !target.startsWith(join(root, 'src') + sep)
        || !target.endsWith('.rs.html')) {
        throw new Error(`API adapter rejects unrecognized source range: ${href}`)
      }
      const [start, end] = url.hash.slice(1).split('-').map(Number)
      if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 1 || end < start) {
        throw new Error(`API adapter rejects invalid source range: ${href}`)
      }
      if (!ranges.has(target)) ranges.set(target, new Map())
      ranges.get(target).set(url.hash.slice(1), [start, end])
    })
    if (edits.length) planned.set(path, apply(html, edits))
  }
  let aliases = 0
  for (const [path, fragments] of ranges) {
    await contained(path)
    const html = planned.get(path) ?? await readFile(path, 'utf8')
    checkToolchain(html, path)
    const lines = new Map()
    const ids = new Set()
    elements(html, node => {
      const id = value(node, 'id')
      if (!id) return
      if (ids.has(id)) throw new Error(`API adapter rejects duplicate source ID ${id}: ${path}`)
      ids.add(id)
      if (!/^[1-9][0-9]*$/.test(id)) return
      if (node.tag !== 'a' || value(node, 'href') !== `#${id}`
        || !attribute(node, 'data-nosnippet') || node.children.length !== 1
        || node.children[0].type !== NodeTypes.TEXT || node.children[0].content !== id) {
        throw new Error(`API adapter rejects unrecognized source line ${id}: ${path}`)
      }
      lines.set(Number(id), node)
    })
    const starts = new Map()
    for (const [fragment, [start, end]] of fragments) {
      if (ids.has(fragment) || end > lines.size) {
        throw new Error(`API adapter rejects conflicting or missing source range ${fragment}: ${path}`)
      }
      let previous = -1
      for (let line = start; line <= end; line++) {
        const offset = lines.get(line)?.loc.start.offset
        if (offset === undefined || offset <= previous) {
          throw new Error(`API adapter cannot resolve ordered source line ${line}: ${path}`)
        }
        previous = offset
      }
      if (!starts.has(start)) starts.set(start, [])
      starts.get(start).push(fragment)
    }
    const edits = []
    for (const [start, fragments] of starts) {
      const node = lines.get(start)
      edits.push({ start: node.loc.start.offset, end: node.loc.end.offset,
        text: fragments.map(id => `<span id="${id}">`).join('') + node.loc.source
          + '</span>'.repeat(fragments.length) })
      aliases += fragments.length
    }
    planned.set(path, apply(html, edits))
  }
  for (const [path, html] of planned) {
    await contained(path)
    await writeFile(path, html)
  }
  console.log(`PASS: API HTML compatibility: ${aliases} real source-range aliases, `
    + `${repaired} versioned upstream links; original Cargo output unchanged`)
}
