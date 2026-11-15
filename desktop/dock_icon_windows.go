//go:build windows

package main

import (
	"os"
	"sync"
	"time"
	"unsafe"

	"github.com/wailsapp/wails/v3/pkg/w32"
	"golang.org/x/sys/windows"
)

// The taskbar and Alt+Tab draw each window's own icon, which a running app can
// replace. A pinned shortcut and the Start menu read the icon stored in the
// executable and keep the shipped one, as does the taskbar once Meron has quit.
func appIconStyleSupported() bool { return true }

const (
	wmSetIcon = 0x0080
	iconSmall = 0
	iconBig   = 1
)

type iconPair struct{ small, big w32.HICON }

// windowIcons holds the icons every Meron window should carry. Windows opened
// later (a composer, an .eml reader) get Wails' default at creation, so the
// current pair is kept and handed out again by refreshWindowIcons.
var windowIcons struct {
	mu      sync.Mutex
	current iconPair
}

// retiredIconLifetime is how long a replaced pair outlives its replacement.
// The icons reach windows as posted messages, so a window may still be about
// to take the old pair, or still be drawing it, when the new one is handed
// out. Every window has been sent the new pair by then, and a posted message
// is handled within milliseconds unless the window is hung.
const retiredIconLifetime = 10 * time.Second

func (p iconPair) destroy() {
	if p.small != 0 {
		w32.DestroyIcon(p.small)
	}
	if p.big != 0 {
		w32.DestroyIcon(p.big)
	}
}

// platformSetAppIcon draws a PNG as the icon of every Meron window; nil
// restores the shipped icon.
func platformSetAppIcon(_ string, png []byte) {
	if len(png) == 0 {
		png = appIconPNG
	}
	small, smallErr := w32.CreateSmallHIconFromImage(png)
	big, bigErr := w32.CreateLargeHIconFromImage(png)
	if smallErr != nil || bigErr != nil || small == 0 || big == 0 {
		iconPair{small, big}.destroy()
		return
	}
	windowIcons.mu.Lock()
	retired := windowIcons.current
	windowIcons.current = iconPair{small, big}
	windowIcons.mu.Unlock()
	refreshWindowIcons()
	if retired.big != 0 || retired.small != 0 {
		time.AfterFunc(retiredIconLifetime, retired.destroy)
	}
}

// refreshWindowIcons gives the chosen icon to every window this process owns.
// It does nothing until a style other than the shipped one has been applied.
func refreshWindowIcons() {
	windowIcons.mu.Lock()
	defer windowIcons.mu.Unlock()
	if windowIcons.current.big == 0 {
		return
	}
	procEnumWindows.Call(enumWindowIconCallback, uintptr(os.Getpid()))
}

// Created once, like enumMainWindowCallback: callback slots are never released.
// Runs with windowIcons.mu held, so it posts rather than sends: a window's
// thread may itself be waiting for the lock.
var enumWindowIconCallback = windows.NewCallback(func(hwnd uintptr, pid uintptr) uintptr {
	var owner uint32
	procGetWindowThreadProcessId.Call(hwnd, uintptr(unsafe.Pointer(&owner)))
	if uintptr(owner) != pid {
		return 1
	}
	var class [64]uint16
	n, _, _ := procGetClassName.Call(hwnd, uintptr(unsafe.Pointer(&class[0])), uintptr(len(class)))
	if n > 0 && windows.UTF16ToString(class[:n]) == wailsWindowClass {
		procPostMessage.Call(hwnd, wmSetIcon, iconSmall, uintptr(windowIcons.current.small))
		procPostMessage.Call(hwnd, wmSetIcon, iconBig, uintptr(windowIcons.current.big))
	}
	return 1
})
