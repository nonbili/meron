package main

import (
	"crypto/sha256"
	"fmt"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

// installDesktopEntry writes a .desktop file and icon into the user's
// XDG data dir so GNOME/KDE/etc. show the app's logo in the dock and
// switcher, and registers Meron as the user's mailto: handler. Safe to
// call on every startup: it only rewrites files when their content has changed.
func installDesktopEntry() {
	if runtime.GOOS != "linux" {
		return
	}

	exe, err := os.Executable()
	if err != nil {
		return
	}
	exe, err = filepath.EvalSymlinks(exe)
	if err != nil {
		return
	}

	dataHome := os.Getenv("XDG_DATA_HOME")
	if dataHome == "" {
		home, err := os.UserHomeDir()
		if err != nil {
			return
		}
		dataHome = filepath.Join(home, ".local", "share")
	}

	iconPath := filepath.Join(dataHome, "icons", "meron.png")
	desktopPath := filepath.Join(dataHome, "applications", "meron.desktop")

	writeIfChanged(iconPath, appIconPNG)

	desktop := fmt.Sprintf(`[Desktop Entry]
Type=Application
Name=Meron
Comment=Messages that spark joy
Exec=%s %%u
Icon=meron
Terminal=false
Categories=Office;Network;Email;
StartupWMClass=meron
MimeType=x-scheme-handler/mailto;
`, desktopExecArg(exe))
	writeIfChanged(desktopPath, []byte(desktop))

	// Refresh the MIME associations database so the mailto: handler takes
	// effect without a re-login. No-op on hosts without the utility.
	if bin, err := exec.LookPath("update-desktop-database"); err == nil {
		appsDir := filepath.Join(dataHome, "applications")
		go func() { _ = exec.Command(bin, appsDir).Run() }()
	}
}

func writeIfChanged(path string, data []byte) {
	if existing, err := os.ReadFile(path); err == nil {
		if sha256.Sum256(existing) == sha256.Sum256(data) {
			return
		}
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return
	}
	_ = os.WriteFile(path, data, 0o644)
}

func desktopExecArg(path string) string {
	if strings.ContainsAny(path, " \t\n\"'\\") {
		return strconv.Quote(path)
	}
	return path
}

func (a *App) queueStartupMailtoURLs(args []string) {
	urls := mailtoURLs(args)
	if len(urls) == 0 {
		return
	}
	a.mailtoMu.Lock()
	a.pendingMailto = append(a.pendingMailto, urls...)
	a.mailtoMu.Unlock()
}

func (a *App) consumePendingMailto() []string {
	a.mailtoMu.Lock()
	defer a.mailtoMu.Unlock()
	urls := append([]string(nil), a.pendingMailto...)
	a.pendingMailto = nil
	if urls == nil {
		return []string{}
	}
	return urls
}

func (a *App) openMailtoURL(raw string) {
	if !isMailtoURL(raw) {
		return
	}
	if a.ctx == nil {
		a.queueStartupMailtoURLs([]string{raw})
		return
	}
	wailsRuntime.WindowShow(a.ctx)
	wailsRuntime.WindowUnminimise(a.ctx)
	wailsRuntime.EventsEmit(a.ctx, "mailto.open", raw)
}

func mailtoURLs(args []string) []string {
	urls := make([]string, 0, len(args))
	for _, arg := range args {
		if isMailtoURL(arg) {
			urls = append(urls, arg)
		}
	}
	return urls
}

func isMailtoURL(raw string) bool {
	u, err := url.Parse(strings.TrimSpace(raw))
	return err == nil && strings.EqualFold(u.Scheme, "mailto")
}
