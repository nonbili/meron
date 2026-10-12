package main

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestAtomicFileSupportsPrivateConfigPublicIconsAndExecutables(t *testing.T) {
	path := filepath.Join(t.TempDir(), "nested", "file")
	for _, mode := range []os.FileMode{0o600, 0o644, 0o755} {
		if err := writeFileAtomic(path, []byte("complete replacement"), mode, 0o700); err != nil {
			t.Fatal(err)
		}
		data, err := os.ReadFile(path)
		if err != nil || string(data) != "complete replacement" {
			t.Fatalf("invalid replacement: %q, %v", data, err)
		}
		info, err := os.Stat(path)
		if err != nil {
			t.Fatal(err)
		}
		if runtime.GOOS != "windows" && info.Mode().Perm() != mode {
			t.Fatalf("mode %o, want %o", info.Mode().Perm(), mode)
		}
	}
	entries, err := os.ReadDir(filepath.Dir(path))
	if err != nil || len(entries) != 1 {
		t.Fatalf("temporary files remain: %v, %v", entries, err)
	}
}
