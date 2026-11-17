package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestDesktopEntry(t *testing.T) {
	entry := desktopEntry("/home/u/My Apps/meron.appimage", "meron")
	for _, line := range []string{
		`Exec="/home/u/My Apps/meron.appimage" %U`,
		"StartupWMClass=" + desktopProgramName,
	} {
		if !strings.Contains(entry, "\n"+line+"\n") {
			t.Errorf("desktop entry lacks %q:\n%s", line, entry)
		}
	}
}

func TestWriteIfChangedReplacesAReadOnlyFile(t *testing.T) {
	// What an AppImage integrator leaves behind: its own entry, read-only.
	path := filepath.Join(t.TempDir(), "applications", "meron.desktop")
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte("Icon=/somewhere/else\n"), 0o444); err != nil {
		t.Fatal(err)
	}

	writeIfChanged(path, []byte("Icon=meron\n"))

	got, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "Icon=meron\n" {
		t.Fatalf("the read-only entry was left as it was: %q", got)
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode().Perm() != 0o644 {
		t.Fatalf("mode = %v, want 0644", info.Mode().Perm())
	}
	// Nothing left next to it.
	entries, err := os.ReadDir(filepath.Dir(path))
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 1 {
		t.Fatalf("temporary files left behind: %v", entries)
	}
}

func TestWriteIfChangedLeavesAnUpToDateFileAlone(t *testing.T) {
	path := filepath.Join(t.TempDir(), "meron.desktop")
	if err := os.WriteFile(path, []byte("same"), 0o444); err != nil {
		t.Fatal(err)
	}
	before, _ := os.Stat(path)
	writeIfChanged(path, []byte("same"))
	after, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if !os.SameFile(before, after) || after.Mode().Perm() != 0o444 {
		t.Fatal("an unchanged file must not be replaced")
	}
}

func TestDesktopLaunchTarget(t *testing.T) {
	mount := t.TempDir()
	if resolved, err := filepath.EvalSymlinks(mount); err == nil {
		mount = resolved
	}
	inside := filepath.Join(mount, "usr", "bin", "meron")
	if err := os.MkdirAll(filepath.Dir(inside), 0o755); err != nil {
		t.Fatal(err)
	}
	const image = "/home/u/AppImages/meron.appimage"

	tests := []struct {
		name string
		exe  string
		env  map[string]string
		want string
	}{
		{name: "plain install", exe: "/opt/meron/meron", want: "/opt/meron/meron"},
		{
			name: "appimage launches the image, not the mount",
			exe:  inside,
			env:  map[string]string{"APPIMAGE": image, "APPDIR": mount},
			want: image,
		},
		{
			name: "inherited from an unrelated appimage",
			exe:  "/opt/meron/meron",
			env:  map[string]string{"APPIMAGE": "/home/u/Other.AppImage", "APPDIR": mount},
			want: "/opt/meron/meron",
		},
		{
			name: "sibling directory sharing the mount prefix",
			exe:  mount + "-other/meron",
			env:  map[string]string{"APPIMAGE": image, "APPDIR": mount},
			want: mount + "-other/meron",
		},
		{
			name: "no APPDIR",
			exe:  inside,
			env:  map[string]string{"APPIMAGE": image},
			want: inside,
		},
		{
			name: "relative APPIMAGE",
			exe:  inside,
			env:  map[string]string{"APPIMAGE": "meron.appimage", "APPDIR": mount},
			want: inside,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := desktopLaunchTarget(tt.exe, func(key string) string { return tt.env[key] })
			if got != tt.want {
				t.Errorf("desktopLaunchTarget = %q, want %q", got, tt.want)
			}
		})
	}
}
