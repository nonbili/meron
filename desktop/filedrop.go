package main

import (
	"context"
	"encoding/base64"
	"fmt"
	"mime"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"runtime"
	"slices"
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
	// GTK reports a drop within moments of the page seeing it; a report older
	// than this when the page asks belongs to an earlier drop.
	droppedPathsSameDrop = time.Second
	// A dropped file is base64-encoded and sent to the page in one piece; past
	// these sizes that stalls the app, and no mail server takes it anyway. The
	// page applies the same limits to whatever it attaches (lib/attachFiles.ts).
	maxDroppedFileBytes  = 100 << 20
	maxDroppedTotalBytes = 150 << 20
)

// The page's request and GTK's report of the same drop arrive in either order;
// the request waits this long for the report.
var droppedPathsWait = 2 * time.Second

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
// once. `wanted` are the paths the page saw in its drop event. A report GTK made
// around the time of the request is that drop: its wanted paths are handed out,
// or all of it when the page names none of them. An older one may be of an earlier drop that landed nowhere: it counts
// only if it holds every wanted path, and only those are handed out; a partial
// match is another drop, and the wait for this one's report goes on. Either
// way the page can't have Go read a file that wasn't dropped.
func (a *App) takeDroppedPaths(wanted []string, wait time.Duration) []string {
	asked := time.Now()
	deadline := asked.Add(wait)
	for {
		a.fileDropMu.Lock()
		var taken []string
		if time.Since(a.droppedAt) <= droppedPathsTTL {
			if !a.droppedAt.Before(asked.Add(-droppedPathsSameDrop)) {
				// What the page saw of it, if the two agree on any path at all:
				// they may spell paths differently.
				for _, path := range a.droppedPaths {
					if slices.Contains(wanted, path) {
						taken = append(taken, path)
					}
				}
				if len(taken) == 0 {
					taken = a.droppedPaths
				}
			} else if len(wanted) > 0 {
				for _, want := range wanted {
					if !slices.Contains(a.droppedPaths, want) {
						taken = nil
						break
					}
					taken = append(taken, want)
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
// native drop whose file:// addresses the page passes in `uris`, and how many
// dropped entries were left out, for the page to tell the reader.
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
	files, skipped := a.readDroppedFiles(paths)
	// Paths the page saw that GTK spells differently were left out of a
	// partly matching report: count them, so the reader is told.
	matched := 0
	for _, want := range wanted {
		if slices.Contains(paths, want) {
			matched++
		}
	}
	if matched > 0 {
		skipped += len(wanted) - matched
	}
	if len(paths) == 0 {
		// GTK reported nothing for what the page saw dropped: say so, also when
		// the page couldn't tell how many files that was.
		skipped = max(len(wanted), 1)
	}
	return map[string]any{"files": files, "skipped": skipped}, nil
}

// readDroppedFiles reads the regular files among paths the OS reported for a
// drop, in the shape the file picker returns. Folders, unreadable entries and
// whatever goes over the size limits are skipped (logged and counted), so one
// bad entry doesn't lose the rest.
func (a *App) readDroppedFiles(paths []string) (files []any, skipped int) {
	files = make([]any, 0, len(paths))
	var total int64
	for _, path := range paths {
		if path == "" {
			continue
		}
		info, err := os.Stat(path)
		if err == nil && (info.Size() > maxDroppedFileBytes || total+info.Size() > maxDroppedTotalBytes) {
			err = fmt.Errorf("%d bytes is over the limit", info.Size())
		}
		var file map[string]any
		if err == nil {
			// By its name, as the webview types a dropped file on macOS and
			// Windows: sniffing calls a .docx a zip and an .svg plain XML.
			file, err = readFileAs(path, info, true)
		}
		if err != nil {
			fileDropLog(a, "file drop: skipped %s: %v", filepath.Base(path), err)
			skipped++
			continue
		}
		total += info.Size()
		files = append(files, file)
	}
	return files, skipped
}

// readPickedFile reads one file into the {name, mime, data} shape the
// frontend turns back into a File (lib/nativeFilePicker.ts).
func readPickedFile(path string) (map[string]any, error) {
	info, err := os.Stat(path)
	if err != nil {
		return nil, fmt.Errorf("read file: %w", err)
	}
	return readFileAs(path, info, false)
}

// readFileAs reads the file `info` describes. Its type is sniffed from the
// content, or with `byExtension` taken from its name where that tells.
func readFileAs(path string, info os.FileInfo, byExtension bool) (map[string]any, error) {
	if !info.Mode().IsRegular() {
		return nil, fmt.Errorf("read file: %s is not a regular file", filepath.Base(path))
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("read file: %w", err)
	}
	contentType := http.DetectContentType(data)
	if byExtension {
		if named, _, err := mime.ParseMediaType(mime.TypeByExtension(filepath.Ext(path))); err == nil {
			contentType = named
		}
	}
	return map[string]any{
		"name": filepath.Base(path),
		"mime": contentType,
		"data": "data:" + contentType + ";base64," + base64.StdEncoding.EncodeToString(data),
	}, nil
}
