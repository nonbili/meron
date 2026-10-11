package main

import (
	"context"
	"os"
	"path/filepath"
	"testing"
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

func TestSetupFileDropSendsReadFilesToThePage(t *testing.T) {
	defer func(old bool) { nativeFileDrop = old }(nativeFileDrop)
	defer func(old func(context.Context, func(int, int, []string))) { onFileDrop = old }(onFileDrop)
	defer func(old func(context.Context, string, ...any)) { eventsEmit = old }(eventsEmit)

	nativeFileDrop = true
	var drop func(int, int, []string)
	onFileDrop = func(_ context.Context, callback func(int, int, []string)) { drop = callback }
	var emitted []any
	eventsEmit = func(_ context.Context, name string, data ...any) {
		if name == "files.dropped" {
			emitted = append(emitted, data...)
		}
	}

	(&App{}).setupFileDrop(context.Background())
	if drop == nil {
		t.Fatal("no file drop callback registered")
	}

	dir := t.TempDir()
	note := filepath.Join(dir, "note.txt")
	if err := os.WriteFile(note, []byte("hello"), 0o600); err != nil {
		t.Fatal(err)
	}
	drop(10, 20, []string{note})
	if len(emitted) != 1 {
		t.Fatalf("emitted %d payloads, want 1", len(emitted))
	}
	files := emitted[0].(map[string]any)["files"].([]any)
	if len(files) != 1 || files[0].(map[string]any)["name"] != "note.txt" {
		t.Fatalf("unexpected payload: %v", emitted[0])
	}

	emitted = nil
	drop(0, 0, []string{""})
	if len(emitted) != 0 {
		t.Fatal("a drop without readable files should send nothing")
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
