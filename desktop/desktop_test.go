package main

import (
	"os"
	"path/filepath"
	"testing"
)

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
