//go:build windows

package main

import (
	"runtime"
	"sync"
	"syscall"
	"unsafe"

	"github.com/wailsapp/wails/v3/pkg/w32"
	"golang.org/x/sys/windows"
)

var (
	ole32                = windows.NewLazySystemDLL("ole32.dll")
	procCoInitializeEx   = ole32.NewProc("CoInitializeEx")
	procCoUninitialize   = ole32.NewProc("CoUninitialize")
	procCoCreateInstance = ole32.NewProc("CoCreateInstance")
)

const (
	coinitApartmentThreaded = 0x2
	clsctxInprocServer      = 0x1
)

// CLSID_TaskbarList {56FDF344-FD6D-11D0-958A-006097C9A090}
var clsidTaskbarList = windows.GUID{
	Data1: 0x56FDF344,
	Data2: 0xFD6D,
	Data3: 0x11D0,
	Data4: [8]byte{0x95, 0x8A, 0x00, 0x60, 0x97, 0xC9, 0xA0, 0x90},
}

// IID_ITaskbarList3 {EA1AFB91-9E28-4B86-90E9-9E9F8A5EEFAF}
var iidTaskbarList3 = windows.GUID{
	Data1: 0xEA1AFB91,
	Data2: 0x9E28,
	Data3: 0x4B86,
	Data4: [8]byte{0x90, 0xE9, 0x9E, 0x9F, 0x8A, 0x5E, 0xEF, 0xAF},
}

// iTaskbarList3Vtbl matches the Windows ITaskbarList3 COM vtable layout.
type iTaskbarList3Vtbl struct {
	QueryInterface        uintptr // 0
	AddRef                uintptr // 1
	Release               uintptr // 2
	HrInit                uintptr // 3
	AddTab                uintptr // 4
	DeleteTab             uintptr // 5
	ActivateTab           uintptr // 6
	SetActiveAlt          uintptr // 7
	MarkFullscreenWindow  uintptr // 8
	SetProgressValue      uintptr // 9
	SetProgressState      uintptr // 10
	RegisterTab           uintptr // 11
	UnregisterTab         uintptr // 12
	SetTabOrder           uintptr // 13
	SetTabActive          uintptr // 14
	ThumbBarAddButtons    uintptr // 15
	ThumbBarUpdateButtons uintptr // 16
	ThumbBarSetImageList  uintptr // 17
	SetOverlayIcon        uintptr // 18
	SetThumbnailTooltip   uintptr // 19
	SetThumbnailClip      uintptr // 20
}

type iTaskbarList3 struct {
	vtbl *iTaskbarList3Vtbl
}

func (t *iTaskbarList3) hrInit() error {
	hr, _, _ := syscall.SyscallN(t.vtbl.HrInit, uintptr(unsafe.Pointer(t)))
	if int32(hr) < 0 {
		return syscall.Errno(hr)
	}
	return nil
}

func (t *iTaskbarList3) setOverlayIcon(hwnd uintptr, hicon w32.HICON, desc *uint16) error {
	hr, _, _ := syscall.SyscallN(
		t.vtbl.SetOverlayIcon,
		uintptr(unsafe.Pointer(t)),
		hwnd,
		uintptr(hicon),
		uintptr(unsafe.Pointer(desc)),
	)
	if int32(hr) < 0 {
		return syscall.Errno(hr)
	}
	return nil
}

func (t *iTaskbarList3) release() {
	syscall.SyscallN(t.vtbl.Release, uintptr(unsafe.Pointer(t)))
}

// taskbarBadgeWorker runs all ITaskbarList3 STA COM calls on one pinned OS
// thread and coalesces rapid label updates to the latest requested value.
var taskbarBadgeWorker struct {
	once    sync.Once
	mu      sync.Mutex
	pending string
	wake    chan struct{}
}

func platformSetTaskbarBadgeLabel(label string) {
	taskbarBadgeWorker.once.Do(func() {
		taskbarBadgeWorker.wake = make(chan struct{}, 1)
		go runTaskbarBadgeWorker()
	})
	taskbarBadgeWorker.mu.Lock()
	taskbarBadgeWorker.pending = label
	taskbarBadgeWorker.mu.Unlock()
	select {
	case taskbarBadgeWorker.wake <- struct{}{}:
	default:
	}
}

func runTaskbarBadgeWorker() {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	hr, _, _ := procCoInitializeEx.Call(0, coinitApartmentThreaded)
	if int32(hr) >= 0 {
		defer procCoUninitialize.Call()
	}

	var tbl *iTaskbarList3
	hr, _, _ = procCoCreateInstance.Call(
		uintptr(unsafe.Pointer(&clsidTaskbarList)),
		0,
		clsctxInprocServer,
		uintptr(unsafe.Pointer(&iidTaskbarList3)),
		uintptr(unsafe.Pointer(&tbl)),
	)
	if int32(hr) < 0 || tbl == nil {
		return
	}
	defer tbl.release()
	if tbl.hrInit() != nil {
		return
	}

	for range taskbarBadgeWorker.wake {
		taskbarBadgeWorker.mu.Lock()
		label := taskbarBadgeWorker.pending
		taskbarBadgeWorker.mu.Unlock()

		hwnd := findMainWindow()
		if hwnd == 0 {
			continue
		}
		if label == "" {
			_ = tbl.setOverlayIcon(hwnd, 0, nil)
			continue
		}
		pngBytes, err := renderTaskbarBadgePNG(label)
		if err != nil {
			continue
		}
		hicon, err := w32.CreateSmallHIconFromImage(pngBytes)
		if err != nil || hicon == 0 {
			continue
		}
		desc, _ := windows.UTF16PtrFromString(label)
		_ = tbl.setOverlayIcon(hwnd, hicon, desc)
		// ITaskbarList3::SetOverlayIcon copies the icon; the caller owns hicon.
		w32.DestroyIcon(hicon)
	}
}
