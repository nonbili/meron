//go:build linux

package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
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
	platformSetAppIcon("#1d6fe0", blue)
	want := filepath.Join(dataHome, "icons", "meron-icon-1d6fe0.png")
	if got := iconLine(); got != want {
		t.Fatalf("Icon = %q, want %q", got, want)
	}
	if data, err := os.ReadFile(want); err != nil || string(data) != string(blue) {
		t.Fatalf("alternative icon not written: %q, %v", data, err)
	}

	// Another colour replaces the file rather than piling up next to it.
	platformSetAppIcon("#d0342c", []byte("red icon"))
	if _, err := os.Stat(want); !os.IsNotExist(err) {
		t.Fatalf("the previous colour's icon should be gone: %v", err)
	}

	platformSetAppIcon("", nil)
	if got := iconLine(); got != "meron" {
		t.Fatalf("Icon = %q, want the shipped icon name", got)
	}
	if left, _ := filepath.Glob(filepath.Join(dataHome, "icons", "meron-icon-*.png")); len(left) != 0 {
		t.Fatalf("recoloured icons left behind: %v", left)
	}
	// The shipped icon itself is never touched.
	if _, err := os.Stat(filepath.Join(dataHome, "icons", "meron.png")); err != nil {
		t.Fatal(err)
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
