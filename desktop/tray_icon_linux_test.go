//go:build linux

package main

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

type trayIconCalls struct {
	icon      []byte
	iconSet   bool
	name      string
	nameSet   bool
	themePath string
}

func stubTrayIconCalls(t *testing.T) *trayIconCalls {
	t.Helper()
	calls := &trayIconCalls{}
	setIcon, setName, setThemePath, dir := traySetIcon, traySetIconName, traySetIconThemePath, trayIconDir
	traySetIcon = func(icon []byte) { calls.icon, calls.iconSet = icon, true }
	traySetIconName = func(name string) { calls.name, calls.nameSet = name, true }
	traySetIconThemePath = func(path string) { calls.themePath = path }
	iconDir := t.TempDir()
	trayIconDir = func() string { return iconDir }
	t.Cleanup(func() {
		traySetIcon, traySetIconName, traySetIconThemePath, trayIconDir = setIcon, setName, setThemePath, dir
	})
	return calls
}

func TestMonochromeTrayIconIsASymbolicIconTheShellCanRecolour(t *testing.T) {
	calls := stubTrayIconCalls(t)

	if err := applyPlatformTrayIcon(false, true); err != nil {
		t.Fatal(err)
	}
	if calls.themePath != trayIconDir() || calls.name != trayIconName {
		t.Fatalf("theme path %q, name %q", calls.themePath, calls.name)
	}
	files := map[string][]byte{
		trayIconName + ".svg":             traySymbolicSVG,
		trayUnreadIconName + ".svg":       trayUnreadSymbolicSVG,
		trayColourIconName + ".png":       trayIconPNG,
		trayColourUnreadIconName + ".png": trayIconUnreadPNG,
	}
	for file, want := range files {
		got, err := os.ReadFile(filepath.Join(trayIconDir(), file))
		if err != nil || !bytes.Equal(got, want) {
			t.Fatalf("%s not written as embedded: %v", file, err)
		}
	}

	if err := applyPlatformTrayIcon(true, true); err != nil {
		t.Fatal(err)
	}
	if calls.name != trayUnreadIconName {
		t.Fatalf("unread name %q", calls.name)
	}
}

// The colour icon is named too: a picture, once set, can't be taken off the
// tray item again, and GNOME keeps showing it over the symbolic name.
func TestColourTrayIconIsNamedAndSetsNoPicture(t *testing.T) {
	calls := stubTrayIconCalls(t)

	if err := applyPlatformTrayIcon(true, false); err != nil {
		t.Fatal(err)
	}
	if calls.name != trayColourUnreadIconName || calls.themePath != trayIconDir() {
		t.Fatalf("theme path %q, name %q", calls.themePath, calls.name)
	}
	if err := applyPlatformTrayIcon(false, true); err != nil {
		t.Fatal(err)
	}
	if calls.iconSet {
		t.Fatal("a picture was set")
	}
}

func TestTrayIconFallsBackToAPictureWhenTheIconsCannotBeWritten(t *testing.T) {
	calls := stubTrayIconCalls(t)
	blocked := filepath.Join(t.TempDir(), "file")
	if err := os.WriteFile(blocked, nil, 0o600); err != nil {
		t.Fatal(err)
	}
	trayIconDir = func() string { return filepath.Join(blocked, "tray-icons") }

	if err := applyPlatformTrayIcon(false, true); err == nil {
		t.Fatal("expected the write error")
	}
	if calls.name != "" || !bytes.Equal(calls.icon, trayIconPNG) {
		t.Fatalf("no colour fallback: name %q", calls.name)
	}
}

func TestTraySetStyleSwitchesTheIcon(t *testing.T) {
	calls := stubTrayIconCalls(t)
	app := &App{}
	app.trayMonochrome.Store(trayMonochromeDefault)
	if !app.trayMonochrome.Load() {
		t.Fatal("monochrome should be the default on Linux")
	}

	if _, err := app.traySetStyle(map[string]any{"monochrome": false}); err != nil {
		t.Fatal(err)
	}
	if app.trayMonochrome.Load() || calls.name != trayColourIconName {
		t.Fatalf("switching to colour did not apply the colour icon: %q", calls.name)
	}

	if _, err := app.traySetStyle(map[string]any{"monochrome": true}); err != nil {
		t.Fatal(err)
	}
	if calls.name != trayIconName {
		t.Fatalf("switching back did not apply the symbolic icon: %q", calls.name)
	}

	if _, err := app.traySetStyle(map[string]any{}); err == nil {
		t.Fatal("expected an error without a monochrome flag")
	}
}

// The shell recolours a symbolic icon through its ColorScheme-Text class; an
// empty or broken file shows as a loading placeholder in the panel.
func TestSymbolicTrayIconsAreRecolourableSVGs(t *testing.T) {
	for name, svg := range map[string][]byte{"tray-symbolic.svg": traySymbolicSVG, "tray-unread-symbolic.svg": trayUnreadSymbolicSVG} {
		if !bytes.HasPrefix(svg, []byte("<svg")) || !bytes.Contains(svg, []byte(`class="ColorScheme-Text"`)) {
			t.Errorf("%s is not a recolourable SVG (%d bytes)", name, len(svg))
		}
	}
}
