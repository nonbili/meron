//go:build darwin

package main

/*
#cgo CFLAGS: -x objective-c -Wno-deprecated-declarations
#cgo LDFLAGS: -framework Foundation

#include <stdlib.h>

void setupNotificationDelegate();
void deliverUserNotification(char *title, char *body, char *account, char *threadID);
*/
import "C"

import "unsafe"

//export goNotificationClicked
func goNotificationClicked(account, threadID *C.char) {
	if globalApp == nil {
		return
	}
	// Copy C strings to Go strings synchronously on the callback thread before returning,
	// so the underlying C memory isn't invalidated or freed.
	accountGo := C.GoString(account)
	threadIDGo := C.GoString(threadID)

	go func() {
		globalApp.openThreadFromNotification(accountGo, threadIDGo)
	}()
}

func (a *App) setupNotificationListener() {
	C.setupNotificationDelegate()
}

func (a *App) closeNotificationListener() {}

// deliverNotification posts an NSUserNotification carrying the thread to open in
// its userInfo, which the center delegate hands back on click.
func (a *App) deliverNotification(n notification) {
	cTitle := C.CString(n.title)
	cBody := C.CString(n.body)
	cAccount := C.CString(n.account)
	cThreadID := C.CString(n.threadID)
	defer C.free(unsafe.Pointer(cTitle))
	defer C.free(unsafe.Pointer(cBody))
	defer C.free(unsafe.Pointer(cAccount))
	defer C.free(unsafe.Pointer(cThreadID))
	C.deliverUserNotification(cTitle, cBody, cAccount, cThreadID)
}
