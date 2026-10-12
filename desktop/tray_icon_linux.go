//go:build linux

package main

import (
	"bytes"
	"crypto/sha256"
	_ "embed"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"sync"

	"fyne.io/systray"
)

// Named symbolic icons follow the host's panel colour. Publish a matching
// pixmap too, for hosts that cannot resolve the name or ignore IconThemePath.

//go:embed build/tray-symbolic.svg
var traySymbolicSVG []byte

//go:embed build/tray-unread-symbolic.svg
var trayUnreadSymbolicSVG []byte

// A neutral grey fallback remains visible on light and dark panels; hosts
// without symbolic icon support cannot recolour it to the panel's text colour.
//
//go:embed build/tray-fallback-mono.png
var trayFallbackMonoPNG []byte

//go:embed build/tray-fallback-mono-unread.png
var trayFallbackMonoUnreadPNG []byte

const (
	trayIconName             = "jp.nonbili.meron-tray-symbolic"
	trayUnreadIconName       = "jp.nonbili.meron-tray-unread-symbolic"
	trayColourIconName       = "jp.nonbili.meron-tray"
	trayColourUnreadIconName = "jp.nonbili.meron-tray-unread"
)

type trayIconAsset struct {
	name string
	data []byte
}

var trayIconAssets = []trayIconAsset{
	{trayIconName + ".svg", traySymbolicSVG},
	{trayUnreadIconName + ".svg", trayUnreadSymbolicSVG},
	{trayColourIconName + ".png", trayIconPNG},
	{trayColourUnreadIconName + ".png", trayIconUnreadPNG},
}

var (
	traySetIconWithName = systray.SetIconWithName
	trayIconDir         = func() string { return filepath.Join(appCacheDir(), "tray-icons") }
	trayIconFiles       struct {
		sync.Mutex
		dir string
	}
)

func applyPlatformTrayIcon(unread, monochrome bool) error {
	icon := trayIcon(unread)
	if monochrome {
		icon = trayFallbackMonoPNG
		if unread {
			icon = trayFallbackMonoUnreadPNG
		}
	}
	dir, err := writeTrayIcons()
	if err != nil {
		// Replace all three properties, including stale names and pixmaps.
		traySetIconWithName(icon, "", "")
		return err
	}
	traySetIconWithName(icon, trayIconNameFor(unread, monochrome), dir)
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

// A content-versioned directory prevents hosts from caching an older icon
// under the same path after an upgrade. Successful installation is cached;
// failures are retried on the next update instead of poisoning a sync.Once.
func writeTrayIcons() (string, error) {
	trayIconFiles.Lock()
	defer trayIconFiles.Unlock()
	root := trayIconDir()
	// Hash only once per process; the embedded assets cannot change at runtime.
	dir := filepath.Join(root, trayIconFilesVersion)
	if trayIconFiles.dir == dir && trayIconFilesExist(dir) {
		return dir, nil
	}
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return "", err
	}
	for _, file := range trayIconAssets {
		path := filepath.Join(dir, file.name)
		if current, err := os.ReadFile(path); err == nil && bytes.Equal(current, file.data) {
			continue
		}
		if err := writeFileAtomic(path, file.data, 0o644, 0o755); err != nil {
			return "", err
		}
	}
	trayIconFiles.dir = dir
	pruneTrayIconDirectories(root, dir)
	return dir, nil
}

func trayIconFilesExist(dir string) bool {
	for _, file := range trayIconAssets {
		if info, err := os.Stat(filepath.Join(dir, file.name)); err != nil || !info.Mode().IsRegular() {
			return false
		}
	}
	return true
}

// Check metadata, not file contents, so an unchanged unread update can repair
// a cleared cache without rereading the embedded colour PNGs on every sync.
func platformTrayIconNeedsRepair() bool {
	// Do not wait on the installer mutex from a bridge call: installation
	// may be doing disk I/O. The expected versioned path is deterministic.
	return !trayIconFilesExist(filepath.Join(trayIconDir(), trayIconFilesVersion))
}

// Keep this release and one previous version for an older running instance.
// Only touch our 24-digit content-hash directories; unrelated entries survive.
func pruneTrayIconDirectories(root, current string) {
	// Retire the previous flat installation after the versioned files exist.
	for _, file := range trayIconAssets {
		_ = os.Remove(filepath.Join(root, file.name))
	}
	entries, err := os.ReadDir(root)
	if err != nil {
		return
	}
	var older []os.FileInfo
	for _, entry := range entries {
		if !entry.IsDir() || filepath.Join(root, entry.Name()) == current || len(entry.Name()) != 24 {
			continue
		}
		if _, err := hex.DecodeString(entry.Name()); err != nil {
			continue
		}
		if info, err := entry.Info(); err == nil {
			older = append(older, info)
		}
	}
	sort.Slice(older, func(i, j int) bool { return older[i].ModTime().After(older[j].ModTime()) })
	if len(older) > 1 {
		for _, info := range older[1:] {
			_ = os.RemoveAll(filepath.Join(root, info.Name()))
		}
	}
}

var trayIconFilesVersion = func() string {
	hash := sha256.New()
	for _, file := range trayIconAssets {
		hash.Write(file.data)
	}
	return fmt.Sprintf("%x", hash.Sum(nil)[:12])
}()
