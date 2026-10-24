import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { defineConfig, loadEnv } from 'vite'

const root = fileURLToPath(new URL('./', import.meta.url))
const fontRoot = fileURLToPath(new URL('../assets/editor/fonts/', import.meta.url))

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, root, 'KEDRA_')
  const configured = process.env.KEDRA_DOCS_URL ?? env.KEDRA_DOCS_URL
  let docs
  try {
    docs = new URL(configured ?? 'http://127.0.0.1:4173/')
  } catch {
    throw new Error('KEDRA_DOCS_URL must be an absolute HTTP(S) URL')
  }
  if (!['http:', 'https:'].includes(docs.protocol)
      || docs.username || docs.password || docs.search || docs.hash) {
    throw new Error('KEDRA_DOCS_URL must be an HTTP(S) root without credentials, query or fragment')
  }
  if (!docs.pathname.endsWith('/')) docs.pathname += '/'

  // URL serialisation escapes markup delimiters; preserve literal ampersands in HTML attributes.
  const localeURL = locale => new URL(`${locale}/`, docs).href.replaceAll('&', '&amp;')
  const licenses = [
    ['Inter-OFL.txt', new URL('../assets/editor/fonts/Inter-OFL.txt', import.meta.url)],
    ['LICENSE-MIT', new URL('../LICENSE-MIT', import.meta.url)],
    ['LICENSE-APACHE', new URL('../LICENSE-APACHE', import.meta.url)],
  ]

  return {
    root,
    appType: 'mpa',
    plugins: [{
      name: 'kedra-landing-links-and-notices',
      transformIndexHtml: {
        order: 'pre',
        handler: html => html
          .replaceAll('%KEDRA_DOCS_EN%', localeURL('en'))
          .replaceAll('%KEDRA_DOCS_RU%', localeURL('ru')),
      },
      generateBundle() {
        for (const [name, source] of licenses) {
          this.emitFile({ type: 'asset', fileName: `licenses/${name}`, source: readFileSync(source) })
        }
      },
    }],
    server: {
      host: '127.0.0.1',
      port: 5174,
      strictPort: true,
      fs: { allow: [root, fontRoot] },
    },
    preview: { host: '127.0.0.1', port: 4174, strictPort: true },
    build: {
      outDir: fileURLToPath(new URL('../target/website/site/', import.meta.url)),
      emptyOutDir: true,
      rollupOptions: {
        input: {
          en: fileURLToPath(new URL('index.html', import.meta.url)),
          ru: fileURLToPath(new URL('ru/index.html', import.meta.url)),
        },
      },
    },
  }
})
