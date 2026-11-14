//go:build linux

package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"meron/internal/appicon"
)

func TestLinuxAppIconRewritesDesktopEntry(t *testing.T) {
	dataHome := t.TempDir()
	t.Setenv("XDG_DATA_HOME", dataHome)
	entry := filepath.Join(dataHome, "applications", "meron.desktop")
	iconLine := func() string {
		t.Helper()
		data, err := os.ReadFile(entry)
		if err != nil {
			t.Fatal(err)
		}
		for _, line := range strings.Split(string(data), "\n") {
			if strings.HasPrefix(line, "Icon=") {
				return strings.TrimPrefix(line, "Icon=")
			}
		}
		t.Fatal("desktop entry has no Icon line")
		return ""
	}

	blue := []byte("blue icon")
	platformSetAppIcon(appicon.VariantBlue, blue)
	want := filepath.Join(dataHome, "icons", "meron-blue.png")
	if got := iconLine(); got != want {
		t.Fatalf("Icon = %q, want %q", got, want)
	}
	if data, err := os.ReadFile(want); err != nil || string(data) != string(blue) {
		t.Fatalf("alternative icon not written: %q, %v", data, err)
	}

	platformSetAppIcon(appicon.VariantColour, nil)
	if got := iconLine(); got != "meron" {
		t.Fatalf("Icon = %q, want the shipped icon name", got)
	}
}

func TestLinuxAppIconNotOfferedInSandboxes(t *testing.T) {
	for _, key := range []string{"FLATPAK_ID", "SNAP_NAME"} {
		t.Run(key, func(t *testing.T) {
			t.Setenv(key, "meron")
			if appIconStyleSupported() {
				t.Fatalf("%s builds do not write their own desktop entry", key)
			}
		})
	}
}
