package main

import (
	"os"

	"golang.org/x/sys/windows"
)

func attachHelpConsole() {
	// GUI releases have no console by default. Preserve valid inherited
	// stdout handles so pipes and redirection continue to work.
	if _, err := os.Stdout.Stat(); err == nil {
		return
	}
	attachConsole := windows.NewLazySystemDLL("kernel32.dll").NewProc("AttachConsole")
	const attachParentProcess = ^uint32(0)
	if attached, _, _ := attachConsole.Call(uintptr(attachParentProcess)); attached == 0 {
		return
	}
	if handle, err := windows.GetStdHandle(windows.STD_OUTPUT_HANDLE); err == nil && handle != 0 && handle != windows.InvalidHandle {
		os.Stdout = os.NewFile(uintptr(handle), "stdout")
	}
}
