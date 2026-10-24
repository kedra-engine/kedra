import { defineConfig } from 'vite'
import { fileURLToPath } from 'node:url'

// VitePress 1.x preview ignores --host; Vite's static preview honors loopback.
export default defineConfig({
  root: fileURLToPath(new URL('../site/', import.meta.url)),
  appType: 'mpa',
  build: {
    outDir: fileURLToPath(new URL('../../target/documentation/site/', import.meta.url)),
  },
  preview: { host: '127.0.0.1', port: 4173, strictPort: true },
})
