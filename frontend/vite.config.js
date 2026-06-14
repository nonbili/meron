import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'node:path'
import os from 'node:os'
import fs from 'node:fs'

// Where the Go bridge / Rust sidecar write cached attachment + feed images in
// Wails dev. Must match app.go's mediaDir(): MERON_MEDIA_DIR, else
// $XDG_CACHE_HOME (or ~/.cache) + meron-dev/attachments.
function mediaRoot() {
  if (process.env.MERON_MEDIA_DIR) return process.env.MERON_MEDIA_DIR
  const base = process.env.XDG_CACHE_HOME || path.join(os.homedir(), '.cache')
  return path.join(base, 'meron-dev', 'attachments')
}

const MEDIA_MIME = {
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.png': 'image/png',
  '.gif': 'image/gif',
  '.webp': 'image/webp',
  '.svg': 'image/svg+xml',
}

// In `wails dev` Wails serves the page from wails.localhost but proxies every
// request to this Vite server (see frontend:dev:serverUrl), bypassing the Go
// AssetServer's mediaHandler — so `/media/<key>` reaches Vite, not Go, and Vite's
// SPA fallback would answer with index.html and the image renders broken. Serve
// those files here from the same on-disk root instead.
// Production builds embed the frontend and fall through to the Go handler, so
// this plugin only hooks the dev server (configureServer).
function meronMedia() {
  const root = path.resolve(mediaRoot())
  return {
    name: 'meron-media',
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        if (!req.url || !req.url.startsWith('/media/')) return next()
        const rel = decodeURIComponent(req.url.slice('/media/'.length).split('?')[0])
        const filePath = path.resolve(root, rel)
        if (filePath !== root && !filePath.startsWith(root + path.sep)) {
          res.statusCode = 403
          return res.end('forbidden')
        }
        fs.readFile(filePath, (err, data) => {
          if (err) {
            res.statusCode = 404
            return res.end('not found')
          }
          res.setHeader('Content-Type', MEDIA_MIME[path.extname(filePath).toLowerCase()] || 'application/octet-stream')
          res.end(data)
        })
      })
    },
  }
}

export default defineConfig({
  plugins: [react(), tailwindcss(), meronMedia()],
})
