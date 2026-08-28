//go:build darwin || linux

package main

import (
	"os"
	"os/exec"
	"strconv"
	"strings"
	"syscall"
)

// scheduleRelaunch spawns a detached shell that waits for this process to exit
// before running argv. The wait matters: Meron holds a single-instance lock, so
// a new copy started while the old one is still alive would just be forwarded to
// it instead of replacing it.
func scheduleRelaunch(argv ...string) error {
	if len(argv) == 0 {
		return nil
	}
	script := "while kill -0 " + strconv.Itoa(os.Getpid()) + " 2>/dev/null; do sleep 0.2; done; exec " + shellQuoteAll(argv)
	cmd := exec.Command("/bin/sh", "-c", script)
	cmd.SysProcAttr = &syscall.SysProcAttr{Setsid: true}
	cmd.Stdin = nil
	cmd.Stdout = nil
	cmd.Stderr = nil
	return cmd.Start()
}

func shellQuoteAll(args []string) string {
	quoted := make([]string, 0, len(args))
	for _, arg := range args {
		quoted = append(quoted, "'"+strings.ReplaceAll(arg, "'", `'\''`)+"'")
	}
	return strings.Join(quoted, " ")
}
