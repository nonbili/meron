package main

import (
	"runtime"
	"testing"

	"github.com/wailsapp/wails/v3/pkg/application"
)

func TestTrayComposerReusesWindowAndInterceptsClose(t *testing.T) {
	oldFactory := newComposeWindow
	defer func() { newComposeWindow = oldFactory }()
	var requested func() bool
	var windows []*fakeEmlWindow
	newComposeWindow = func(options application.WebviewWindowOptions, closeRequested func() bool, closed func(), dropped func([]string)) emlReaderWindow {
		if options.URL != "/?compose=standalone" || !options.Hidden || options.EnableFileDrop != (nativeFileDrop || runtime.GOOS == "windows") {
			t.Fatalf("unexpected composer options: %+v", options)
		}
		requested = closeRequested
		window := &fakeEmlWindow{onClose: closed}
		windows = append(windows, window)
		return window
	}
	app := &App{}
	app.windowHidden.Store(true)
	app.openComposeWindow()
	app.openComposeWindow()
	if len(windows) != 1 || windows[0].shown != 2 || windows[0].focused != 2 || !app.windowHidden.Load() {
		t.Fatal("tray action did not reuse the composer independently of the inbox")
	}
	if !requested() || windows[0].closed != 0 || app.composeWindow == nil {
		t.Fatal("native close must wait for the frontend's final save")
	}
	_, _ = app.composeWindowCommand("composer.setDraft", map[string]any{"draft_id": "<draft@example.com>"})
	if id, _ := app.composeWindowCommand("composer.getDraft", nil); id != "<draft@example.com>" {
		t.Fatalf("wrong draft owner: %v", id)
	}
	if focused, _ := app.composeWindowCommand("composer.focusDraft", map[string]any{"draft_id": "draft@example.com"}); focused != true {
		t.Fatal("opening the draft should focus its standalone editor")
	}
	if focused, _ := app.composeWindowCommand("composer.focusDraft", map[string]any{"draft_id": "another@example.com"}); focused != false {
		t.Fatal("focused a different draft")
	}
	_, _ = app.composeWindowCommand("composer.closeWindow", nil)
	if requested() || windows[0].closed != 1 || app.composeWindow != nil || !app.windowHidden.Load() {
		t.Fatal("saved composer did not close independently of the inbox")
	}
	if id, _ := app.composeWindowCommand("composer.getDraft", nil); id != "" {
		t.Fatal("closed window retained draft ownership")
	}
	app.openComposeWindow()
	if len(windows) != 2 {
		t.Fatal("could not start another message after closing")
	}
	app.quitting.Store(true)
	if requested() {
		t.Fatal("composer prevented application shutdown")
	}
}

func TestComposerCloseFallbackPreservesAcknowledgedSaves(t *testing.T) {
	app := &App{}
	composer := &composeWindow{}
	app.composeWindow = composer
	if !app.composeWindowCloseRequested(composer) {
		t.Fatal("first close must allow the page to save")
	}
	composer.closeRequestedAt = composer.closeRequestedAt.Add(-composeCloseResponseTimeout)
	_, err := app.composeWindowCommand("composer.closeResponded", map[string]any{"request": float64(composer.closeRequest)})
	if err != nil {
		t.Fatal(err)
	}
	if !app.composeWindowCloseRequested(composer) || composer.closing.Load() {
		t.Fatal("a slow acknowledged save must not be forcibly closed")
	}
	// The next request went unanswered (e.g. the page crashed). A rapid second
	// click still waits, but a later attempt lets the user dismiss the window.
	if !app.composeWindowCloseRequested(composer) {
		t.Fatal("fallback did not allow time for a response")
	}
	composer.closeRequestedAt = composer.closeRequestedAt.Add(-composeCloseResponseTimeout)
	if app.composeWindowCloseRequested(composer) || !composer.closing.Load() {
		t.Fatal("an unresponsive page trapped the window open")
	}
}

func TestComposerRecoveryCannotClaimAnInboxDraft(t *testing.T) {
	app := &App{composeWindow: &composeWindow{}}
	_, err := app.composeWindowCommand("composer.setMainDrafts", map[string]any{"draft_ids": []any{"<MAIN@example.com>"}})
	if err != nil {
		t.Fatal(err)
	}
	claimed, err := app.composeWindowCommand("composer.claimDraft", map[string]any{"draft_id": "main@example.com"})
	if err != nil || claimed != false || app.composeWindow.draftID != "" {
		t.Fatal("recovery stole the inbox's draft")
	}
	claimed, err = app.composeWindowCommand("composer.claimDraft", map[string]any{"draft_id": "another@example.com"})
	if err != nil || claimed != true || app.composeWindow.draftID != "another@example.com" {
		t.Fatal("could not claim an unowned recovery draft")
	}
}
