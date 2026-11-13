package main

import (
	"encoding/json"
	"fmt"
	"os"
	"runtime"
	"sync"

	"meron/internal/appicon"
)

// Native storage owns the app icon style, like the tray style: the icon is
// applied at startup, before the frontend or the core are up.
func loadAppIconStyle(path string) appicon.Style {
	data, err := os.ReadFile(path)
	if err != nil {
		return appicon.StyleDefault
	}
	var saved struct {
		Style string `json:"style"`
	}
	if json.Unmarshal(data, &saved) != nil {
		return appicon.StyleDefault
	}
	style, _ := appicon.ParseStyle(saved.Style)
	return style
}

func saveAppIconStyle(path string, style appicon.Style) error {
	data, _ := json.Marshal(struct {
		Style appicon.Style `json:"style"`
	}{style})
	if current, err := os.ReadFile(path); err == nil && string(current) == string(data) {
		return nil
	}
	return writeFileAtomic(path, data, 0o600, 0o700)
}

// appIconState is the chosen style plus what is currently drawn, so repeated
// requests and appearance changes that resolve to the same icon do nothing.
type appIconState struct {
	mu      sync.Mutex
	path    string
	style   appicon.Style
	drawn   appicon.Variant
	renders map[appicon.Variant][]byte
}

func (a *App) appIconGetStyle() map[string]any {
	a.appIcon.mu.Lock()
	defer a.appIcon.mu.Unlock()
	return map[string]any{"style": string(a.appIcon.style), "supported": appIconStyleSupported()}
}

func (a *App) appIconSetStyle(payload map[string]any) (any, error) {
	value, _ := payload["style"].(string)
	style, ok := appicon.ParseStyle(value)
	if !ok {
		return nil, fmt.Errorf("appIcon.setStyle: unknown style %q", value)
	}
	a.appIcon.mu.Lock()
	a.appIcon.style = style
	path := a.appIcon.path
	a.appIcon.mu.Unlock()
	// Apply the live choice even if native storage is temporarily read-only.
	a.applyAppIcon()
	persisted := true
	if path != "" {
		if err := saveAppIconStyle(path, style); err != nil {
			persisted = false
			a.logf("app icon: cannot save style (live choice still applied): %v", err)
		}
	}
	return map[string]any{"ok": true, "persisted": persisted}, nil
}

// launcherIconPending reports whether applyAppIcon, rather than startup, will
// write the Linux desktop entry: the launcher there shows the chosen icon.
func (a *App) launcherIconPending() bool {
	if runtime.GOOS != "linux" || !appIconStyleSupported() {
		return false
	}
	a.appIcon.mu.Lock()
	defer a.appIcon.mu.Unlock()
	return a.appIcon.style != appicon.StyleDefault
}

// applyAppIcon draws the icon the style resolves to right now. It runs at
// startup, when the style changes and when the system appearance changes.
func (a *App) applyAppIcon() {
	if !appIconStyleSupported() {
		return
	}
	systemAppearanceMu.Lock()
	dark, known := systemAppearanceDark, systemAppearanceKnown
	systemAppearanceMu.Unlock()

	a.appIcon.mu.Lock()
	defer a.appIcon.mu.Unlock()
	variant := a.appIcon.style.Resolve(dark, known)
	if variant == a.appIcon.drawn {
		return
	}
	if variant == appicon.VariantColour {
		// Hand the icon back to the system rather than drawing the shipped
		// PNG ourselves: the installed icon keeps whatever treatment the OS gives it.
		platformSetAppIcon(variant, nil)
		a.appIcon.drawn = variant
		return
	}
	rendered, ok := a.appIcon.renders[variant]
	if !ok {
		var err error
		if rendered, err = appicon.Render(appIconPNG, variant); err != nil {
			a.logf("app icon: %v", err)
			return
		}
		if a.appIcon.renders == nil {
			a.appIcon.renders = map[appicon.Variant][]byte{}
		}
		a.appIcon.renders[variant] = rendered
	}
	platformSetAppIcon(variant, rendered)
	a.appIcon.drawn = variant
}
