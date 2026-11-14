package main

import (
	"os"
	"path/filepath"
	"testing"

	"meron/internal/appicon"
)

func TestAppIconStyleRoundTrip(t *testing.T) {
	path := filepath.Join(t.TempDir(), "appicon.json")
	if got := loadAppIconStyle(path); got != appicon.StyleDefault {
		t.Fatalf("missing file: got %q, want default", got)
	}
	for _, style := range []appicon.Style{appicon.StyleAuto, appicon.StyleBlue, appicon.StyleDark, appicon.StyleLight, appicon.StyleDefault} {
		if err := saveAppIconStyle(path, style); err != nil {
			t.Fatal(err)
		}
		if got := loadAppIconStyle(path); got != style {
			t.Fatalf("got %q, want %q", got, style)
		}
	}
}

func TestAppIconStyleIgnoresDamagedFile(t *testing.T) {
	path := filepath.Join(t.TempDir(), "appicon.json")
	for _, content := range []string{"{", `{"style":"neon"}`, `{"style":7}`} {
		if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
			t.Fatal(err)
		}
		if got := loadAppIconStyle(path); got != appicon.StyleDefault {
			t.Fatalf("%s: got %q, want default", content, got)
		}
	}
}

func TestAppIconSetStyleValidatesAndPersists(t *testing.T) {
	// On Linux applying a style rewrites the desktop entry; keep it off the
	// real one.
	t.Setenv("XDG_DATA_HOME", t.TempDir())
	app := &App{}
	app.appIcon.path = filepath.Join(t.TempDir(), "appicon.json")

	if _, err := app.appIconSetStyle(map[string]any{"style": "neon"}); err == nil {
		t.Fatal("an unknown style must be rejected")
	}
	if _, err := app.appIconSetStyle(map[string]any{}); err == nil {
		t.Fatal("a missing style must be rejected")
	}
	result, err := app.appIconSetStyle(map[string]any{"style": "dark"})
	if err != nil {
		t.Fatal(err)
	}
	if persisted, _ := result.(map[string]any)["persisted"].(bool); !persisted {
		t.Fatal("the style should have been saved")
	}
	if got := loadAppIconStyle(app.appIcon.path); got != appicon.StyleDark {
		t.Fatalf("saved %q, want dark", got)
	}
	if got := app.appIconGetStyle()["style"]; got != "dark" {
		t.Fatalf("getStyle = %v, want dark", got)
	}
}
