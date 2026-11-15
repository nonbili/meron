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

	wailsRuntime "meron/internal/desktopruntime"
)

// installDesktopEntry writes a .desktop file and icon into the user's
// XDG data dir so GNOME/KDE/etc. show the app's logo in the dock and
// switcher, and registers Meron as the user's mailto: handler. Safe to
// call on every startup: it only rewrites files when their content has changed.
//
// alternative is the app icon the user chose in place of the shipped one, as
// PNG data filed under name; nil keeps the shipped icon.
func installDesktopEntry(name string, alternative []byte) {
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

	// An alternative is referenced by path under a name of its own. Shells
	// cache icons by name, so swapping the bytes behind "meron" would keep
	// showing the old one until the next login.
	icon := "meron"
	iconsDir := filepath.Join(dataHome, "icons")
	if len(alternative) > 0 {
		icon = filepath.Join(iconsDir, alternativeIconPrefix+strings.TrimPrefix(name, "#")+".png")
		writeIfChanged(icon, alternative)
	}
	// Each colour tried leaves a file behind; only the one in use is kept.
	if stale, err := filepath.Glob(filepath.Join(iconsDir, alternativeIconPrefix+"*.png")); err == nil {
		for _, path := range stale {
			if path != icon {
				_ = os.Remove(path)
			}
		}
	}

	desktop := desktopEntry(desktopLaunchTarget(exe, os.Getenv), icon)
	writeIfChanged(desktopPath, []byte(desktop))

	// Refresh the MIME associations database so the mailto: handler takes
	// effect without a re-login. No-op on hosts without the utility.
	if bin, err := exec.LookPath("update-desktop-database"); err == nil {
		appsDir := filepath.Join(dataHome, "applications")
		go func() { _ = exec.Command(bin, appsDir).Run() }()
	}
}

// alternativeIconPrefix starts the file name of a recoloured launcher icon.
const alternativeIconPrefix = "meron-icon-"

// desktopEntry renders the .desktop file launching target with icon, an icon
// theme name or an absolute path.
func desktopEntry(target, icon string) string {
	return fmt.Sprintf(`[Desktop Entry]
Type=Application
Name=Meron
Comment=Messages that spark joy
Exec=%s %%U
Icon=%s
Terminal=false
Categories=Office;Network;Email;
StartupWMClass=%s
MimeType=x-scheme-handler/mailto;message/rfc822;
`, desktopExecArg(target), icon, desktopProgramName)
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

// desktopProgramName is the name the windows announce themselves under (the
// Wayland app_id and the X11 WM_CLASS), and so what StartupWMClass has to say
// for the shell to tie a window to the launcher. GTK would otherwise take it
// from argv[0], which inside an AppImage is the image's file name.
const desktopProgramName = "meron"

// linuxProgramName is desktopProgramName for release builds. Dev launches keep
// the name GTK derives, so they stay apart from an installed copy in the dock.
func linuxProgramName() string {
	if isWailsDevLaunch() {
		return ""
	}
	return desktopProgramName
}

// desktopLaunchTarget returns the path the launcher should execute. That is
// the executable, except inside an AppImage: there exe sits in the image's
// temporary mount, which is gone once the app quits, and the image itself is
// what has to be started.
func desktopLaunchTarget(exe string, getenv func(string) string) string {
	if image := appImagePath(exe, getenv); image != "" {
		return image
	}
	return exe
}

// appImagePath returns the path of the AppImage exe runs from, or "" when it
// is not running from one. The runtime exports the image's path as APPIMAGE
// and its mount as APPDIR.
//
// Both are inherited by child processes, so a copy merely started from
// another AppImage sees them too; they only count when exe is inside APPDIR.
func appImagePath(exe string, getenv func(string) string) string {
	image, appDir := getenv("APPIMAGE"), getenv("APPDIR")
	if image == "" || appDir == "" || !filepath.IsAbs(image) {
		return ""
	}
	if resolved, err := filepath.EvalSymlinks(appDir); err == nil {
		appDir = resolved
	}
	rel, err := filepath.Rel(appDir, exe)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return ""
	}
	return image
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
	a.showMainWindow()
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
