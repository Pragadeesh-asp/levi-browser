import { defineConfig } from 'vite'
import { fileURLToPath } from 'node:url'
import { resolve } from 'node:path'

const projectRoot = fileURLToPath(new URL('.', import.meta.url))

export default defineConfig({
  server: {
    host: '127.0.0.1',
    port: 4173,
    strictPort: true,
  },
  build: {
    rollupOptions: {
      input: {
        main: resolve(projectRoot, 'index.html'),
        browserAlpha: resolve(projectRoot, 'browser-alpha.html'),
        browserBeta: resolve(projectRoot, 'browser-beta.html'),
        browserProfile: resolve(projectRoot, 'browser-profile.html'),
      },
    },
  },
})
