//go:build windows

package main

import (
	"os"
	"strconv"
	"sync"
	"unsafe"

	"golang.org/x/sys/windows"
)

func setNativeWindowDark(dark bool) {
	if hwnd := findMainWindow(); hwnd != 0 {
		var enabled uint32
		if dark {
			enabled = 1
		}
		setDwmColor(hwnd, 20, enabled) // DWMWA_USE_IMMERSIVE_DARK_MODE
	}
}

const (
	dwmwaCaptionColor = 35
	dwmwaTextColor    = 36
	// DWMWA_COLOR_DEFAULT: hand the attribute back to the system.
	dwmColorDefault = 0xFFFFFFFF
	// Wails' window class, used as long as options.Windows sets no WindowClassName.
	wailsWindowClass = "wailsWindow"
)

var (
	dwmapi                       = windows.NewLazySystemDLL("dwmapi.dll")
	procDwmSetWindowAttribute    = dwmapi.NewProc("DwmSetWindowAttribute")
	procEnumWindows              = user32.NewProc("EnumWindows")
	procGetClassName             = user32.NewProc("GetClassNameW")
	procGetWindowThreadProcessId = user32.NewProc("GetWindowThreadProcessId")
)

// setNativeTitlebarColors paints the DWM caption in the side nav's colors.
// Wails only sets the caption color when options.Windows.CustomTheme is given,
// which main.go leaves unset, so this is the only writer. Windows 11 honors
// it; Windows 10 rejects the attribute and keeps its caption, which is fine.
// Anything but an opaque #rrggbb pair restores the system colors.
func setNativeTitlebarColors(bg, fg string) {
	hwnd := findMainWindow()
	if hwnd == 0 {
		return
	}
	caption, text := uint32(dwmColorDefault), uint32(dwmColorDefault)
	if opaqueHexColor.MatchString(bg) && opaqueHexColor.MatchString(fg) {
		caption, text = colorRef(bg), colorRef(fg)
	}
	setDwmColor(hwnd, dwmwaCaptionColor, caption)
	setDwmColor(hwnd, dwmwaTextColor, text)
}

func setDwmColor(hwnd uintptr, attr uintptr, value uint32) {
	procDwmSetWindowAttribute.Call(hwnd, attr, uintptr(unsafe.Pointer(&value)), unsafe.Sizeof(value))
}

// colorRef converts "#rrggbb" to a COLORREF (0x00bbggrr).
func colorRef(hex string) uint32 {
	rgb, _ := strconv.ParseUint(hex[1:], 16, 32)
	r, g, b := uint32(rgb>>16)&0xff, uint32(rgb>>8)&0xff, uint32(rgb)&0xff
	return b<<16 | g<<8 | r
}

// mainWindow caches the handle findMainWindow resolves: the window lives as
// long as the app, and theme changes would otherwise re-walk every window.
// Guarded by mainWindowMu, since invoke calls run concurrently.
var (
	mainWindowMu sync.Mutex
	mainWindow   uintptr
)

// findMainWindow returns this process's Wails top-level window, or 0. Wails v2
// does not expose the handle, and the class name alone could match another
// Wails app, so the owning process is checked too.
func findMainWindow() uintptr {
	mainWindowMu.Lock()
	defer mainWindowMu.Unlock()
	if mainWindow == 0 {
		procEnumWindows.Call(enumMainWindowCallback, uintptr(os.Getpid()))
	}
	return mainWindow
}

// Created once: every windows.NewCallback takes one of a fixed number of slots
// that are never released.
var enumMainWindowCallback = windows.NewCallback(func(hwnd uintptr, pid uintptr) uintptr {
	var owner uint32
	procGetWindowThreadProcessId.Call(hwnd, uintptr(unsafe.Pointer(&owner)))
	if uintptr(owner) != pid {
		return 1
	}
	var class [64]uint16
	n, _, _ := procGetClassName.Call(hwnd, uintptr(unsafe.Pointer(&class[0])), uintptr(len(class)))
	if n > 0 && windows.UTF16ToString(class[:n]) == wailsWindowClass {
		mainWindow = hwnd
		return 0
	}
	return 1
})

func refreshNativeTitlebarCss() {}
