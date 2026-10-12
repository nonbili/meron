//go:build linux

package main

import (
	"bytes"
	_ "embed"
	"os"
	"path/filepath"

	"fyne.io/systray"
)

// On Linux the tray icon is passed by name from a folder the tray item names
// as its icon theme path. The monochrome one is a symbolic icon, which the
// shell (GNOME's AppIndicator extension, KDE Plasma) draws in the panel's own
// text colour. The colour one is named too: a picture, once set, can't be
// taken off the tray item again, and GNOME keeps showing it over the name.

//go:embed build/tray-symbolic.svg
var traySymbolicSVG []byte

//go:embed build/tray-unread-symbolic.svg
var trayUnreadSymbolicSVG []byte

const (
	trayIconName             = "jp.nonbili.meron-tray-symbolic"
	trayUnreadIconName       = "jp.nonbili.meron-tray-unread-symbolic"
	trayColourIconName       = "jp.nonbili.meron-tray"
	trayColourUnreadIconName = "jp.nonbili.meron-tray-unread"
)

var (
	traySetIconName      = systray.SetIconName
	traySetIconThemePath = systray.SetIconThemePath
	trayIconDir          = func() string { return filepath.Join(appCacheDir(), "tray-icons") }
)

func applyPlatformTrayIcon(unread, monochrome bool) error {
	dir, err := writeTrayIcons()
	if err != nil {
		// Without the files there is nothing to name: show the colour picture.
		traySetIconName("")
		traySetIcon(trayIcon(unread))
		return err
	}
	traySetIconThemePath(dir)
	traySetIconName(trayIconNameFor(unread, monochrome))
	return nil
}

func trayIconNameFor(unread, monochrome bool) string {
	switch {
	case monochrome && unread:
		return trayUnreadIconName
	case monochrome:
		return trayIconName
	case unread:
		return trayColourUnreadIconName
	default:
		return trayColourIconName
	}
}

// writeTrayIcons puts the icons where the shell looks for them, leaving files
// that are already current alone.
func writeTrayIcons() (string, error) {
	dir := trayIconDir()
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return "", err
	}
	files := map[string][]byte{
		trayIconName + ".svg":             traySymbolicSVG,
		trayUnreadIconName + ".svg":       trayUnreadSymbolicSVG,
		trayColourIconName + ".png":       trayIconPNG,
		trayColourUnreadIconName + ".png": trayIconUnreadPNG,
	}
	for file, data := range files {
		path := filepath.Join(dir, file)
		if current, err := os.ReadFile(path); err == nil && bytes.Equal(current, data) {
			continue
		}
		if err := os.WriteFile(path, data, 0o644); err != nil {
			return "", err
		}
	}
	return dir, nil
}
