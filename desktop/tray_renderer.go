package main

import (
	"sync"
	"time"
)

type trayIconState struct{ unread, monochrome bool }
type trayTimer interface{ Stop() bool }
type trayWakeup struct{ timer trayTimer }

// desired always records the latest request, including during an in-flight
// write or retry backoff. applied is the last native write (possibly a
// fallback); retryAt records whether that write still needs repair.
type trayRenderer struct {
	mu         sync.Mutex
	desired    trayIconState
	applied    *trayIconState
	retryAt    time.Time
	retryDelay time.Duration
	refresh    bool
	done       chan struct{}
	timer      *trayWakeup
	closed     bool

	apply       func(trayIconState) error
	needsRepair func() bool
	now         func() time.Time
	after       func(time.Duration, func()) trayTimer
}

const trayHealthInterval = 30 * time.Second

func (r *trayRenderer) initialize() {
	if r.now == nil {
		r.now = time.Now
	}
	if r.after == nil {
		r.after = func(delay time.Duration, fn func()) trayTimer { return time.AfterFunc(delay, fn) }
	}
}

func (r *trayRenderer) state() trayIconState {
	r.mu.Lock()
	defer r.mu.Unlock()
	return r.desired
}

func (r *trayRenderer) setUnread(unread bool) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.desired.unread = unread
	r.kickLocked()
}

func (r *trayRenderer) setMonochrome(monochrome bool) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.desired.monochrome = monochrome
	r.kickLocked()
}

func (r *trayRenderer) request(force bool) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.refresh = r.refresh || force
	r.kickLocked()
}

func (r *trayRenderer) kickLocked() {
	if r.closed || r.done != nil {
		return
	}
	changed := r.applied == nil || *r.applied != r.desired
	if !changed && !r.retryAt.IsZero() && r.now().Before(r.retryAt) {
		return
	}
	retryDue := !r.retryAt.IsZero() && !r.now().Before(r.retryAt)
	if !changed && !r.refresh && !retryDue {
		return
	}
	r.cancelTimerLocked()
	r.done = make(chan struct{})
	go r.drain()
}

func (r *trayRenderer) drain() {
	for {
		r.mu.Lock()
		if r.closed {
			r.finishLocked()
			r.mu.Unlock()
			return
		}
		state := r.desired
		r.refresh = false
		r.mu.Unlock()

		err := r.apply(state)

		r.mu.Lock()
		r.applied = &state
		if err != nil {
			if r.retryDelay == 0 {
				r.retryDelay = 5 * time.Second
			} else {
				r.retryDelay = min(r.retryDelay*2, time.Minute)
			}
			r.retryAt = r.now().Add(r.retryDelay)
		} else {
			r.retryDelay, r.retryAt = 0, time.Time{}
		}
		// Always compare to the write that just completed. A request returning
		// to an older state while this write was in flight cannot be dropped.
		if !r.closed && (r.desired != state || (r.refresh && err == nil)) {
			r.mu.Unlock()
			continue
		}
		if !r.closed {
			delay := trayHealthInterval
			if !r.retryAt.IsZero() {
				delay = r.retryAt.Sub(r.now())
			}
			r.armLocked(delay)
		}
		r.finishLocked()
		r.mu.Unlock()
		return
	}
}

func (r *trayRenderer) finishLocked() { close(r.done); r.done = nil }
func (r *trayRenderer) cancelTimerLocked() {
	if r.timer != nil {
		r.timer.timer.Stop()
		r.timer = nil
	}
}

func (r *trayRenderer) armLocked(delay time.Duration) {
	r.cancelTimerLocked()
	// Capture an immutable identity before scheduling: a zero-delay callback
	// may start before after returns its timer handle.
	wakeup := &trayWakeup{}
	r.timer = wakeup
	wakeup.timer = r.after(max(delay, 0), func() { r.onTimer(wakeup) })
}

func (r *trayRenderer) onTimer(timer *trayWakeup) {
	r.mu.Lock()
	if r.closed || r.timer != timer {
		r.mu.Unlock()
		return
	}
	r.timer = nil
	if !r.retryAt.IsZero() {
		r.kickLocked()
		if r.done == nil {
			r.armLocked(r.retryAt.Sub(r.now()))
		}
		r.mu.Unlock()
		return
	}
	r.mu.Unlock()

	// Cache health is checked on this timer, never on the bridge goroutine.
	repair := r.needsRepair()
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.closed || r.done != nil || r.timer != nil {
		return
	}
	r.refresh = r.refresh || repair
	r.kickLocked()
	if r.done == nil {
		r.armLocked(trayHealthInterval)
	}
}

func (r *trayRenderer) wait() {
	r.mu.Lock()
	done := r.done
	r.mu.Unlock()
	if done != nil {
		<-done
	}
}

func (r *trayRenderer) stop() {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.closed = true
	r.cancelTimerLocked()
}

func (a *App) trayState() *trayRenderer {
	a.trayInit.Do(func() {
		apply := trayApplyPlatformIcon
		a.tray.apply = func(state trayIconState) error {
			err := apply(state.unread, state.monochrome)
			if err != nil {
				a.logf("tray: icon files unavailable, using the fallback pixmap: %v", err)
			}
			return err
		}
		a.tray.needsRepair = trayIconNeedsRepair
		a.tray.initialize()
	})
	return &a.tray
}
