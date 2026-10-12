package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestTrayStyleNativeStorageSurvivesFrontendFailure(t *testing.T) {
	path := filepath.Join(t.TempDir(), "tray.json")
	if loadTrayStyle(path) != trayMonochromeDefault {
		t.Fatal("missing preference did not use the native default")
	}
	for _, monochrome := range []bool{false, true, false} {
		if err := saveTrayStyle(path, monochrome); err != nil {
			t.Fatal(err)
		}
		if loadTrayStyle(path) != monochrome {
			t.Fatal("saved style was not restored before frontend startup")
		}
	}
	if err := os.WriteFile(path, []byte(`{"monochrome":"false"}`), 0o600); err != nil {
		t.Fatal(err)
	}
	if loadTrayStyle(path) != trayMonochromeDefault {
		t.Fatal("invalid preference did not use the native default")
	}
}

func TestTrayStyleIsSavedBeforeAcknowledgementAndNotByUnreadRenders(t *testing.T) {
	path := filepath.Join(t.TempDir(), "tray.json")
	app := &App{trayStylePath: path}
	t.Cleanup(func() { app.stopTray(); app.waitForTrayIcon() })
	stubTrayRenderer(t, func(bool, bool) error { return nil })
	app.setTrayUnread(true)
	app.waitForTrayIcon()
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Fatal("an unread render persisted an unchosen default")
	}
	if _, err := app.traySetStyle(map[string]any{"monochrome": true}); err != nil {
		t.Fatal(err)
	}
	if !loadTrayStyle(path) {
		t.Fatal("native style was acknowledged before being saved")
	}
	app.waitForTrayIcon()
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	app.setTrayUnread(false)
	app.waitForTrayIcon()
	after, err := os.Stat(path)
	if err != nil || !info.ModTime().Equal(after.ModTime()) {
		t.Fatal("an unread render rewrote the style")
	}
}

func TestTrayStylePersistenceFailureStillChangesLiveIcon(t *testing.T) {
	blocker := filepath.Join(t.TempDir(), "file")
	if err := os.WriteFile(blocker, []byte("not a directory"), 0o600); err != nil {
		t.Fatal(err)
	}
	app := &App{trayStylePath: filepath.Join(blocker, "tray.json")}
	t.Cleanup(func() { app.stopTray(); app.waitForTrayIcon() })
	var rendered bool
	stubTrayRenderer(t, func(_ bool, mono bool) error { rendered = mono; return nil })
	result, err := app.traySetStyle(map[string]any{"monochrome": true})
	if err != nil {
		t.Fatal(err)
	}
	app.waitForTrayIcon()
	if !rendered || !app.trayState().state().monochrome {
		t.Fatal("save failure prevented the live style change")
	}
	if result.(map[string]any)["persisted"] != false {
		t.Fatal("save failure was reported as persisted")
	}
}

func TestSystemCheckSuppliesLiveStyleWhenTrayGetStyleFails(t *testing.T) {
	for _, monochrome := range []bool{true, false} {
		app := &App{tray: trayRenderer{desired: trayIconState{monochrome: monochrome}}}
		result, err := app.systemCheck()
		if err != nil {
			t.Fatal(err)
		}
		if result.(map[string]any)["tray_monochrome"] != monochrome {
			t.Fatal("system check did not expose the native live preference for boot fallback")
		}
	}
}
