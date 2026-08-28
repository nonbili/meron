package main

import (
	"os/exec"
	"syscall"
)

// createNoWindow is CREATE_NO_WINDOW: the child gets no console at all, rather
// than an invisible one.
const createNoWindow = 0x08000000

// hideSidecarConsole keeps the sidecar from flashing a console window. It is a
// console-subsystem binary, so Windows hands it a console of its own whenever a
// GUI app spawns it — a black window that pops up on every launch.
func hideSidecarConsole(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{HideWindow: true, CreationFlags: createNoWindow}
}
