//go:build darwin

package main

/*
#cgo CFLAGS: -x objective-c
#cgo LDFLAGS: -framework AppKit

#include <stdlib.h>
int startAttachmentDrag(const char *path);
*/
import "C"

import (
	"unsafe"

	"github.com/wailsapp/wails/v3/pkg/application"
)

func startNativeAttachmentDrag(path string) (bool, error) {
	name := C.CString(path)
	defer C.free(unsafe.Pointer(name))
	var started bool
	application.InvokeSync(func() { started = C.startAttachmentDrag(name) != 0 })
	return started, nil
}
