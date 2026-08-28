//go:build production

package main

// contentSecurityPolicy locks the production webview down to same-origin assets,
// allowing only what the bundle actually needs: inline styles (Tailwind v4 injects
// a <style> and components use inline style attributes) and the Google Fonts hosts.
// All app data flows through Wails IPC, not the network, so connect-src stays 'self'.
const contentSecurityPolicy = "default-src 'self'; " +
	"script-src 'self'; " +
	"style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; " +
	"font-src 'self' https://fonts.gstatic.com; " +
	"img-src 'self' data: blob: http: https:; " +
	"media-src 'self' data: blob: http: https:; " +
	"connect-src 'self'; " +
	"object-src 'none'; base-uri 'self'; frame-ancestors 'none'"
