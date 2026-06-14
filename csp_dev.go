//go:build !production

package main

// contentSecurityPolicy for `wails dev`: Wails serves the page from
// wails.localhost and proxies to the Vite dev server, so this middleware runs on
// dev responses. The strict production policy breaks Vite, so we relax the two
// directives it trips on:
//   - script-src: Vite injects an inline @vitejs/plugin-react preamble and uses
//     eval for HMR module evaluation, so allow 'unsafe-inline' and 'unsafe-eval'.
//   - connect-src: HMR needs the Vite websocket (ws://127.0.0.1:5178) and the
//     Wails runtime websocket (ws://wails.localhost:34115), so allow ws:/wss:.
// Everything else mirrors csp_prod.go. This file is never compiled into a
// production build (which adds the `production` tag).
const contentSecurityPolicy = "default-src 'self'; " +
	"script-src 'self' 'unsafe-inline' 'unsafe-eval'; " +
	"style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; " +
	"font-src 'self' https://fonts.gstatic.com; " +
	"img-src 'self' data: blob: http: https:; " +
	"media-src 'self' data: blob: http: https:; " +
	"connect-src 'self' ws: wss:; " +
	"object-src 'none'; base-uri 'self'; frame-ancestors 'none'"
