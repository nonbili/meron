//go:build windows

package main

import (
	"os"
	"sync"
	"unsafe"

	"github.com/wailsapp/wails/v3/pkg/w32"
	"golang.org/x/sys/windows"

	"meron/internal/appicon"
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
//
// Pairs are made once per variant and never destroyed: there are four at most,
// and a window may still be drawing the one that was just replaced.
var windowIcons struct {
	mu      sync.Mutex
	made    map[appicon.Variant]iconPair
	current iconPair
}

// platformSetAppIcon draws a PNG as the icon of every Meron window; nil
// restores the shipped icon.
func platformSetAppIcon(variant appicon.Variant, png []byte) {
	if len(png) == 0 {
		png = appIconPNG
	}
	windowIcons.mu.Lock()
	pair, ok := windowIcons.made[variant]
	if !ok {
		small, smallErr := w32.CreateSmallHIconFromImage(png)
		big, bigErr := w32.CreateLargeHIconFromImage(png)
		if smallErr != nil || bigErr != nil || small == 0 || big == 0 {
			windowIcons.mu.Unlock()
			return
		}
		pair = iconPair{small, big}
		if windowIcons.made == nil {
			windowIcons.made = map[appicon.Variant]iconPair{}
		}
		windowIcons.made[variant] = pair
	}
	windowIcons.current = pair
	windowIcons.mu.Unlock()
	refreshWindowIcons()
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
