package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestAppIconChoiceRoundTrip(t *testing.T) {
	path := filepath.Join(t.TempDir(), "appicon.json")
	if got := loadAppIconChoice(path); got.Custom || got.Colour != defaultAppIconColour {
		t.Fatalf("missing file: got %+v, want the shipped icon", got)
	}
	for _, choice := range []appIconChoice{{true, "#d0342c"}, {false, "#d0342c"}, {true, "#000000"}} {
		if err := saveAppIconChoice(path, choice); err != nil {
			t.Fatal(err)
		}
		if got := loadAppIconChoice(path); got != choice {
			t.Fatalf("got %+v, want %+v", got, choice)
		}
	}
}

func TestAppIconChoiceIgnoresDamagedFile(t *testing.T) {
	path := filepath.Join(t.TempDir(), "appicon.json")
	for _, content := range []string{"{", `{"custom":true,"colour":"neon"}`, `{"custom":true}`, `{"custom":"yes","colour":"#fff"}`} {
		if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
			t.Fatal(err)
		}
		if got := loadAppIconChoice(path); got.Custom {
			t.Fatalf("%s: got %+v, want the shipped icon", content, got)
		}
	}
}

func TestAppIconSetValidatesAndPersists(t *testing.T) {
	// On Linux applying a colour rewrites the desktop entry; keep it off the
	// real one.
	t.Setenv("XDG_DATA_HOME", t.TempDir())
	app := &App{}
	app.appIcon.path = filepath.Join(t.TempDir(), "appicon.json")

	if _, err := app.appIconSet(map[string]any{"custom": true, "colour": "neon"}); err == nil {
		t.Fatal("a value that is not a colour must be rejected")
	}
	if _, err := app.appIconSet(map[string]any{"custom": true}); err == nil {
		t.Fatal("a missing colour must be rejected")
	}
	result, err := app.appIconSet(map[string]any{"custom": true, "colour": "#D0342C"})
	if err != nil {
		t.Fatal(err)
	}
	if persisted, _ := result.(map[string]any)["persisted"].(bool); !persisted {
		t.Fatal("the choice should have been saved")
	}
	want := appIconChoice{Custom: true, Colour: "#d0342c"}
	if got := loadAppIconChoice(app.appIcon.path); got != want {
		t.Fatalf("saved %+v, want %+v", got, want)
	}
	state := app.appIconGet()
	if state["custom"] != true || state["colour"] != "#d0342c" {
		t.Fatalf("appIconGet = %v", state)
	}
	// Back to the shipped icon keeps the colour for next time.
	if _, err := app.appIconSet(map[string]any{"custom": false, "colour": "#d0342c"}); err != nil {
		t.Fatal(err)
	}
	if got := loadAppIconChoice(app.appIcon.path); got.Custom || got.Colour != "#d0342c" {
		t.Fatalf("saved %+v, want the shipped icon with the colour kept", got)
	}
}
