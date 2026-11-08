package main

import (
	"net/url"
	"runtime"
	"testing"
	"time"

	"github.com/wailsapp/wails/v3/pkg/application"
)

func TestThreadWindowsReuseAndCloseIndependently(t *testing.T) {
	oldFactory := newComposeWindow
	defer func() { newComposeWindow = oldFactory }()
	var windows []*fakeEmlWindow
	var requests []func() bool
	newComposeWindow = func(options application.WebviewWindowOptions, requested func() bool, closed func(), dropped func([]string)) emlReaderWindow {
		route, err := url.Parse(options.URL)
		if err != nil || route.Query().Get("threadWindow") == "" || !options.Hidden || options.EnableFileDrop != (nativeFileDrop || runtime.GOOS == "windows") || dropped == nil {
			t.Fatalf("wrong reader options: %+v", options)
		}
		window := &fakeEmlWindow{onClose: closed}
		windows = append(windows, window)
		requests = append(requests, requested)
		return window
	}
	app := &App{}
	app.windowHidden.Store(true)
	view := map[string]any{"scroll": map[string]any{"messageId": "m", "offset": 30}}
	first, err := app.threadWindowCommand("threadWindow.open", map[string]any{"thread_id": "t/1", "view": view})
	if err != nil {
		t.Fatal(err)
	}
	again, _ := app.threadWindowCommand("threadWindow.open", map[string]any{"thread_id": "t/1"})
	second, _ := app.threadWindowCommand("threadWindow.open", map[string]any{"thread_id": "t/2"})
	if first != again || first == second || len(windows) != 2 || windows[0].focused != 2 {
		t.Fatal("threads did not reuse their own window")
	}
	snapshot, err := app.threadWindowCommand("threadWindow.get", map[string]any{"session": first})
	if err != nil || snapshot.(map[string]any)["view"] == nil {
		t.Fatal("view snapshot was not retained")
	}
	if !requests[0]() || windows[0].closed != 0 {
		t.Fatal("close did not wait for the frontend save")
	}
	reader := app.threadWindows["t/1"]
	_, _ = app.threadWindowCommand("threadWindow.closeResponded", map[string]any{"session": first, "request": float64(reader.closeRequest)})
	reader.closeRequestedAt = time.Now().Add(-2 * composeCloseResponseTimeout)
	if !requests[0]() {
		t.Fatal("acknowledged save was forcibly closed")
	}
	_, err = app.threadWindowCommand("threadWindow.close", map[string]any{"session": first})
	if err != nil || windows[0].closed != 1 || len(app.threadWindows) != 1 || !app.windowHidden.Load() {
		t.Fatal("reader close affected another window")
	}
	if _, err = app.threadWindowCommand("threadWindow.get", map[string]any{"session": first}); err == nil {
		t.Fatal("closed reader retained its snapshot")
	}
	if _, err = app.threadWindowCommand("threadWindow.open", map[string]any{}); err == nil {
		t.Fatal("empty thread id accepted")
	}
}

func TestThreadDraftAllocationRemainsOwnedUntilAdopted(t *testing.T) {
	window := &fakeEmlWindow{}
	reader := &threadWindow{composeWindow: composeWindow{session: "s", window: window}, drafts: map[string]bool{}, identities: map[string]string{}}
	app := &App{threadWindows: map[string]*threadWindow{"t": reader}, composeWindow: &composeWindow{}}
	command := func(name string, payload map[string]any) any {
		t.Helper()
		payload["session"] = "s"
		result, err := app.threadWindowCommand(name, payload)
		if err != nil {
			t.Fatal(err)
		}
		return result
	}
	command("threadWindow.setDrafts", map[string]any{"draft_ids": []any{"local-draft-1"}})
	command("threadWindow.reserveDraft", map[string]any{"previous_id": "local-draft-1", "draft_id": "<NEW@example.com>"})
	// Another edit can publish the still-local identity while its save is running.
	command("threadWindow.setDrafts", map[string]any{"draft_ids": []any{"local-draft-1", "another-draft"}})
	if command("threadWindow.focusDraft", map[string]any{"draft_id": "new@example.com"}) != true || window.focused != 1 {
		t.Fatal("allocated identity lost its editor")
	}
	if claimed, _ := app.composeWindowCommand("composer.claimDraft", map[string]any{"draft_id": "<NEW@example.com>"}); claimed != false {
		t.Fatal("tray composer claimed an open thread draft")
	}
	command("threadWindow.setDrafts", map[string]any{"draft_ids": []any{"new@example.com"}})
	if !reader.ownsDraft("<NEW@example.com>") || len(reader.identities) != 0 {
		t.Fatal("adoption did not retire the allocation alias")
	}
	command("threadWindow.setDrafts", map[string]any{"draft_ids": []any{}})
	if command("threadWindow.focusDraft", map[string]any{"draft_id": "new@example.com"}) != false {
		t.Fatal("closed editor retained ownership")
	}
}

func TestWindowMailChangeOmitsMessageContent(t *testing.T) {
	detail := windowMailChangeDetail(map[string]any{
		"thread_id": "t", "account_id": "a", "starred": true,
		"body": "private text", "html": "private HTML", "attachments": []any{"large attachment"},
	})
	if len(detail) != 3 || detail["thread_id"] != "t" || detail["starred"] != true {
		t.Fatalf("unexpected invalidation: %+v", detail)
	}
}

func TestThreadWindowSessionsOnlyIncludesLiveWindows(t *testing.T) {
	app := &App{threadWindows: map[string]*threadWindow{"t": {composeWindow: composeWindow{session: "live"}}}}
	sessions, err := app.threadWindowCommand("threadWindow.sessions", nil)
	if err != nil || len(sessions.([]string)) != 1 || sessions.([]string)[0] != "live" {
		t.Fatalf("wrong live sessions: %v, %v", sessions, err)
	}
	delete(app.threadWindows, "t")
	sessions, err = app.threadWindowCommand("threadWindow.sessions", nil)
	if err != nil || len(sessions.([]string)) != 0 {
		t.Fatal("closed session was still live")
	}
}

func TestOpeningAClosingThreadQueuesReopen(t *testing.T) {
	oldFactory := newComposeWindow
	defer func() { newComposeWindow = oldFactory }()
	created := make(chan *fakeEmlWindow, 3)
	newComposeWindow = func(_ application.WebviewWindowOptions, _ func() bool, closed func(), _ func([]string)) emlReaderWindow {
		window := &fakeEmlWindow{onClose: closed}
		created <- window
		return window
	}
	app := &App{}
	session, err := app.threadWindowCommand("threadWindow.open", map[string]any{"thread_id": "t"})
	if err != nil {
		t.Fatal(err)
	}
	first := <-created
	_, _ = app.threadWindowCommand("threadWindow.beginClose", map[string]any{"session": session})
	_, _ = app.threadWindowCommand("threadWindow.open", map[string]any{"thread_id": "t"})
	select {
	case <-created:
		t.Fatal("created a duplicate during the final save")
	default:
	}
	_, _ = app.threadWindowCommand("threadWindow.close", map[string]any{"session": session})
	select {
	case <-created:
	case <-time.After(time.Second):
		t.Fatal("queued open request was lost")
	}
	deadline := time.Now().Add(time.Second)
	for {
		app.composeMu.Lock()
		reopened := app.threadWindows["t"] != nil && app.threadWindows["t"].session != session
		app.composeMu.Unlock()
		if reopened {
			break
		}
		if time.Now().After(deadline) {
			t.Fatal("reopened reader was not registered")
		}
		time.Sleep(time.Millisecond)
	}
	if first.closed != 1 {
		t.Fatal("original reader did not close")
	}
}

func TestFailedCloseRetainsTheRequestedWindowWithoutReopening(t *testing.T) {
	oldFactory := newComposeWindow
	defer func() { newComposeWindow = oldFactory }()
	created := 0
	newComposeWindow = func(_ application.WebviewWindowOptions, _ func() bool, closed func(), _ func([]string)) emlReaderWindow {
		created++
		return &fakeEmlWindow{onClose: closed}
	}
	app := &App{}
	session, _ := app.threadWindowCommand("threadWindow.open", map[string]any{"thread_id": "t"})
	_, _ = app.threadWindowCommand("threadWindow.beginClose", map[string]any{"session": session})
	_, _ = app.threadWindowCommand("threadWindow.open", map[string]any{"thread_id": "t"})
	_, _ = app.threadWindowCommand("threadWindow.cancelClose", map[string]any{"session": session})
	if created != 1 || app.threadWindows["t"].reopen != nil || app.threadWindows["t"].closeInProgress {
		t.Fatal("failed close did not retain the existing reader")
	}
}

func TestThreadWindowReadiness(t *testing.T) {
	reader := &threadWindow{composeWindow: composeWindow{session: "s"}}
	app := &App{threadWindows: map[string]*threadWindow{"t": reader}}
	check := func(want bool, wantError bool) {
		t.Helper()
		result, err := app.Invoke("threadWindow.isReady", map[string]any{"session": "s"})
		if (err != nil) != wantError || (!wantError && result != want) {
			t.Fatalf("readiness = %v, %v; want %v, error %v", result, err, want, wantError)
		}
	}
	check(false, false)
	_, err := app.Invoke("threadWindow.setReady", map[string]any{"session": "s", "error": "loading failed"})
	if err != nil {
		t.Fatal(err)
	}
	check(false, true)
	_, err = app.Invoke("threadWindow.setReady", map[string]any{"session": "s", "ready": true})
	if err != nil {
		t.Fatal(err)
	}
	check(true, false)
	reader.closeInProgress = true
	check(false, true)
	reader.closeInProgress = false
	delete(app.threadWindows, "t")
	check(false, true)
}

func TestMovingTabIntoExistingWindowRestoresViewWithoutQueuingReopen(t *testing.T) {
	window := &fakeEmlWindow{}
	reader := &threadWindow{composeWindow: composeWindow{session: "s", window: window}}
	app := &App{threadWindows: map[string]*threadWindow{"t": reader}}
	payload := map[string]any{"thread_id": "t", "move_tab": true, "view": "source view"}
	// A busy reader must leave the source in place without scheduling a later open.
	for _, closing := range []bool{false, true} {
		reader.closeInProgress = closing
		if _, err := app.Invoke("threadWindow.open", payload); err == nil {
			t.Fatal("move accepted a busy window")
		}
		if reader.reopen != nil {
			t.Fatal("failed move queued an unwanted reopen")
		}
	}
	reader.closeInProgress = false
	reader.ready = true
	result, err := app.Invoke("threadWindow.open", payload)
	if err != nil || result != "s" || window.focused != 1 || reader.view != "source view" || reader.ready {
		t.Fatalf("existing reader did not start restoring the moved view: %v, %v, %+v", result, err, reader)
	}
	ready, err := app.Invoke("threadWindow.isReady", map[string]any{"session": "s"})
	if err != nil || ready != false {
		t.Fatalf("source can close before restore: %v, %v", ready, err)
	}
}
