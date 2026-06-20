//go:build windows

package main

import (
	"runtime"
	"sync"
	"syscall"
	"unsafe"

	"golang.org/x/sys/windows"
)

const (
	wmDestroy           = 0x0002
	wmClose             = 0x0010
	wmPowerBroadcast    = 0x0218
	pbtApmResumeSuspend = 0x0007
	pbtApmResumeAuto    = 0x0012
	hwndMessage         = ^uintptr(2) // (HWND)-3, a message-only window parent
)

type wndClassEx struct {
	cbSize        uint32
	style         uint32
	lpfnWndProc   uintptr
	cbClsExtra    int32
	cbWndExtra    int32
	hInstance     windows.Handle
	hIcon         windows.Handle
	hCursor       windows.Handle
	hbrBackground windows.Handle
	lpszMenuName  *uint16
	lpszClassName *uint16
	hIconSm       windows.Handle
}

type winMsg struct {
	hwnd    windows.Handle
	message uint32
	wParam  uintptr
	lParam  uintptr
	time    uint32
	pt      struct{ x, y int32 }
}

var (
	user32             = windows.NewLazySystemDLL("user32.dll")
	procRegisterClass  = user32.NewProc("RegisterClassExW")
	procCreateWindowEx = user32.NewProc("CreateWindowExW")
	procDefWindowProc  = user32.NewProc("DefWindowProcW")
	procGetMessage     = user32.NewProc("GetMessageW")
	procDispatchMsg    = user32.NewProc("DispatchMessageW")
	procDestroyWindow  = user32.NewProc("DestroyWindow")
	procPostMessage    = user32.NewProc("PostMessageW")
	procPostQuit       = user32.NewProc("PostQuitMessage")

	resumeMu   sync.Mutex
	resumeHwnd windows.Handle
)

// setupResumeListener creates a hidden message-only window and pumps its message
// loop on a dedicated OS thread so it can receive WM_POWERBROADCAST. Windows
// posts PBT_APMRESUMEAUTOMATIC (and PBT_APMRESUMESUSPEND on user-initiated wake)
// when the machine resumes; on either we tell the sidecar to reconnect.
func (a *App) setupResumeListener() {
	go func() {
		runtime.LockOSThread()
		defer runtime.UnlockOSThread()

		wndProc := syscall.NewCallback(func(hwnd windows.Handle, msg uint32, wParam, lParam uintptr) uintptr {
			switch msg {
			case wmPowerBroadcast:
				if wParam == pbtApmResumeAuto || wParam == pbtApmResumeSuspend {
					go a.onSystemResumed()
				}
				return 1
			case wmDestroy:
				procPostQuit.Call(0)
				return 0
			}
			ret, _, _ := procDefWindowProc.Call(uintptr(hwnd), uintptr(msg), wParam, lParam)
			return ret
		})

		className, _ := windows.UTF16PtrFromString("MeronPowerWindow")
		wc := wndClassEx{
			lpfnWndProc:   wndProc,
			hInstance:     0,
			lpszClassName: className,
		}
		wc.cbSize = uint32(unsafe.Sizeof(wc))
		if atom, _, err := procRegisterClass.Call(uintptr(unsafe.Pointer(&wc))); atom == 0 {
			a.logf("resume: RegisterClassEx: %v", err)
			return
		}

		hwnd, _, err := procCreateWindowEx.Call(
			0,
			uintptr(unsafe.Pointer(className)),
			uintptr(unsafe.Pointer(className)),
			0, 0, 0, 0, 0,
			hwndMessage, // message-only window: receives broadcasts, no UI
			0, 0, 0,
		)
		if hwnd == 0 {
			a.logf("resume: CreateWindowEx: %v", err)
			return
		}

		resumeMu.Lock()
		resumeHwnd = windows.Handle(hwnd)
		resumeMu.Unlock()

		var msg winMsg
		for {
			ret, _, _ := procGetMessage.Call(uintptr(unsafe.Pointer(&msg)), 0, 0, 0)
			if int32(ret) <= 0 { // 0 = WM_QUIT, -1 = error
				break
			}
			procDispatchMsg.Call(uintptr(unsafe.Pointer(&msg)))
		}
	}()
}

func (a *App) closeResumeListener() {
	resumeMu.Lock()
	hwnd := resumeHwnd
	resumeHwnd = 0
	resumeMu.Unlock()
	if hwnd != 0 {
		// Close from any thread; the message loop handles WM_DESTROY and quits.
		procPostMessage.Call(uintptr(hwnd), wmClose, 0, 0)
		procDestroyWindow.Call(uintptr(hwnd))
	}
}
