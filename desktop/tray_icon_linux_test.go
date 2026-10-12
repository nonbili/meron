//go:build linux

package main

import (
	"bytes"
	"encoding/xml"
	"image/png"
	"os"
	"os/exec"
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

// KDE needs both a stylesheet and currentColor; the class alone leaves a
// hardcoded fill unchanged. Keep the unread badge outside that colour scheme.
func TestSymbolicTrayIconsAreRecolourableSVGs(t *testing.T) {
	for name, svg := range map[string][]byte{"tray-symbolic.svg": traySymbolicSVG, "tray-unread-symbolic.svg": trayUnreadSymbolicSVG} {
		var document struct {
			Defs struct {
				Style struct {
					ID   string `xml:"id,attr"`
					Text string `xml:",chardata"`
				} `xml:"style"`
			} `xml:"defs"`
			Rect struct {
				Class string `xml:"class,attr"`
				Fill  string `xml:"fill,attr"`
			} `xml:"rect"`
			Circles []struct {
				Class string `xml:"class,attr"`
				Fill  string `xml:"fill,attr"`
			} `xml:"circle"`
		}
		if err := xml.Unmarshal(svg, &document); err != nil {
			t.Fatalf("%s: %v", name, err)
		}
		if document.Defs.Style.ID != "current-color-scheme" ||
			!bytes.Contains([]byte(document.Defs.Style.Text), []byte(".ColorScheme-Text")) ||
			document.Rect.Class != "ColorScheme-Text" || document.Rect.Fill != "currentColor" {
			t.Errorf("%s does not use the panel colour", name)
		}
		if name == "tray-unread-symbolic.svg" {
			// GTK uses the error class for its red symbolic palette, while
			// KDE leaves the fixed fill outside ColorScheme-Text alone.
			if len(document.Circles) != 1 || document.Circles[0].Class != "error" || document.Circles[0].Fill != "#ff3b30" {
				t.Errorf("%s must have a fixed red unread badge", name)
			}
		} else if len(document.Circles) != 0 {
			t.Errorf("%s must not have an unread badge", name)
		}
	}
}

func TestGTKSymbolicUnreadBadgeUsesErrorColour(t *testing.T) {
	encoder, err := exec.LookPath("gtk-encode-symbolic-svg")
	if err != nil {
		t.Skip("GTK symbolic encoder is not installed")
	}
	dir := t.TempDir()
	for _, name := range []string{"tray-symbolic", "tray-unread-symbolic"} {
		output, err := exec.Command(encoder, "-o", dir, filepath.Join("build", name+".svg"), "128x128").CombinedOutput()
		if err != nil {
			t.Fatalf("%s: %v: %s", name, err, output)
		}
		file, err := os.Open(filepath.Join(dir, name+".symbolic.png"))
		if err != nil {
			t.Fatal(err)
		}
		img, err := png.Decode(file)
		file.Close()
		if err != nil {
			t.Fatal(err)
		}
		// GTK's encoded symbolic PNG stores error coverage in blue, not
		// the final display colour. The foreground uses none of RGB.
		r, g, b, a := img.At(112, 112).RGBA()
		if a == 0 || r != 0 || g != 0 {
			t.Fatalf("%s: unexpected badge pixel (%d, %d, %d, %d)", name, r, g, b, a)
		}
		if name == "tray-unread-symbolic" && b != a {
			t.Fatal("unread badge was encoded as foreground instead of error colour")
		}
		if name == "tray-symbolic" && b != 0 {
			t.Fatal("read icon contains an error-coloured badge")
		}
	}
}
