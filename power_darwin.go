//go:build darwin

package main

/*
#cgo CFLAGS: -x objective-c -Wno-deprecated-declarations
#cgo LDFLAGS: -framework Foundation -framework AppKit

void setupResumeObserver();
void teardownResumeObserver();
*/
import "C"

//export goSystemResumed
func goSystemResumed() {
	if globalApp == nil {
		return
	}
	// The Cocoa callback runs on the main thread; hop off it so the sidecar
	// round-trip doesn't block AppKit.
	go globalApp.onSystemResumed()
}

func (a *App) setupResumeListener() {
	C.setupResumeObserver()
}

func (a *App) closeResumeListener() {
	C.teardownResumeObserver()
}
