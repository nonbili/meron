//go:build !production

package main

// devBuild is true for `wails dev` and plain `bun run build --dev`: builds a
// developer runs from the repository, whose core is the freshly built debug
// binary rather than one shipped with the app.
const devBuild = true
