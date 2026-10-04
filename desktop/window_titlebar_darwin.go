//go:build darwin

package main

/*
#cgo CFLAGS: -x objective-c
#cgo LDFLAGS: -framework AppKit

void alignTrafficLights(void);
*/
import "C"

func alignNativeTitlebar() {
	C.alignTrafficLights()
}
