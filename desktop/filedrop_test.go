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

	files, skipped := (&App{}).readDroppedFiles([]string{pdf, dir, filepath.Join(dir, "missing.txt"), ""})
	if skipped != 2 {
		t.Fatalf("counted %d skipped, want 2", skipped)
	}

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

func TestDroppedFilesAreTypedByTheirName(t *testing.T) {
	dir := t.TempDir()
	svg := filepath.Join(dir, "logo.svg")
	if err := os.WriteFile(svg, []byte(`<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"/>`), 0o600); err != nil {
		t.Fatal(err)
	}
	files, _ := (&App{}).readDroppedFiles([]string{svg})
	if len(files) != 1 || files[0].(map[string]any)["mime"] != "image/svg+xml" {
		t.Fatalf("got %v, want an SVG image", files)
	}
	// The picker keeps sniffing: its image filter trusts content, not names.
	picked, err := readPickedFile(svg)
	if err != nil || picked["mime"] == "image/svg+xml" {
		t.Fatalf("picker typed the file by name: %v, %v", picked["mime"], err)
	}
}

func TestPathsLeftOutOfAPartlyMatchingReportAreCounted(t *testing.T) {
	dir := t.TempDir()
	note := filepath.Join(dir, "note.txt")
	if err := os.WriteFile(note, []byte("hello"), 0o600); err != nil {
		t.Fatal(err)
	}
	a := &App{}
	a.noteDroppedPaths([]string{note, filepath.Join(dir, "spelled-one-way.txt")})
	res, err := a.readDroppedFilesCommand(map[string]any{
		"uris": []any{"file://" + filepath.ToSlash(note), "file://" + filepath.ToSlash(dir) + "/spelled-another-way.txt"},
	})
	if err != nil {
		t.Fatal(err)
	}
	got := res.(map[string]any)
	if len(got["files"].([]any)) != 1 || got["skipped"] != 1 {
		t.Fatalf("got %d files, %v skipped, want the note and one skipped", len(got["files"].([]any)), got["skipped"])
	}
}

func TestAnEmptyNativeDropIsReportedAsSkipped(t *testing.T) {
	defer func(old time.Duration) { droppedPathsWait = old }(droppedPathsWait)
	droppedPathsWait = 0
	res, err := (&App{}).readDroppedFilesCommand(map[string]any{})
	if err != nil || res.(map[string]any)["skipped"] != 1 {
		t.Fatalf("got %v, %v, want one skipped entry", res, err)
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

	if files, skipped := (&App{}).readDroppedFiles([]string{big}); len(files) != 0 || skipped != 1 || logged != 1 {
		t.Fatalf("got %d files, %d skipped, %d log lines, want the file skipped and logged", len(files), skipped, logged)
	}
}

func TestReadDroppedFilesStopsAtTheTotalLimit(t *testing.T) {
	dir := t.TempDir()
	var paths []string
	for _, name := range []string{"a.bin", "b.bin"} {
		path := filepath.Join(dir, name)
		f, err := os.Create(path)
		if err != nil {
			t.Fatal(err)
		}
		// Each fits alone; together they are over the total.
		if err := f.Truncate(maxDroppedTotalBytes/2 + 1); err != nil {
			t.Fatal(err)
		}
		f.Close()
		paths = append(paths, path)
	}
	defer func(old func(*App, string, ...any)) { fileDropLog = old }(fileDropLog)
	fileDropLog = func(*App, string, ...any) {}

	if files, skipped := (&App{}).readDroppedFiles(paths); len(files) != 1 || skipped != 1 {
		t.Fatalf("got %d files and %d skipped, want the second file left out", len(files), skipped)
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
	if paths := a.takeDroppedPaths(nil, 0); len(paths) != 0 {
		t.Fatalf("the drop was handed out twice: %v", paths)
	}
}

func TestAnEarlierDropIsNotHandedOutForALaterOne(t *testing.T) {
	a := &App{}
	// Dropped on the sidebar a few seconds ago: recorded, never asked for.
	a.noteDroppedPaths([]string{"/home/me/private.txt"})
	a.droppedAt = time.Now().Add(-3 * time.Second)

	if paths := a.takeDroppedPaths(nil, 0); len(paths) != 0 {
		t.Fatalf("handed out an earlier drop to a page that named no files: %v", paths)
	}
	if paths := a.takeDroppedPaths([]string{"/home/me/other.txt"}, 0); len(paths) != 0 {
		t.Fatalf("handed out an earlier drop for other files: %v", paths)
	}
	// Still there for the page that names it.
	if paths := a.takeDroppedPaths([]string{"/home/me/private.txt"}, 0); len(paths) != 1 {
		t.Fatalf("got %v, want the named file", paths)
	}
}

func TestAnEarlierDropThatSharesAFileIsNotHandedOut(t *testing.T) {
	a := &App{}
	// [a, b] landed on the sidebar; [a, c] is dropped on a conversation and the
	// page asks before GTK has reported it.
	a.noteDroppedPaths([]string{"/home/me/a.pdf", "/home/me/b.pdf"})
	a.droppedAt = time.Now().Add(-3 * time.Second)
	go func() {
		time.Sleep(50 * time.Millisecond)
		a.noteDroppedPaths([]string{"/home/me/a.pdf", "/home/me/c.pdf"})
	}()
	paths := a.takeDroppedPaths([]string{"/home/me/a.pdf", "/home/me/c.pdf"}, 2*time.Second)
	if len(paths) != 2 || paths[1] != "/home/me/c.pdf" {
		t.Fatalf("got %v, want both files of the later drop", paths)
	}
}

func TestTheDropJustReportedIsHandedOutWhenThePageCannotNameIt(t *testing.T) {
	a := &App{}
	a.noteDroppedPaths([]string{"/home/me/note.txt"})
	if paths := a.takeDroppedPaths(nil, 0); len(paths) != 1 || paths[0] != "/home/me/note.txt" {
		t.Fatalf("got %v, want the drop GTK just reported", paths)
	}
	// Likewise when the page's address doesn't spell the path the way GTK does.
	a.noteDroppedPaths([]string{"/home/me/note.txt"})
	if paths := a.takeDroppedPaths([]string{"/home/me/link-to-note.txt"}, 0); len(paths) != 1 {
		t.Fatalf("got %v, want the drop GTK just reported", paths)
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
