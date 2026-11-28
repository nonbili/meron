//go:build windows

package main

import (
	"fmt"
	"path/filepath"
	"unsafe"

	"github.com/go-ole/go-ole"
	"github.com/wailsapp/wails/v3/pkg/application"
	"golang.org/x/sys/windows"
)

var (
	attachmentDragShell       = windows.NewLazySystemDLL("shell32.dll")
	attachmentDragUser        = windows.NewLazySystemDLL("user32.dll")
	attachmentDragOLE         = windows.NewLazySystemDLL("ole32.dll")
	attachmentDragParseName   = attachmentDragShell.NewProc("SHParseDisplayName")
	attachmentDragLastID      = attachmentDragShell.NewProc("ILFindLastID")
	attachmentDragCreateData  = attachmentDragShell.NewProc("SHCreateDataObject")
	attachmentDragDrop        = attachmentDragShell.NewProc("SHDoDragDrop")
	attachmentDragKeyState    = attachmentDragUser.NewProc("GetAsyncKeyState")
	attachmentDragInit        = attachmentDragOLE.NewProc("OleInitialize")
	attachmentDragUninit      = attachmentDragOLE.NewProc("OleUninitialize")
	attachmentDragFree        = attachmentDragOLE.NewProc("CoTaskMemFree")
	attachmentDragIDataObject = ole.NewGUID("{0000010E-0000-0000-C000-000000000046}")
)

func startNativeAttachmentDrag(path string) (bool, error) {
	var copied bool
	err := application.InvokeSyncWithError(func() error {
		var err error
		copied, err = dragShellFile(path)
		return err
	})
	return copied, err
}

func dragShellFile(path string) (bool, error) {
	const leftMouseButton = 0x01
	const pressedMask = 0x8000
	const dropEffectCopy = 1
	state, _, _ := attachmentDragKeyState.Call(leftMouseButton)
	if state&pressedMask == 0 {
		return false, nil
	}
	hr, _, _ := attachmentDragInit.Call(0)
	if int32(hr) < 0 {
		return false, fmt.Errorf("initialize attachment drag: HRESULT %#x", uint32(hr))
	}
	defer attachmentDragUninit.Call()

	parent, err := attachmentDragPIDL(filepath.Dir(path))
	if err != nil {
		return false, err
	}
	defer attachmentDragFree.Call(parent)
	file, err := attachmentDragPIDL(path)
	if err != nil {
		return false, err
	}
	defer attachmentDragFree.Call(file)
	child, _, _ := attachmentDragLastID.Call(file)

	var data *ole.IUnknown
	hr, _, _ = attachmentDragCreateData.Call(parent, 1, uintptr(unsafe.Pointer(&child)), 0,
		uintptr(unsafe.Pointer(attachmentDragIDataObject)), uintptr(unsafe.Pointer(&data)))
	if int32(hr) < 0 {
		return false, fmt.Errorf("create attachment drag data: HRESULT %#x", uint32(hr))
	}
	defer data.Release()

	var effect uint32
	hr, _, _ = attachmentDragDrop.Call(0, uintptr(unsafe.Pointer(data)), 0, dropEffectCopy, uintptr(unsafe.Pointer(&effect)))
	if int32(hr) < 0 {
		return false, fmt.Errorf("drag attachment: HRESULT %#x", uint32(hr))
	}
	return effect == dropEffectCopy, nil
}

func attachmentDragPIDL(path string) (uintptr, error) {
	name, err := windows.UTF16PtrFromString(path)
	if err != nil {
		return 0, err
	}
	var pidl uintptr
	hr, _, _ := attachmentDragParseName.Call(uintptr(unsafe.Pointer(name)), 0, uintptr(unsafe.Pointer(&pidl)), 0, 0)
	if int32(hr) < 0 {
		return 0, fmt.Errorf("resolve attachment drag item: HRESULT %#x", uint32(hr))
	}
	return pidl, nil
}
