package main

import (
	"errors"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

func stubTrayRenderer(t *testing.T, render func(bool, bool) error) {
	t.Helper()
	previous, needsRepair := trayApplyPlatformIcon, trayIconNeedsRepair
	trayApplyPlatformIcon, trayIconNeedsRepair = render, func() bool { return false }
	t.Cleanup(func() { trayApplyPlatformIcon, trayIconNeedsRepair = previous, needsRepair })
}

type fakeTrayTimer struct {
	fn      func()
	at      time.Time
	stopped bool
	clock   *fakeTrayClock
}

func (t *fakeTrayTimer) Stop() bool {
	t.clock.mu.Lock()
	defer t.clock.mu.Unlock()
	was := !t.stopped
	t.stopped = true
	return was
}

type fakeTrayClock struct {
	mu      sync.Mutex
	current time.Time
	timers  []*fakeTrayTimer
}

func configureTrayClock(r *trayRenderer) *fakeTrayClock {
	c := &fakeTrayClock{current: time.Unix(100, 0)}
	r.now = func() time.Time { c.mu.Lock(); defer c.mu.Unlock(); return c.current }
	r.after = func(d time.Duration, fn func()) trayTimer {
		c.mu.Lock()
		defer c.mu.Unlock()
		timer := &fakeTrayTimer{fn: fn, at: c.current.Add(d), clock: c}
		c.timers = append(c.timers, timer)
		return timer
	}
	return c
}
func (c *fakeTrayClock) advance(d time.Duration) {
	c.mu.Lock()
	c.current = c.current.Add(d)
	var due []func()
	for _, timer := range c.timers {
		if !timer.stopped && !c.current.Before(timer.at) {
			timer.stopped = true
			due = append(due, timer.fn)
		}
	}
	c.mu.Unlock()
	for _, fn := range due {
		fn()
	}
}

func TestTrayRendererCoalescesWithoutBlockingStateChanges(t *testing.T) {
	app := &App{tray: trayRenderer{desired: trayIconState{monochrome: true}}}
	stubTrayRenderer(t, func(bool, bool) error { return nil })
	t.Cleanup(func() { app.stopTray(); app.waitForTrayIcon() })
	entered, release := make(chan struct{}), make(chan struct{})
	var calls, active atomic.Int32
	var last trayIconState
	trayApplyPlatformIcon = func(unread, mono bool) error {
		if active.Add(1) != 1 {
			t.Error("native writes overlapped")
		}
		defer active.Add(-1)
		if calls.Add(1) == 1 {
			close(entered)
			<-release
		}
		last = trayIconState{unread, mono}
		return nil
	}
	app.setTrayUnread(true)
	<-entered
	if !app.tray.mu.TryLock() {
		close(release)
		app.waitForTrayIcon()
		t.Fatal("native work holds the state mutex")
	}
	app.tray.mu.Unlock()
	_, _ = app.traySetStyle(map[string]any{"monochrome": false})
	app.setTrayUnread(false)
	app.setTrayUnread(true)
	close(release)
	app.waitForTrayIcon()
	if last != (trayIconState{true, false}) || calls.Load() != 2 {
		t.Fatalf("last=%+v calls=%d", last, calls.Load())
	}
	app.setTrayUnread(true)
	app.waitForTrayIcon()
	if calls.Load() != 2 {
		t.Fatal("unchanged state rewrote the icon")
	}
}

func TestTrayRendererSchedulesRetryWithoutMailboxActivity(t *testing.T) {
	app := &App{}
	clock := configureTrayClock(&app.tray)
	var calls atomic.Int32
	stubTrayRenderer(t, func(bool, bool) error {
		if calls.Add(1) == 1 {
			return errors.New("temporary failure")
		}
		return nil
	})
	t.Cleanup(func() { app.stopTray(); app.waitForTrayIcon() })
	app.setTrayUnread(true)
	app.waitForTrayIcon()
	for range 10 {
		app.setTrayUnread(true)
	}
	if calls.Load() != 1 {
		t.Fatal("refreshes ignored backoff")
	}
	clock.advance(5 * time.Second)
	app.waitForTrayIcon()
	if calls.Load() != 2 {
		t.Fatal("idle mailbox prevented automatic retry")
	}
	clock.advance(trayHealthInterval)
	app.waitForTrayIcon()
	if calls.Load() != 2 {
		t.Fatal("healthy idle tray was rewritten")
	}
	app.stopTray()
	clock.advance(time.Hour)
	if calls.Load() != 2 {
		t.Fatal("stopped renderer retried")
	}
}

func TestTrayRendererKeepsReturnToFailedStateDuringInflightWrite(t *testing.T) {
	app := &App{}
	configureTrayClock(&app.tray)
	var calls atomic.Int32
	var last bool
	entered, release := make(chan struct{}), make(chan struct{})
	stubTrayRenderer(t, func(unread, _ bool) error {
		call := calls.Add(1)
		if call == 1 {
			return errors.New("initial write failed")
		}
		if call == 2 {
			close(entered)
			<-release
		}
		last = unread
		return nil
	})
	t.Cleanup(func() { app.stopTray(); app.waitForTrayIcon() })
	app.setTrayUnread(false)
	app.waitForTrayIcon()
	app.setTrayUnread(true)
	<-entered
	app.setTrayUnread(false)
	close(release)
	app.waitForTrayIcon()
	if last || calls.Load() != 3 {
		t.Fatalf("return to read state was dropped: unread=%v calls=%d", last, calls.Load())
	}
}

func TestTrayUnreadBridgeDoesNotStatIconFiles(t *testing.T) {
	app := &App{}
	var checks atomic.Int32
	stubTrayRenderer(t, func(bool, bool) error { return nil })
	trayIconNeedsRepair = func() bool { checks.Add(1); return false }
	t.Cleanup(func() { app.stopTray(); app.waitForTrayIcon() })
	app.setTrayUnread(true)
	app.waitForTrayIcon()
	for range 20 {
		app.setTrayUnread(true)
	}
	if checks.Load() != 0 {
		t.Fatal("unchanged bridge calls checked the filesystem")
	}
}
