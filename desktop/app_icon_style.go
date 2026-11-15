package main

import (
	"encoding/json"
	"fmt"
	"os"
	"runtime"
	"sync"

	"meron/internal/appicon"
)

// defaultAppIconColour is what the colour picker starts on before the user has
// chosen anything.
const defaultAppIconColour = "#1d6fe0"

// appIconChoice is the user's app icon: the shipped one, or the same drawing
// in a colour of their own. The colour is kept while the shipped icon is in
// use, so switching back and forth does not lose it.
type appIconChoice struct {
	Custom bool   `json:"custom"`
	Colour string `json:"colour"`
}

// colour is the colour to draw the icon in, or false for the shipped icon.
func (c appIconChoice) colour() (appicon.Colour, bool) {
	if !c.Custom {
		return appicon.Colour{}, false
	}
	return appicon.ParseColour(c.Colour)
}

// Native storage owns the app icon choice, like the tray style: the icon is
// applied at startup, before the frontend or the core are up.
func loadAppIconChoice(path string) appIconChoice {
	choice := appIconChoice{Colour: defaultAppIconColour}
	data, err := os.ReadFile(path)
	if err != nil {
		return choice
	}
	var saved appIconChoice
	if json.Unmarshal(data, &saved) != nil {
		return choice
	}
	// A damaged colour falls back to the shipped icon rather than to a colour
	// the user never picked.
	colour, ok := appicon.ParseColour(saved.Colour)
	if !ok {
		return choice
	}
	return appIconChoice{Custom: saved.Custom, Colour: colour.Hex()}
}

func saveAppIconChoice(path string, choice appIconChoice) error {
	data, _ := json.Marshal(choice)
	if current, err := os.ReadFile(path); err == nil && string(current) == string(data) {
		return nil
	}
	return writeFileAtomic(path, data, 0o600, 0o700)
}

// appIconState is the choice plus what is currently drawn, so repeated
// requests for the same icon do nothing.
type appIconState struct {
	mu     sync.Mutex
	path   string
	choice appIconChoice
	// The colour of the icon on screen, as hex; empty for the shipped icon.
	drawn string
}

func (a *App) appIconGet() map[string]any {
	a.appIcon.mu.Lock()
	defer a.appIcon.mu.Unlock()
	return map[string]any{
		"custom": a.appIcon.choice.Custom,
		"colour": a.appIcon.choice.Colour,
		// Whether this platform lets the app replace its icon, and which
		// platform it is: the frontend describes the Dock, the launcher or
		// the taskbar accordingly.
		"supported": appIconStyleSupported(),
		"platform":  runtime.GOOS,
	}
}

func (a *App) appIconSet(payload map[string]any) (any, error) {
	custom, _ := payload["custom"].(bool)
	value, _ := payload["colour"].(string)
	colour, ok := appicon.ParseColour(value)
	if !ok {
		return nil, fmt.Errorf("appIcon.set: not a colour: %q", value)
	}
	choice := appIconChoice{Custom: custom, Colour: colour.Hex()}
	a.appIcon.mu.Lock()
	a.appIcon.choice = choice
	path := a.appIcon.path
	a.appIcon.mu.Unlock()
	// Apply the live choice even if native storage is temporarily read-only.
	a.applyAppIcon()
	persisted := true
	if path != "" {
		if err := saveAppIconChoice(path, choice); err != nil {
			persisted = false
			a.logf("app icon: cannot save the choice (it is still applied): %v", err)
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
	_, custom := a.appIcon.choice.colour()
	return custom
}

// applyAppIcon draws the chosen icon. It runs at startup and when the choice
// changes.
func (a *App) applyAppIcon() {
	if !appIconStyleSupported() {
		return
	}
	a.appIcon.mu.Lock()
	defer a.appIcon.mu.Unlock()
	colour, custom := a.appIcon.choice.colour()
	if !custom {
		if a.appIcon.drawn == "" {
			return
		}
		// Hand the icon back to the system rather than drawing the shipped
		// PNG ourselves: the installed icon keeps whatever treatment the OS
		// gives it.
		platformSetAppIcon("", nil)
		a.appIcon.drawn = ""
		return
	}
	if colour.Hex() == a.appIcon.drawn {
		return
	}
	rendered, err := appicon.Render(appIconPNG, colour)
	if err != nil {
		a.logf("app icon: %v", err)
		return
	}
	platformSetAppIcon(colour.Hex(), rendered)
	a.appIcon.drawn = colour.Hex()
}
