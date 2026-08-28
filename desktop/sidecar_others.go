//go:build !windows

package main

import "os/exec"

// hideSidecarConsole is a no-op off Windows: no other platform gives a spawned
// child a window of its own.
func hideSidecarConsole(*exec.Cmd) {}
