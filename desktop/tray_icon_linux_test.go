//go:build linux

package main

import (
	"bytes"
	"encoding/xml"
	"image/png"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"
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
	setIcon, dir := traySetIconWithName, trayIconDir
	traySetIconWithName = func(icon []byte, name, path string) {
		calls.icon, calls.iconSet = icon, true
		calls.name, calls.nameSet, calls.themePath = name, true, path
	}
	iconDir := t.TempDir()
	trayIconDir = func() string { return iconDir }
	t.Cleanup(func() {
		traySetIconWithName, trayIconDir = setIcon, dir
	})
	return calls
}

func TestMonochromeTrayIconIsASymbolicIconTheShellCanRecolour(t *testing.T) {
	calls := stubTrayIconCalls(t)

	if err := applyPlatformTrayIcon(false, true); err != nil {
		t.Fatal(err)
	}
	if calls.themePath != filepath.Join(trayIconDir(), trayIconFilesVersion) || calls.name != trayIconName {
		t.Fatalf("theme path %q, name %q", calls.themePath, calls.name)
	}
	files := map[string][]byte{
		trayIconName + ".svg":             traySymbolicSVG,
		trayUnreadIconName + ".svg":       trayUnreadSymbolicSVG,
		trayColourIconName + ".png":       trayIconPNG,
		trayColourUnreadIconName + ".png": trayIconUnreadPNG,
	}
	for file, want := range files {
		got, err := os.ReadFile(filepath.Join(calls.themePath, file))
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

// Both forms update together, so hosts choosing the pixmap cannot keep a
// stale unread state or style.
func TestColourTrayIconIsNamedAndUpdatesItsPixmap(t *testing.T) {
	calls := stubTrayIconCalls(t)

	if err := applyPlatformTrayIcon(true, false); err != nil {
		t.Fatal(err)
	}
	if calls.name != trayColourUnreadIconName || calls.themePath != filepath.Join(trayIconDir(), trayIconFilesVersion) {
		t.Fatalf("theme path %q, name %q", calls.themePath, calls.name)
	}
	if err := applyPlatformTrayIcon(false, true); err != nil {
		t.Fatal(err)
	}
	if !calls.iconSet || !bytes.Equal(calls.icon, trayFallbackMonoPNG) {
		t.Fatal("the fallback pixmap did not switch to monochrome")
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
	if calls.name != "" || !bytes.Equal(calls.icon, trayFallbackMonoPNG) {
		t.Fatalf("no monochrome fallback: name %q", calls.name)
	}
}

func TestTraySetStyleSwitchesTheIcon(t *testing.T) {
	calls := stubTrayIconCalls(t)
	app := &App{}
	t.Cleanup(func() { app.stopTray(); app.waitForTrayIcon() })
	app.tray.desired.monochrome = trayMonochromeDefault
	if !app.trayState().state().monochrome {
		t.Fatal("monochrome should be the default on Linux")
	}

	if _, err := app.traySetStyle(map[string]any{"monochrome": false}); err != nil {
		t.Fatal(err)
	}
	app.waitForTrayIcon()
	if app.trayState().state().monochrome || calls.name != trayColourIconName {
		t.Fatalf("switching to colour did not apply the colour icon: %q", calls.name)
	}

	if _, err := app.traySetStyle(map[string]any{"monochrome": true}); err != nil {
		t.Fatal(err)
	}
	app.waitForTrayIcon()
	if calls.name != trayIconName {
		t.Fatalf("switching back did not apply the symbolic icon: %q", calls.name)
	}

	if _, err := app.traySetStyle(map[string]any{}); err == nil {
		t.Fatal("expected an error without a monochrome flag")
	}
}

func TestTrayIconRecoversFromWriteFailureWithCurrentState(t *testing.T) {
	calls := stubTrayIconCalls(t)
	root := filepath.Join(t.TempDir(), "blocked")
	if err := os.WriteFile(root, nil, 0o600); err != nil {
		t.Fatal(err)
	}
	trayIconDir = func() string { return root }
	if err := applyPlatformTrayIcon(false, false); err == nil {
		t.Fatal("expected an installation error")
	}
	if calls.name != "" || !bytes.Equal(calls.icon, trayIconPNG) {
		t.Fatal("failed installation did not publish the current fallback")
	}
	if err := os.Remove(root); err != nil {
		t.Fatal(err)
	}
	if err := applyPlatformTrayIcon(true, true); err != nil {
		t.Fatal(err)
	}
	if calls.name != trayUnreadIconName || !bytes.Equal(calls.icon, trayFallbackMonoUnreadPNG) {
		t.Fatal("recovery kept the earlier pixmap instead of the current unread/style state")
	}
}

func TestTrayIconRepairsClearedCacheWhileMailboxIsIdle(t *testing.T) {
	calls := stubTrayIconCalls(t)
	app := &App{tray: trayRenderer{desired: trayIconState{monochrome: true}}}
	clock := configureTrayClock(&app.tray)
	t.Cleanup(func() { app.stopTray(); app.waitForTrayIcon() })
	app.setTrayUnread(true)
	app.waitForTrayIcon()
	if err := os.RemoveAll(calls.themePath); err != nil {
		t.Fatal(err)
	}
	if !platformTrayIconNeedsRepair() {
		t.Fatal("deleted cache was considered valid")
	}
	clock.advance(trayHealthInterval)
	app.waitForTrayIcon()
	if !trayIconFilesExist(calls.themePath) {
		t.Fatal("health timer did not repair the deleted icon files")
	}
}

func TestTrayCacheKeepsOnlyCurrentAndPreviousVersions(t *testing.T) {
	root := t.TempDir()
	current := filepath.Join(root, strings.Repeat("a", 24))
	for _, file := range trayIconAssets {
		if err := os.WriteFile(filepath.Join(root, file.name), []byte("old flat icon"), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	for i, name := range []string{strings.Repeat("a", 24), strings.Repeat("b", 24), strings.Repeat("c", 24), strings.Repeat("d", 24), "unrelated"} {
		path := filepath.Join(root, name)
		if err := os.Mkdir(path, 0o755); err != nil {
			t.Fatal(err)
		}
		when := time.Unix(int64(i), 0)
		if err := os.Chtimes(path, when, when); err != nil {
			t.Fatal(err)
		}
	}
	pruneTrayIconDirectories(root, current)
	for _, file := range trayIconAssets {
		if _, err := os.Stat(filepath.Join(root, file.name)); !os.IsNotExist(err) {
			t.Fatal("legacy flat icon survived pruning")
		}
	}
	entries, err := os.ReadDir(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 3 {
		t.Fatalf("kept %d directories, want current, previous and unrelated", len(entries))
	}
	for _, name := range []string{strings.Repeat("a", 24), strings.Repeat("d", 24), "unrelated"} {
		if _, err := os.Stat(filepath.Join(root, name)); err != nil {
			t.Fatal(err)
		}
	}
}

func TestColourTrayPixmapsAreSizedForPanels(t *testing.T) {
	for _, data := range [][]byte{trayIconPNG, trayIconUnreadPNG} {
		config, err := png.DecodeConfig(bytes.NewReader(data))
		if err != nil {
			t.Fatal(err)
		}
		if config.Width > 128 || config.Height > 128 {
			t.Fatalf("oversized pixmap: %dx%d", config.Width, config.Height)
		}
	}
}

func TestTrayIconAtomicWriteKeepsExistingFileOnFailure(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "icon.svg")
	if err := os.WriteFile(path, []byte("original"), 0o644); err != nil {
		t.Fatal(err)
	}
	// Renaming over a directory fails; the previous entry must remain intact,
	// and the temporary file must be removed.
	blocked := filepath.Join(dir, "blocked")
	if err := os.Mkdir(blocked, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := writeFileAtomic(blocked, []byte("new"), 0o644, 0o755); err == nil {
		t.Fatal("expected a rename failure")
	}
	entries, err := os.ReadDir(dir)
	if err != nil || len(entries) != 2 {
		t.Fatalf("temporary files remain: %v, %v", entries, err)
	}
	if err := writeFileAtomic(path, []byte("replacement"), 0o644, 0o755); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(path)
	if err != nil || string(data) != "replacement" {
		t.Fatalf("icon was not replaced: %q, %v", data, err)
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
			Path struct {
				Class string `xml:"class,attr"`
				Fill  string `xml:"fill,attr"`
			} `xml:"path"`
			Circles []struct {
				Class string `xml:"class,attr"`
				Fill  string `xml:"fill,attr"`
			} `xml:"circle"`
		}
		if bytes.Contains(svg, []byte("<mask")) || bytes.Contains(svg, []byte("<image")) {
			t.Fatalf("%s must use vector paths without masks or raster images", name)
		}
		if err := xml.Unmarshal(svg, &document); err != nil {
			t.Fatalf("%s: %v", name, err)
		}
		if document.Defs.Style.ID != "current-color-scheme" ||
			!bytes.Contains([]byte(document.Defs.Style.Text), []byte(".ColorScheme-Text")) ||
			document.Path.Class != "ColorScheme-Text" || document.Path.Fill != "currentColor" {
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
