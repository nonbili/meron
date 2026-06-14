//go:build !embed_sidecar

package main

// embeddedSidecar is empty in dev builds; the sidecar is found via the
// MERON_CORE_SERVER env var or the relative debug path. See sidecar_embed.go
// for the release build that bakes the binary in.
var embeddedSidecar []byte
