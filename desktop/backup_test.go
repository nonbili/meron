package main

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
	"time"
)

func TestBackupFilenameIsDated(t *testing.T) {
	got := backupFilename(time.Date(2026, 8, 10, 15, 4, 5, 0, time.UTC))
	if want := "meron-backup-2026-08-10.json"; got != want {
		t.Fatalf("backupFilename() = %q, want %q", got, want)
	}
}

// The sidecar's summary counts arrive as JSON numbers, which decode as float64.
func TestJSONInt(t *testing.T) {
	tests := map[string]struct {
		value any
		want  int
	}{
		"json number": {float64(7), 7},
		"zero":        {float64(0), 0},
		"missing key": {nil, 0},
		"wrong type":  {"12", 0},
	}
	for name, tc := range tests {
		if got := jsonInt(tc.value); got != tc.want {
			t.Errorf("%s: jsonInt(%v) = %d, want %d", name, tc.value, got, tc.want)
		}
	}
}

// Exporting secrets without a passphrase must fail before the sidecar is asked
// for anything, so a plaintext file carrying passwords cannot be produced even
// if the core's own guard regressed.
func TestExportBackupRequiresPassphraseForSecrets(t *testing.T) {
	app := &App{}
	_, err := app.exportBackup(map[string]any{"include_secrets": true})
	if err == nil {
		t.Fatal("exportBackup() with secrets and no passphrase succeeded, want error")
	}
	if got := err.Error(); got != "a passphrase is required to include account passwords" {
		t.Fatalf("exportBackup() error = %q", got)
	}
}

// os.WriteFile's mode applies only on create, so overwriting a world-readable
// file would leave the backup readable by other users.
func TestWritePrivateFileTightensAnExistingFilesMode(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("Unix file modes are not meaningful on Windows")
	}
	path := filepath.Join(t.TempDir(), "meron-backup.json")
	if err := os.WriteFile(path, []byte("stale"), 0o644); err != nil {
		t.Fatal(err)
	}

	if err := writePrivateFile(path, []byte("fresh")); err != nil {
		t.Fatalf("writePrivateFile() = %v", err)
	}

	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if got := info.Mode().Perm(); got != 0o600 {
		t.Errorf("mode = %#o, want 0600", got)
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != "fresh" {
		t.Errorf("contents = %q, want %q", data, "fresh")
	}
}

func TestWritePrivateFileCreatesWithOwnerOnlyMode(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("Unix file modes are not meaningful on Windows")
	}
	path := filepath.Join(t.TempDir(), "new-backup.json")

	if err := writePrivateFile(path, []byte("data")); err != nil {
		t.Fatalf("writePrivateFile() = %v", err)
	}

	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if got := info.Mode().Perm(); got != 0o600 {
		t.Errorf("mode = %#o, want 0600", got)
	}
}
