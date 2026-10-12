package main

import (
	"encoding/json"
	"os"
)

// Native storage owns tray style, independently of frontend/core startup.
// Only Go chooses the initial platform default.
func loadTrayStyle(path string) bool {
	data, err := os.ReadFile(path)
	if err != nil {
		return trayMonochromeDefault
	}
	var saved struct {
		Monochrome *bool `json:"monochrome"`
	}
	if json.Unmarshal(data, &saved) != nil || saved.Monochrome == nil {
		return trayMonochromeDefault
	}
	return *saved.Monochrome
}

func saveTrayStyle(path string, monochrome bool) error {
	data, _ := json.Marshal(struct {
		Monochrome bool `json:"monochrome"`
	}{monochrome})
	if current, err := os.ReadFile(path); err == nil && string(current) == string(data) {
		return nil
	}
	return writeFileAtomic(path, data, 0o600, 0o700)
}

func (a *App) trayGetStyle() map[string]any {
	return map[string]any{"monochrome": a.trayState().state().monochrome}
}
