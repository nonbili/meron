//go:build darwin

package main

/*
#cgo CFLAGS: -x objective-c -Wno-deprecated-declarations
#cgo LDFLAGS: -framework Foundation

#include <stdlib.h>

void setupNotificationDelegate();
void deliverUserNotification(char *title, char *body, char *account, char *threadID, char *identifier);
void removeUserNotification(char *identifier);
*/
import "C"

import (
	"unsafe"

	"github.com/google/uuid"
)

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
// its userInfo, which the center delegate hands back on click. Returns its
// identifier, the handle withdrawNotification takes.
func (a *App) deliverNotification(n notification) string {
	identifier := "meron-" + uuid.NewString()
	cTitle := C.CString(n.title)
	cBody := C.CString(n.body)
	cAccount := C.CString(n.account)
	cThreadID := C.CString(n.threadID)
	cIdentifier := C.CString(identifier)
	defer C.free(unsafe.Pointer(cTitle))
	defer C.free(unsafe.Pointer(cBody))
	defer C.free(unsafe.Pointer(cAccount))
	defer C.free(unsafe.Pointer(cThreadID))
	defer C.free(unsafe.Pointer(cIdentifier))
	C.deliverUserNotification(cTitle, cBody, cAccount, cThreadID, cIdentifier)
	return identifier
}

// withdrawNotification removes a delivered notification from Notification
// Center; a no-op once the user has dismissed it.
func (a *App) withdrawNotification(handle string) {
	cIdentifier := C.CString(handle)
	defer C.free(unsafe.Pointer(cIdentifier))
	C.removeUserNotification(cIdentifier)
}
