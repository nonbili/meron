package main

import (
	"fmt"
	"runtime"
	"strings"
	"sync/atomic"
	"time"

	"github.com/wailsapp/wails/v3/pkg/application"
	"github.com/wailsapp/wails/v3/pkg/events"
	"meron/internal/desktopruntime"
)

// A tray composer has its own webview and draft state. Repeated tray actions
// focus it, so an unfinished message is never covered by a second empty editor.
type composeWindow struct {
	window            emlReaderWindow
	closing           atomic.Bool
	draftID           string
	session           string
	closeRequestedAt  time.Time
	closeRequest      uint64
	closeAcknowledged bool
}

var newComposeWindow = func(options application.WebviewWindowOptions, requested func() bool, closed func(), dropped func([]string)) emlReaderWindow {
	window := desktopruntime.App.Window.NewWithOptions(options)
	if options.EnableFileDrop {
		window.OnWindowEvent(events.Common.WindowFilesDropped, func(event *application.WindowEvent) { dropped(event.Context().DroppedFiles()) })
	}
	window.RegisterHook(events.Common.WindowClosing, func(event *application.WindowEvent) {
		if requested() {
			event.Cancel()
		}
	})
	window.OnWindowEvent(events.Common.WindowClosing, func(_ *application.WindowEvent) { closed() })
	return window
}

func (a *App) openComposeWindow() {
	a.composeOpenMu.Lock()
	defer a.composeOpenMu.Unlock()
	a.composeMu.Lock()
	existing := a.composeWindow
	a.composeMu.Unlock()
	if existing != nil {
		existing.window.Show()
		existing.window.Focus()
		return
	}
	composer := &composeWindow{}
	window := newComposeWindow(application.WebviewWindowOptions{
		Name: "composer", Title: a.currentNativeLabels().trayCompose + " — " + appTitle(),
		URL: "/?compose=standalone", Width: 840, Height: 720,
		MinWidth: 480, MinHeight: 360, Hidden: true, EnableFileDrop: nativeFileDrop || runtime.GOOS == "windows",
	}, func() bool {
		return a.composeWindowCloseRequested(composer)
	}, func() {
		a.composeMu.Lock()
		if a.composeWindow == composer {
			a.composeWindow = nil
		}
		a.composeMu.Unlock()
		desktopruntime.EventsEmit(a.ctx, "composer.draftChanged", "")
	}, a.noteDroppedPaths)
	composer.window = window
	a.composeMu.Lock()
	a.composeWindow = composer
	a.composeMu.Unlock()
	window.Show()
	window.Focus()
}

func (a *App) composeWindowCommand(command string, payload map[string]any) (any, error) {
	a.composeMu.Lock()
	if command == "composer.setMainDrafts" {
		a.mainComposeDrafts = make(map[string]bool)
		if ids, ok := payload["draft_ids"].([]any); ok {
			for _, raw := range ids {
				if id, ok := raw.(string); ok {
					a.mainComposeDrafts[normalizeComposeDraftID(id)] = true
				}
			}
		}
		a.composeMu.Unlock()
		return map[string]any{"ok": true}, nil
	}
	composer := a.composeWindow
	if command == "composer.isOpen" {
		a.composeMu.Unlock()
		return composer != nil, nil
	}
	if composer == nil {
		a.composeMu.Unlock()
		if command == "composer.getDraft" {
			return "", nil
		}
		if command == "composer.focusDraft" {
			return false, nil
		}
		return nil, fmt.Errorf("composer window is closed")
	}
	switch command {
	case "composer.getDraft":
		id := composer.draftID
		a.composeMu.Unlock()
		return id, nil
	case "composer.closeResponded":
		request, _ := payload["request"].(float64)
		if uint64(request) == composer.closeRequest {
			composer.closeAcknowledged = true
		}
		a.composeMu.Unlock()
	case "composer.claimDraft":
		id, _ := payload["draft_id"].(string)
		claimed := id != "" && !a.mainComposeDrafts[normalizeComposeDraftID(id)]
		for _, reader := range a.threadWindows {
			if reader.ownsDraft(id) {
				claimed = false
			}
		}
		if claimed {
			composer.draftID = id
		}
		a.composeMu.Unlock()
		if claimed {
			desktopruntime.EventsEmit(a.ctx, "composer.draftChanged", id)
		}
		return claimed, nil
	case "composer.setDraft":
		composer.draftID, _ = payload["draft_id"].(string)
		id := composer.draftID
		a.composeMu.Unlock()
		desktopruntime.EventsEmit(a.ctx, "composer.draftChanged", id)
	case "composer.focusDraft":
		id, _ := payload["draft_id"].(string)
		matches := id != "" && normalizeComposeDraftID(id) == normalizeComposeDraftID(composer.draftID)
		a.composeMu.Unlock()
		if matches {
			composer.window.Show()
			composer.window.Focus()
		}
		return matches, nil
	case "composer.discardDraft":
		a.composeMu.Unlock()
		// The main webview stays alive after the composer closes and owns the
		// existing retry/suppression logic for discarded and sent drafts.
		desktopruntime.EventsEmit(a.ctx, "composer.discardDraft", payload)
	case "composer.closeWindow":
		composer.closing.Store(true)
		a.composeMu.Unlock()
		composer.window.Close()
	default:
		a.composeMu.Unlock()
		return nil, fmt.Errorf("unknown composer command: %s", command)
	}
	return map[string]any{"ok": true}, nil
}

func normalizeComposeDraftID(id string) string {
	return strings.ToLower(strings.Trim(strings.TrimSpace(id), "<>"))
}

const composeCloseResponseTimeout = 3 * time.Second

// An unresponsive/crashed page cannot save. After giving it time to answer,
// let a repeated native close dismiss it. An acknowledged save may take longer
// and must never be interrupted by this fallback.
func (a *App) composeWindowCloseRequested(composer *composeWindow) bool {
	if composer.closing.Load() || a.quitting.Load() {
		return false
	}
	a.composeMu.Lock()
	if !composer.closeRequestedAt.IsZero() && !composer.closeAcknowledged {
		if time.Since(composer.closeRequestedAt) >= composeCloseResponseTimeout {
			composer.closing.Store(true)
			a.composeMu.Unlock()
			return false
		}
	} else {
		composer.closeRequest++
		composer.closeRequestedAt = time.Now()
		composer.closeAcknowledged = false
	}
	request := composer.closeRequest
	a.composeMu.Unlock()
	event := "composer.closeRequested"
	if composer.session != "" {
		event = "threadWindow.closeRequested"
	}
	desktopruntime.EventsEmit(a.ctx, event, map[string]any{"request": request, "session": composer.session})
	return true
}
