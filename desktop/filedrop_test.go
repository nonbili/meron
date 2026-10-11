package main

import (
	"context"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestReadDroppedFilesReadsFilesAndSkipsTheRest(t *testing.T) {
	dir := t.TempDir()
	pdf := filepath.Join(dir, "report.pdf")
	if err := os.WriteFile(pdf, []byte("%PDF-1.4 test"), 0o600); err != nil {
		t.Fatal(err)
	}
	logged := 0
	defer func(old func(*App, string, ...any)) { fileDropLog = old }(fileDropLog)
	fileDropLog = func(*App, string, ...any) { logged++ }

	files := (&App{}).readDroppedFiles([]string{pdf, dir, filepath.Join(dir, "missing.txt"), ""})

	if len(files) != 1 {
		t.Fatalf("got %d files, want 1: %v", len(files), files)
	}
	file := files[0].(map[string]any)
	if file["name"] != "report.pdf" || file["mime"] != "application/pdf" {
		t.Fatalf("unexpected file: %v", file)
	}
	if file["data"] != "data:application/pdf;base64,JVBERi0xLjQgdGVzdA==" {
		t.Fatalf("unexpected data: %v", file["data"])
	}
	if logged != 2 {
		t.Fatalf("logged %d skips, want 2 (the folder and the missing file)", logged)
	}
}

func TestReadDroppedFilesSkipsOversizedFiles(t *testing.T) {
	dir := t.TempDir()
	big := filepath.Join(dir, "big.bin")
	f, err := os.Create(big)
	if err != nil {
		t.Fatal(err)
	}
	// Sparse: the size counts, nothing is written.
	if err := f.Truncate(maxDroppedFileBytes + 1); err != nil {
		t.Fatal(err)
	}
	f.Close()
	defer func(old func(*App, string, ...any)) { fileDropLog = old }(fileDropLog)
	logged := 0
	fileDropLog = func(*App, string, ...any) { logged++ }

	if files := (&App{}).readDroppedFiles([]string{big}); len(files) != 0 || logged != 1 {
		t.Fatalf("got %d files and %d log lines, want the file skipped and logged", len(files), logged)
	}
}

// droppedNames runs system.readDroppedFiles and lists the names it returns.
func droppedNames(t *testing.T, a *App, uris ...any) []string {
	t.Helper()
	res, err := a.readDroppedFilesCommand(map[string]any{"uris": uris})
	if err != nil {
		t.Fatal(err)
	}
	names := []string{}
	for _, file := range res.(map[string]any)["files"].([]any) {
		names = append(names, file.(map[string]any)["name"].(string))
	}
	return names
}

func TestDroppedFilesAreReadOnlyWhenThePageAsksForThem(t *testing.T) {
	defer func(old bool) { nativeFileDrop = old }(nativeFileDrop)
	defer func(old func(context.Context, func(int, int, []string))) { onFileDrop = old }(onFileDrop)
	nativeFileDrop = true
	var drop func(int, int, []string)
	onFileDrop = func(_ context.Context, callback func(int, int, []string)) { drop = callback }

	a := &App{}
	a.setupFileDrop(context.Background())
	if drop == nil {
		t.Fatal("no file drop callback registered")
	}

	dir := t.TempDir()
	note := filepath.Join(dir, "my note.txt")
	other := filepath.Join(dir, "other.txt")
	for _, path := range []string{note, other} {
		if err := os.WriteFile(path, []byte("hello"), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	noteURI := "file://" + filepath.ToSlash(dir) + "/my%20note.txt"

	// A drop that landed nowhere, then the one the page asks about.
	drop(0, 0, []string{other})
	drop(10, 20, []string{note})
	if names := droppedNames(t, a, noteURI); len(names) != 1 || names[0] != "my note.txt" {
		t.Fatalf("got %v, want the dropped note", names)
	}
	// Handed out once.
	if names := a.readDroppedFiles(a.takeDroppedPaths(nil, 0)); len(names) != 0 {
		t.Fatalf("the drop was handed out twice: %v", names)
	}
}

func TestDroppedFilesAreLimitedToWhatWasDropped(t *testing.T) {
	dir := t.TempDir()
	dropped := filepath.Join(dir, "dropped.txt")
	secret := filepath.Join(dir, "secret.txt")
	for _, path := range []string{dropped, secret} {
		if err := os.WriteFile(path, []byte("hello"), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	a := &App{}
	a.noteDroppedPaths([]string{dropped})
	if paths := a.takeDroppedPaths([]string{secret}, 0); len(paths) != 0 {
		t.Fatalf("handed out a file that was not dropped: %v", paths)
	}
	// The page could not read the addresses: the latest drop, whole.
	if paths := a.takeDroppedPaths(nil, 0); len(paths) != 1 || paths[0] != dropped {
		t.Fatalf("got %v, want the latest drop", paths)
	}
}

func TestDroppedFilesWaitForTheNativeReport(t *testing.T) {
	dir := t.TempDir()
	note := filepath.Join(dir, "note.txt")
	if err := os.WriteFile(note, []byte("hello"), 0o600); err != nil {
		t.Fatal(err)
	}
	a := &App{}
	go func() {
		time.Sleep(50 * time.Millisecond)
		a.noteDroppedPaths([]string{note})
	}()
	if paths := a.takeDroppedPaths([]string{note}, 2*time.Second); len(paths) != 1 {
		t.Fatalf("got %v, want the path reported after the page asked", paths)
	}
}

func TestDroppedPathsExpire(t *testing.T) {
	a := &App{}
	a.noteDroppedPaths([]string{"/tmp/old.txt"})
	a.droppedAt = time.Now().Add(-droppedPathsTTL - time.Second)
	if paths := a.takeDroppedPaths(nil, 0); len(paths) != 0 {
		t.Fatalf("handed out an expired drop: %v", paths)
	}
}

func TestSetupFileDropIsOffWhereTheWebviewGivesRealFiles(t *testing.T) {
	defer func(old bool) { nativeFileDrop = old }(nativeFileDrop)
	defer func(old func(context.Context, func(int, int, []string))) { onFileDrop = old }(onFileDrop)
	nativeFileDrop = false
	registered := false
	onFileDrop = func(context.Context, func(int, int, []string)) { registered = true }
	(&App{}).setupFileDrop(context.Background())
	if registered {
		t.Fatal("file drop registered although native drop is off")
	}
}
