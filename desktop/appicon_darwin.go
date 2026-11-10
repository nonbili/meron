package main

import _ "embed"

// macOS gets its own artwork, scaled down to sit on Apple's icon grid so it
// matches neighbouring Dock icons; the shared icon fills more of the canvas.
//
//go:embed build/darwin/appicon.png
var appIconPNG []byte
