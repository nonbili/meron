package main

import (
	"fmt"
	"net/url"
	"runtime"
	"strings"

	"github.com/google/uuid"
	"github.com/wailsapp/wails/v3/pkg/application"
	"meron/internal/desktopruntime"
)

// Mutations in one webview invalidate the other readers after the store write.
var windowMailMutations = map[string]bool{
	"mail.send": true, "mail.saveDraft": true, "mail.discardDraft": true,
	"mail.markRead": true, "mail.markStarred": true, "mail.setTags": true,
	"mail.move": true, "mail.copy": true, "mail.archive": true, "mail.delete": true,
}

type threadWindow struct {
	ready        bool
	startupError string
	composeWindow
	threadID        string
	reference       any
	view            any
	drafts          map[string]bool
	identities      map[string]string
	closeInProgress bool
	reopen          map[string]any
}

// Serialise creation just like the tray composer: an existing thread always
// focuses its reader. Snapshots are held only in memory, never in a URL or prefs.
func (a *App) openThreadWindow(payload map[string]any) (any, error) {
	if a.quitting.Load() {
		return nil, fmt.Errorf("application is quitting")
	}
	id, _ := payload["thread_id"].(string)
	if strings.TrimSpace(id) == "" {
		return nil, fmt.Errorf("thread id is required")
	}
	a.composeOpenMu.Lock()
	defer a.composeOpenMu.Unlock()
	a.composeMu.Lock()
	existing := a.threadWindows[id]
	moveTab, _ := payload["move_tab"].(bool)
	if existing != nil && moveTab {
		// A move must not queue a reopen after reporting failure, or discard
		// the source view by merely focusing a reader with a different view.
		if existing.closeInProgress || existing.closing.Load() || !existing.ready {
			a.composeMu.Unlock()
			return nil, fmt.Errorf("thread window is busy; try again when it has finished loading or closing")
		}
		existing.ready = false
		existing.startupError = ""
		existing.view = payload["view"]
	} else if existing != nil && (existing.closeInProgress || existing.closing.Load()) {
		existing.reopen = payload
	}
	a.composeMu.Unlock()
	if existing != nil {
		if moveTab {
			desktopruntime.EventsEmit(a.ctx, "threadWindow.restoreView", map[string]any{"session": existing.session})
		}
		existing.window.Show()
		existing.window.Focus()
		return existing.session, nil
	}
	reader := &threadWindow{composeWindow: composeWindow{session: uuid.NewString()}, threadID: id, reference: payload["thread"], view: payload["view"], drafts: map[string]bool{}, identities: map[string]string{}}
	subject, _ := payload["subject"].(string)
	if subject == "" {
		subject = appTitle()
	} else {
		subject += " — " + appTitle()
	}
	reader.window = newComposeWindow(application.WebviewWindowOptions{
		Name: "thread-" + reader.session, Title: subject,
		URL: "/?threadWindow=" + url.QueryEscape(reader.session), Width: 900, Height: 760,
		MinWidth: 480, MinHeight: 360, Hidden: true, EnableFileDrop: nativeFileDrop || runtime.GOOS == "windows",
	}, func() bool { return a.threadWindowCloseRequested(reader) }, func() {
		a.composeMu.Lock()
		if a.threadWindows[id] == reader {
			delete(a.threadWindows, id)
		}
		reopen := reader.reopen
		a.composeMu.Unlock()
		a.publishThreadDrafts()
		if reopen != nil && !a.quitting.Load() {
			go func() {
				if _, err := a.openThreadWindow(reopen); err != nil {
					a.logf("reopen thread window: %v", err)
				}
			}()
		}
	}, a.noteDroppedPaths)
	a.composeMu.Lock()
	if a.threadWindows == nil {
		a.threadWindows = map[string]*threadWindow{}
	}
	a.threadWindows[id] = reader
	a.composeMu.Unlock()
	reader.window.Show()
	reader.window.Focus()
	return reader.session, nil
}

func (a *App) threadDrafts() []string {
	a.composeMu.Lock()
	defer a.composeMu.Unlock()
	ids := []string{}
	for _, reader := range a.threadWindows {
		for id := range reader.drafts {
			ids = append(ids, id)
			if allocated := reader.identities[id]; allocated != "" {
				ids = append(ids, allocated)
			}
		}
	}
	return ids
}
func (a *App) publishThreadDrafts() {
	desktopruntime.EventsEmit(a.ctx, "threadWindow.draftsChanged", a.threadDrafts())
}

func (a *App) threadWindowCommand(command string, payload map[string]any) (any, error) {
	switch command {
	case "threadWindow.open":
		return a.openThreadWindow(payload)
	case "threadWindow.sessions":
		a.composeMu.Lock()
		sessions := []string{}
		for _, reader := range a.threadWindows {
			sessions = append(sessions, reader.session)
		}
		a.composeMu.Unlock()
		return sessions, nil
	case "threadWindow.setSetting", "threadWindow.setRemoteSender":
		desktopruntime.EventsEmit(a.ctx, command, payload)
		return true, nil
	case "threadWindow.drafts":
		return a.threadDrafts(), nil
	case "threadWindow.discardDraft":
		desktopruntime.EventsEmit(a.ctx, "composer.discardDraft", payload)
		return true, nil
	case "threadWindow.openContact", "threadWindow.openCorrespondent":
		a.showMainWindow()
		desktopruntime.EventsEmit(a.ctx, command, payload)
		return true, nil
	case "threadWindow.openDraft":
		a.showMainWindow()
		desktopruntime.EventsEmit(a.ctx, "threadWindow.openDraft", payload["draft"])
		return true, nil
	case "threadWindow.focusDraft":
		id, _ := payload["draft_id"].(string)
		a.composeMu.Lock()
		var target emlReaderWindow
		var targetSession, previousID string
		for _, reader := range a.threadWindows {
			if reader.ownsDraft(id) {
				target = reader.window
				targetSession = reader.session
				for previous, allocated := range reader.identities {
					if allocated == normalizeComposeDraftID(id) {
						previousID = previous
						break
					}
				}
				break
			}
		}
		a.composeMu.Unlock()
		if target != nil {
			desktopruntime.EventsEmit(a.ctx, "threadWindow.focusDraft", map[string]any{"session": targetSession, "draft_id": id, "previous_id": previousID})
			target.Show()
			target.Focus()
			return true, nil
		}
		return false, nil
	}
	session, _ := payload["session"].(string)
	a.composeMu.Lock()
	var reader *threadWindow
	for _, candidate := range a.threadWindows {
		if candidate.session == session {
			reader = candidate
			break
		}
	}
	if reader == nil {
		a.composeMu.Unlock()
		return nil, fmt.Errorf("thread window is closed")
	}
	switch command {
	case "threadWindow.setReady":
		reader.ready, _ = payload["ready"].(bool)
		reader.startupError, _ = payload["error"].(string)
		a.composeMu.Unlock()
	case "threadWindow.isReady":
		ready, startupError := reader.ready, reader.startupError
		closing := reader.closeInProgress || reader.closing.Load()
		a.composeMu.Unlock()
		if closing {
			return nil, fmt.Errorf("thread window is closing")
		}
		if startupError != "" {
			return nil, fmt.Errorf("%s", startupError)
		}
		return ready, nil
	case "threadWindow.beginClose":
		reader.closeInProgress = true
		a.composeMu.Unlock()
	case "threadWindow.cancelClose":
		reader.closeInProgress = false
		reader.reopen = nil
		a.composeMu.Unlock()
	case "threadWindow.get":
		result := map[string]any{"thread_id": reader.threadID, "thread": reader.reference, "view": reader.view}
		a.composeMu.Unlock()
		return result, nil
	case "threadWindow.reserveDraft":
		previous, _ := payload["previous_id"].(string)
		id, _ := payload["draft_id"].(string)
		if previous != "" && id != "" {
			previous = normalizeComposeDraftID(previous)
			reader.drafts[previous] = true
			if allocated := normalizeComposeDraftID(id); allocated != previous {
				reader.identities[previous] = allocated
			}
		}
		a.composeMu.Unlock()
		a.publishThreadDrafts()
	case "threadWindow.setDrafts":
		reader.drafts = map[string]bool{}
		if ids, ok := payload["draft_ids"].([]any); ok {
			for _, raw := range ids {
				if id, ok := raw.(string); ok && id != "" {
					reader.drafts[normalizeComposeDraftID(id)] = true
				}
			}
		}
		for previous := range reader.identities {
			if !reader.drafts[previous] {
				delete(reader.identities, previous)
			}
		}
		a.composeMu.Unlock()
		a.publishThreadDrafts()
	case "threadWindow.closeResponded":
		request, _ := payload["request"].(float64)
		if uint64(request) == reader.closeRequest {
			reader.closeAcknowledged = true
		}
		a.composeMu.Unlock()
	case "threadWindow.close":
		reader.closing.Store(true)
		a.composeMu.Unlock()
		reader.window.Close()
	default:
		a.composeMu.Unlock()
		return nil, fmt.Errorf("unknown thread window command: %s", command)
	}
	return true, nil
}

// Caller holds composeMu.
func (reader *threadWindow) ownsDraft(raw string) bool {
	id := normalizeComposeDraftID(raw)
	if reader.drafts[id] {
		return true
	}
	for previous, allocated := range reader.identities {
		if reader.drafts[previous] && allocated == id {
			return true
		}
	}
	return false
}

func (a *App) hasThreadWindows() bool {
	a.composeMu.Lock()
	defer a.composeMu.Unlock()
	return len(a.threadWindows) > 0
}

func windowMailChangeDetail(payload map[string]any) map[string]any {
	detail := map[string]any{}
	for _, key := range []string{"thread_id", "account_id", "target_account_id", "target_folder_id", "message_ids", "starred"} {
		if value, ok := payload[key]; ok {
			detail[key] = value
		}
	}
	return detail
}

func (a *App) threadWindowCloseRequested(reader *threadWindow) bool {
	a.composeMu.Lock()
	reader.closeInProgress = true
	a.composeMu.Unlock()
	return a.composeWindowCloseRequested(&reader.composeWindow)
}
