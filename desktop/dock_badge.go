package main

import (
	"strconv"
	"sync"
)

// Routed through a package var so tests can see which label was set.
var dockSetBadgeLabel = platformSetDockBadgeLabel

// Above this the label reads "9999+", as the side rail's UnreadCountBadge.
const dockBadgeMax = 9999

var dockBadge struct {
	mu    sync.Mutex
	label string
	set   bool
}

// dockBadgeLabel turns the frontend's inbox unread count into the Dock tile's
// badge label: "" (no badge) for zero. ok is false when count isn't a number.
func dockBadgeLabel(count any) (label string, ok bool) {
	var n float64
	switch value := count.(type) {
	case float64:
		n = value
	case int:
		n = float64(value)
	default:
		return "", false
	}
	if !(n >= 1) { // also rejects NaN
		return "", true
	}
	if n > dockBadgeMax {
		return strconv.Itoa(dockBadgeMax) + "+", true
	}
	return strconv.Itoa(int(n)), true
}

// setDockBadge shows count on the app icon in the macOS Dock (a no-op
// elsewhere). The frontend repeats the count on every folder refresh; AppKit
// keeps the label, so only changes are passed on.
func setDockBadge(count any) {
	label, ok := dockBadgeLabel(count)
	if !ok {
		return
	}
	dockBadge.mu.Lock()
	defer dockBadge.mu.Unlock()
	if dockBadge.set && dockBadge.label == label {
		return
	}
	dockBadge.label, dockBadge.set = label, true
	dockSetBadgeLabel(label)
}
