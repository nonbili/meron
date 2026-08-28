//go:build !windows

package main

// sweepReplacedExecutable only has work to do on Windows, where a running exe
// must be renamed rather than overwritten. Elsewhere the rename over the target
// leaves nothing behind.
func sweepReplacedExecutable() {}
