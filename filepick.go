package main

import (
	"encoding/base64"
	"errors"
	"fmt"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

func (a *App) pickImageFile(payload map[string]any) (any, error) {
	res, err := a.pickFiles(payload, true)
	if err != nil {
		return nil, err
	}
	resMap, _ := res.(map[string]any)
	if cancelled, _ := resMap["cancelled"].(bool); cancelled {
		return resMap, nil
	}
	files, _ := resMap["files"].([]any)
	if len(files) == 0 {
		return map[string]any{"cancelled": true}, nil
	}
	return files[0], nil
}

func (a *App) pickFiles(payload map[string]any, imagesOnly bool) (any, error) {
	title, _ := payload["title"].(string)
	if title == "" {
		if imagesOnly {
			title = "Choose image"
		} else {
			title = "Choose files"
		}
	}

	a.filePickerMu.Lock()
	defaultDir := a.lastFileDir
	lastDirPath := lastFileDirPath()
	if imagesOnly {
		defaultDir = a.lastImageDir
		lastDirPath = lastImageDirPath()
	}
	if defaultDir == "" {
		if data, err := os.ReadFile(lastDirPath); err == nil {
			defaultDir = strings.TrimSpace(string(data))
			if imagesOnly {
				a.lastImageDir = defaultDir
			} else {
				a.lastFileDir = defaultDir
			}
		}
	}
	a.filePickerMu.Unlock()

	defaultDir = filePickerDefaultDir(defaultDir)

	options := wailsRuntime.OpenDialogOptions{
		Title:            title,
		DefaultDirectory: defaultDir,
	}
	if imagesOnly {
		options.Filters = []wailsRuntime.FileFilter{
			{DisplayName: "Image files (*.png, *.jpg, *.jpeg, *.webp, *.gif)", Pattern: "*.png;*.jpg;*.jpeg;*.webp;*.gif"},
		}
	}

	paths, err := wailsRuntime.OpenMultipleFilesDialog(a.ctx, options)
	if err != nil {
		return nil, err
	}
	if len(paths) == 0 {
		return map[string]any{"cancelled": true}, nil
	}

	files := make([]any, 0, len(paths))
	for _, path := range paths {
		data, err := os.ReadFile(path)
		if err != nil {
			return nil, fmt.Errorf("read file: %w", err)
		}
		mime := http.DetectContentType(data)
		if imagesOnly && !strings.HasPrefix(mime, "image/") {
			return nil, errors.New("selected file is not an image")
		}
		files = append(files, map[string]any{
			"name": filepath.Base(path),
			"mime": mime,
			"data": "data:" + mime + ";base64," + base64.StdEncoding.EncodeToString(data),
		})
	}

	a.filePickerMu.Lock()
	nextDir := filepath.Dir(paths[0])
	if imagesOnly {
		a.lastImageDir = nextDir
	} else {
		a.lastFileDir = nextDir
	}
	if err := os.MkdirAll(filepath.Dir(lastDirPath), 0o755); err == nil {
		_ = os.WriteFile(lastDirPath, []byte(nextDir), 0o600)
	}
	a.filePickerMu.Unlock()

	return map[string]any{"files": files}, nil
}

// openComposerAttachment writes an in-memory composer attachment to the app
// cache and asks the OS to open it with the user's default handler.
func (a *App) openComposerAttachment(payload map[string]any) (any, error) {
	filename, _ := payload["filename"].(string)
	data, _ := payload["data"].(string)
	if data == "" {
		return nil, errors.New("missing attachment data")
	}
	if filename == "" {
		filename = "attachment"
	}
	if i := strings.Index(data, ","); strings.HasPrefix(data, "data:") && i >= 0 {
		data = data[i+1:]
	}
	bytes, err := base64.StdEncoding.DecodeString(data)
	if err != nil {
		return nil, fmt.Errorf("decode attachment: %w", err)
	}

	dir := filepath.Join(appCacheDir(), "composer-open")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return nil, err
	}
	path := filepath.Join(dir, randomBase64URL(8)+"-"+safeFilename(filename))
	if err := os.WriteFile(path, bytes, 0o600); err != nil {
		return nil, fmt.Errorf("write attachment: %w", err)
	}
	if err := openSystemFile(path); err != nil {
		return nil, err
	}
	return map[string]any{"opened": true, "path": path}, nil
}

func safeFilename(name string) string {
	name = filepath.Base(strings.TrimSpace(name))
	if name == "." || name == string(os.PathSeparator) || name == "" {
		return "attachment"
	}
	replacer := strings.NewReplacer("/", "_", "\\", "_", "\x00", "_", ":", "_")
	return replacer.Replace(name)
}

func openSystemFile(path string) error {
	var cmd *exec.Cmd
	switch runtime.GOOS {
	case "darwin":
		cmd = exec.Command("open", path)
	case "windows":
		cmd = exec.Command("rundll32", "url.dll,FileProtocolHandler", path)
	default:
		cmd = exec.Command("xdg-open", path)
	}
	if err := cmd.Start(); err != nil {
		return fmt.Errorf("open attachment: %w", err)
	}
	_ = cmd.Process.Release()
	return nil
}

func lastImageDirPath() string {
	return filepath.Join(appConfigDir(), "last-image-dir")
}

func lastFileDirPath() string {
	return filepath.Join(appConfigDir(), "last-file-dir")
}

func filePickerDefaultDir(candidate string) string {
	if dir, ok := usablePickerDir(candidate); ok {
		return dir
	}
	home, err := os.UserHomeDir()
	if err == nil {
		if dir, ok := usablePickerDir(home); ok {
			return dir
		}
	}
	return ""
}

func usablePickerDir(path string) (string, bool) {
	if path == "" {
		return "", false
	}
	info, err := os.Stat(path)
	if err != nil || !info.IsDir() {
		return "", false
	}
	realPath, err := filepath.EvalSymlinks(path)
	if err == nil && realPath != "" {
		return realPath, true
	}
	return path, true
}
