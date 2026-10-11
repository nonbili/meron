package main

import (
	"context"
	"encoding/base64"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"runtime"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

// On Linux, WebKitGTK hands the page a file dragged from the file manager only
// as its file:// address, never as a File, so the page can't read it. Wails'
// native file drop gets the paths from GTK instead; Go reads those files and
// sends them to the page (files.dropped), which attaches them where the drop
// landed (useFileDrop). macOS and Windows give the page real files and keep the
// webview's own drop untouched.
var nativeFileDrop = runtime.GOOS == "linux"

var (
	onFileDrop  = wailsRuntime.OnFileDrop
	eventsEmit  = wailsRuntime.EventsEmit
	fileDropLog = func(a *App, format string, args ...any) { a.logf(format, args...) }
)

func (a *App) setupFileDrop(ctx context.Context) {
	if !nativeFileDrop {
		return
	}
	onFileDrop(ctx, func(_, _ int, paths []string) {
		files := a.readDroppedFiles(paths)
		if len(files) > 0 {
			eventsEmit(ctx, "files.dropped", map[string]any{"files": files})
		}
	})
}

// readDroppedFiles reads the regular files among paths the OS reported for a
// drop, in the shape the file picker returns. Folders and unreadable entries
// are skipped (and logged), so one bad entry doesn't lose the rest.
func (a *App) readDroppedFiles(paths []string) []any {
	files := make([]any, 0, len(paths))
	for _, path := range paths {
		if path == "" {
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
		return nil, err
	}
	if !info.Mode().IsRegular() {
		return nil, fmt.Errorf("not a regular file")
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
