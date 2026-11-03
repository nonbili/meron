package main

import (
	"crypto/sha256"
	"fmt"
	"io"
	"mime"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"time"
)

// Fetch only on an explicit save/copy action, never while rendering a message.
// The native client avoids webview CORS restrictions on feed image hosts.
func (a *App) cacheRemoteImage(payload map[string]any) (any, error) {
	rawURL, _ := payload["url"].(string)
	u, err := url.Parse(rawURL)
	if err != nil || (u.Scheme != "http" && u.Scheme != "https") || u.Host == "" {
		return nil, fmt.Errorf("invalid remote image URL")
	}
	client := &http.Client{Timeout: 30 * time.Second}
	resp, err := client.Get(u.String())
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("image download returned %s", resp.Status)
	}
	const maxSize = 50 * 1024 * 1024
	raw, err := io.ReadAll(io.LimitReader(resp.Body, maxSize+1))
	if err != nil {
		return nil, err
	}
	if len(raw) == 0 || len(raw) > maxSize {
		return nil, fmt.Errorf("remote image is empty or too large")
	}
	imageMime := http.DetectContentType(raw)
	// AVIF and SVG aren't recognized by DetectContentType. Accept their image
	// header as well; these bytes are saved, never executed or opened here.
	if !strings.HasPrefix(imageMime, "image/") {
		imageMime, _, _ = mime.ParseMediaType(resp.Header.Get("Content-Type"))
	}
	if !strings.HasPrefix(imageMime, "image/") {
		return nil, fmt.Errorf("remote resource is not an image")
	}
	exts, _ := mime.ExtensionsByType(imageMime)
	ext := ".img"
	if len(exts) > 0 {
		ext = exts[0]
	}
	if imageMime == "image/jpeg" {
		ext = ".jpg"
	}
	key := fmt.Sprintf("remote-images/%x%s", sha256.Sum256([]byte(rawURL)), ext)
	dest, err := resolveMediaPath(key)
	if err != nil {
		return nil, err
	}
	if err := os.MkdirAll(filepath.Dir(dest), 0o755); err != nil {
		return nil, err
	}
	if err := os.WriteFile(dest, raw, 0o600); err != nil {
		return nil, err
	}
	return map[string]any{"key": key, "mime": imageMime}, nil
}
