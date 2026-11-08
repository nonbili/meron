package main

import (
	"fmt"
	"maps"
	"net/url"
	"runtime"
	"strings"
	"time"

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
	ready            bool
	startupError     string
	composeTab       map[string]any
	composeActivated bool
	composeRevision  float64
	composeAliases   map[string]string
	composeTimer     *time.Timer
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
	composeTab, composerOnly := payload["compose_tab"].(map[string]any)
	if composerOnly {
		draft, ok := composeTab["compose"].(map[string]any)
		tabID, _ := composeTab["id"].(string)
		draftID, _ := draft["draftMessageId"].(string)
		if !ok || composeTab["kind"] != "compose" || tabID == "" || draftID == "" {
			return nil, fmt.Errorf("invalid composer tab")
		}
		id = "composer:" + uuid.NewString()
	}
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
	if composerOnly {
		reader.threadID = ""
		reader.composeTab = composeTab
	}

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
		if reader.composeTimer != nil {
			reader.composeTimer.Stop()
		}
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
	if composerOnly {
		reader.composeTimer = time.AfterFunc(60*time.Second, func() { a.expireStagedComposer(reader) })
	}
	a.composeMu.Unlock()
	if !composerOnly {
		reader.window.Show()
		reader.window.Focus()
	}
	return reader.session, nil
}

// A source webview may disappear before it can activate or abort its staged
// destination. Expire that hidden window without relying on frontend timers.
func (a *App) expireStagedComposer(reader *threadWindow) {
	a.composeMu.Lock()
	if reader.composeActivated || reader.closing.Load() {
		a.composeMu.Unlock()
		return
	}
	reader.closing.Store(true)
	if a.expiredComposerSessions == nil {
		a.expiredComposerSessions = map[string]bool{}
	}
	// Keep only bounded, content-free receipts, so a delayed source can safely
	// resume after expiry without confusing an activated editor with an abort.
	if len(a.expiredComposerSessions) >= 64 {
		for session := range a.expiredComposerSessions {
			delete(a.expiredComposerSessions, session)
			break
		}
	}
	a.expiredComposerSessions[reader.session] = true
	a.composeMu.Unlock()
	reader.window.Close()
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
		if command == "threadWindow.abortComposer" && a.expiredComposerSessions[session] {
			delete(a.expiredComposerSessions, session)
			a.composeMu.Unlock()
			return true, nil
		}
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
	case "threadWindow.updateComposer":
		tab, ok := payload["tab"].(map[string]any)
		revision, _ := payload["revision"].(float64)
		if !ok || reader.composeTab == nil || !reader.composeActivated || tab["id"] != reader.composeTab["id"] {
			a.composeMu.Unlock()
			return nil, fmt.Errorf("composer window does not own this tab")
		}
		if revision <= reader.composeRevision {
			result := map[string]any{"applied": false, "revision": reader.composeRevision}
			a.composeMu.Unlock()
			return result, nil
		}
		draft, ok := tab["compose"].(map[string]any)
		if !ok {
			a.composeMu.Unlock()
			return nil, fmt.Errorf("composer draft is missing")
		}
		previous := reader.composeTab["compose"].(map[string]any)
		oldFiles := map[string]any{}
		if files, ok := previous["attachments"].([]any); ok {
			for _, raw := range files {
				if file, ok := raw.(map[string]any); ok {
					id, _ := file["id"].(string)
					oldFiles[id] = file["data"]
				}
			}
		}
		data, _ := payload["attachment_data"].(map[string]any)
		files, _ := draft["attachments"].([]any)
		for _, raw := range files {
			file, ok := raw.(map[string]any)
			if !ok {
				a.composeMu.Unlock()
				return nil, fmt.Errorf("invalid composer attachment")
			}
			id, _ := file["id"].(string)
			bytes, exists := data[id]
			if !exists {
				bytes, exists = oldFiles[id]
			}
			if !exists {
				a.composeMu.Unlock()
				return nil, fmt.Errorf("composer attachment data is missing")
			}
			file["data"] = bytes
		}
		// Resolve only actual allocations, so a newly chosen identity in the
		// same account is never mistaken for an old placeholder arriving late.
		id, _ := draft["draftMessageId"].(string)
		if allocated := reader.composeAliases[normalizeComposeDraftID(id)]; allocated != "" {
			draft["draftMessageId"] = allocated
		}
		reader.composeTab = tab
		reader.composeRevision = revision
		a.composeMu.Unlock()
		return map[string]any{"applied": true, "revision": revision}, nil
	case "threadWindow.activateComposer":
		if reader.composeTab == nil || !reader.ready || reader.closeInProgress || reader.closing.Load() {
			a.composeMu.Unlock()
			return nil, fmt.Errorf("composer window is not ready")
		}
		reader.composeActivated = true
		if reader.composeTimer != nil {
			reader.composeTimer.Stop()
		}
		draft := reader.composeTab["compose"].(map[string]any)
		id, _ := draft["draftMessageId"].(string)
		reader.drafts[normalizeComposeDraftID(id)] = true
		a.composeMu.Unlock()
		a.publishThreadDrafts()
		desktopruntime.EventsEmit(a.ctx, "threadWindow.activateComposer", map[string]any{"session": session})
		reader.window.Show()
		reader.window.Focus()
	case "threadWindow.abortComposer":
		if a.expiredComposerSessions[session] {
			delete(a.expiredComposerSessions, session)
			a.composeMu.Unlock()
			return true, nil
		}
		if reader.composeTab == nil || reader.composeActivated {
			a.composeMu.Unlock()
			return false, nil
		}
		reader.closing.Store(true)
		a.composeMu.Unlock()
		reader.window.Close()
	case "threadWindow.beginClose":
		reader.closeInProgress = true
		a.composeMu.Unlock()
	case "threadWindow.cancelClose":
		reader.closeInProgress = false
		reader.reopen = nil
		a.composeMu.Unlock()
	case "threadWindow.get":
		result := map[string]any{"thread_id": reader.threadID, "thread": reader.reference, "view": reader.view, "compose_tab": reader.composeTab, "compose_activated": reader.composeActivated, "compose_revision": reader.composeRevision}
		a.composeMu.Unlock()
		return result, nil
	case "threadWindow.reserveDraft":
		previous, _ := payload["previous_id"].(string)
		id, _ := payload["draft_id"].(string)
		if previous != "" && id != "" {
			if reader.composeTab != nil {
				if reader.composeAliases == nil {
					reader.composeAliases = map[string]string{}
				}
				reader.composeAliases[normalizeComposeDraftID(previous)] = id
				draft := reader.composeTab["compose"].(map[string]any)
				currentID, _ := draft["draftMessageId"].(string)
				if normalizeComposeDraftID(currentID) == normalizeComposeDraftID(previous) {
					// Returned snapshots may still be serializing in another goroutine.
					draft = maps.Clone(draft)
					draft["draftMessageId"] = id
					reader.composeTab = maps.Clone(reader.composeTab)
					reader.composeTab["compose"] = draft
				}
			}
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
