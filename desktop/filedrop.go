package main

import (
	"context"
	"encoding/base64"
	"fmt"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"runtime"
	"time"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

// On Linux, WebKitGTK hands the page a file dragged from the file manager only
// as its file:// address, never as a File, so the page can't read it. Wails'
// native file drop gets the paths from GTK instead and Go keeps them for a
// moment; when the drop landed on a drop area, the page asks for the files
// (system.readDroppedFiles, useFileDrop) and Go reads them. A drop anywhere
// else is never read. macOS and Windows give the page real files and keep the
// webview's own drop untouched.
var nativeFileDrop = runtime.GOOS == "linux"

const (
	// How long the paths of a native drop stay available to the page.
	droppedPathsTTL = 10 * time.Second
	// The page's request and GTK's report of the same drop arrive in either
	// order; the request waits this long for the report.
	droppedPathsWait = 2 * time.Second
	// A dropped file is base64-encoded and sent to the page in one piece; past
	// this size that stalls the app, and no mail server takes it anyway.
	maxDroppedFileBytes = 100 << 20
)

var (
	onFileDrop  = wailsRuntime.OnFileDrop
	fileDropLog = func(a *App, format string, args ...any) { a.logf(format, args...) }
)

func (a *App) setupFileDrop(ctx context.Context) {
	if !nativeFileDrop {
		return
	}
	onFileDrop(ctx, func(_, _ int, paths []string) {
		a.noteDroppedPaths(paths)
	})
}

func (a *App) noteDroppedPaths(paths []string) {
	dropped := make([]string, 0, len(paths))
	for _, path := range paths {
		if path != "" {
			dropped = append(dropped, filepath.Clean(path))
		}
	}
	a.fileDropMu.Lock()
	a.droppedPaths = dropped
	a.droppedAt = time.Now()
	a.fileDropMu.Unlock()
}

// takeDroppedPaths hands out the paths of the native drop the page asks about,
// once. `wanted` are the paths the page saw in its drop event; only those GTK
// reported too are returned, so the page can't have Go read a file that wasn't
// dropped, and an earlier drop that landed nowhere isn't mistaken for this one.
// With no `wanted` (the page couldn't read the addresses) the latest drop is
// returned whole.
func (a *App) takeDroppedPaths(wanted []string, wait time.Duration) []string {
	deadline := time.Now().Add(wait)
	for {
		a.fileDropMu.Lock()
		var taken []string
		if time.Since(a.droppedAt) <= droppedPathsTTL {
			if len(wanted) == 0 {
				taken = a.droppedPaths
			} else {
				for _, path := range a.droppedPaths {
					for _, want := range wanted {
						if path == want {
							taken = append(taken, path)
							break
						}
					}
				}
			}
		}
		if len(taken) > 0 {
			a.droppedPaths = nil
		}
		a.fileDropMu.Unlock()
		if len(taken) > 0 || !time.Now().Before(deadline) {
			return taken
		}
		time.Sleep(20 * time.Millisecond)
	}
}

// readDroppedFilesCommand answers system.readDroppedFiles: the files of the
// native drop whose file:// addresses the page passes in `uris`.
func (a *App) readDroppedFilesCommand(payload map[string]any) (any, error) {
	var wanted []string
	if uris, ok := payload["uris"].([]any); ok {
		for _, raw := range uris {
			text, _ := raw.(string)
			if parsed, err := url.Parse(text); err == nil && parsed.Scheme == "file" && parsed.Path != "" {
				wanted = append(wanted, filepath.Clean(parsed.Path))
			}
		}
	}
	paths := a.takeDroppedPaths(wanted, droppedPathsWait)
	return map[string]any{"files": a.readDroppedFiles(paths)}, nil
}

// readDroppedFiles reads the regular files among paths the OS reported for a
// drop, in the shape the file picker returns. Folders, oversized and unreadable
// entries are skipped (and logged), so one bad entry doesn't lose the rest.
func (a *App) readDroppedFiles(paths []string) []any {
	files := make([]any, 0, len(paths))
	for _, path := range paths {
		if path == "" {
			continue
		}
		if info, err := os.Stat(path); err == nil && info.Size() > maxDroppedFileBytes {
			fileDropLog(a, "file drop: skipped %s: %d bytes is over the %d limit", filepath.Base(path), info.Size(), maxDroppedFileBytes)
			continue
		}
		file, err := readPickedFile(path)
		if err != nil {
			fileDropLog(a, "file drop: skipped %s: %v", filepath.Base(path), err)
			continue
		}
		files = append(files, file)
	}
	return files
}

// readPickedFile reads one file into the {name, mime, data} shape the
// frontend turns back into a File (lib/nativeFilePicker.ts).
func readPickedFile(path string) (map[string]any, error) {
	info, err := os.Stat(path)
	if err != nil {
		return nil, fmt.Errorf("read file: %w", err)
	}
	if !info.Mode().IsRegular() {
		return nil, fmt.Errorf("read file: %s is not a regular file", filepath.Base(path))
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("read file: %w", err)
	}
	mime := http.DetectContentType(data)
	return map[string]any{
		"name": filepath.Base(path),
		"mime": mime,
		"data": "data:" + mime + ";base64," + base64.StdEncoding.EncodeToString(data),
	}, nil
}
