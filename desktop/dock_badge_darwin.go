//go:build darwin

package main

/*
#cgo CFLAGS: -x objective-c
#cgo LDFLAGS: -framework AppKit

#include <stdlib.h>

void setDockBadgeLabel(const char *label);
*/
import "C"
import "unsafe"

func platformSetDockBadgeLabel(label string) {
	cLabel := C.CString(label)
	defer C.free(unsafe.Pointer(cLabel))
	C.setDockBadgeLabel(cLabel)
}
