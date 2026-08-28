//go:build darwin

package main

/*
#cgo CFLAGS: -x objective-c -Wno-deprecated-declarations
#cgo LDFLAGS: -framework AppKit -framework Foundation

#include <stdlib.h>

void *readPasteboardImage(int *outLen, int *outIsPNG);
int writePasteboardImage(const char *path, int isJPEG);
int openPath(const char *path);
*/
import "C"

import (
	"errors"
	"unsafe"
)

// readClipboardImageDarwin pulls an image off the general pasteboard. It returns
// nil data (and no error) when the clipboard holds no image, matching the other
// platforms' helpers.
func readClipboardImageDarwin() ([]byte, string, error) {
	var length, isPNG C.int
	buffer := C.readPasteboardImage(&length, &isPNG)
	if buffer == nil || length <= 0 {
		return nil, "", nil
	}
	defer C.free(buffer)

	data := C.GoBytes(buffer, length)
	if isPNG != 0 {
		return data, "png", nil
	}
	return data, "jpg", nil
}

func copyImageToClipboardDarwin(path, mime string) error {
	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))

	isJPEG := C.int(0)
	if mime == "image/jpeg" || mime == "image/jpg" {
		isJPEG = 1
	}
	if C.writePasteboardImage(cPath, isJPEG) == 0 {
		return errors.New("could not put the image on the clipboard")
	}
	return nil
}

// openSystemFileDarwin hands the file to its default app via NSWorkspace.
func openSystemFileDarwin(path string) error {
	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))

	if C.openPath(cPath) == 0 {
		return errors.New("no application available to open this file")
	}
	return nil
}
