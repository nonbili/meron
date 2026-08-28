//go:build windows

package main

import (
	"os"
	"strings"

	toast "git.sr.ht/~jackmordaunt/go-toast/v2"
	"github.com/gen2brain/beeep"
)

// Stable activation GUID for Meron's toast COM activator. Must not change across
// releases or previously-registered activations break.
const toastActivatorGUID = "{6f3b1c2a-9d4e-4f8b-bf2a-2e7c5a1d9e30}"

// notifyArgSeparator joins account+threadID into the toast's ActivationArguments
// (returned verbatim to the activation callback). Neither value contains a
// newline, so it round-trips cleanly.
const notifyArgSeparator = "\n"

// setupNotificationListener registers Meron's app metadata + activation GUID in
// the registry and wires the in-process callback Windows invokes when the user
// clicks a toast. The app is long-running, so the COM server is live at click
// time and no out-of-process relaunch is needed.
func (a *App) setupNotificationListener() {
	exe, _ := os.Executable()
	if err := toast.SetAppData(toast.AppData{
		AppID:         "Meron",
		GUID:          toastActivatorGUID,
		ActivationExe: exe,
		IconPath:      notifyIcon(),
	}); err != nil {
		a.logf("toast SetAppData failed: %v", err)
	}

	toast.SetActivationCallback(func(args string, _ []toast.UserData) {
		account, threadID := parseNotifyArgs(args)
		a.openThreadFromNotification(account, threadID)
	})
}

func (a *App) closeNotificationListener() {}

// deliverNotification shows a Windows toast whose activation arguments carry the
// thread to open. Falls back to beeep (no click) if the toast fails.
func (a *App) deliverNotification(n notification) {
	t := toast.Notification{
		AppID:               "Meron",
		Title:               n.title,
		Body:                n.body,
		Icon:                notifyIcon(),
		ActivationType:      toast.Foreground, // required for the activation callback to fire
		ActivationArguments: n.account + notifyArgSeparator + n.threadID,
	}
	if err := t.Push(); err != nil {
		a.logf("toast notify failed: %v, falling back to beeep", err)
		if err := beeep.Notify(n.title, n.body, notifyIcon()); err != nil {
			a.logf("notify new mail failed: %v", err)
		}
	}
}

func parseNotifyArgs(args string) (account, threadID string) {
	account, threadID, _ = strings.Cut(args, notifyArgSeparator)
	return account, threadID
}
