package main

import (
	"bytes"
	"image/png"
	"math"
	"reflect"
	"testing"
)

func stubTaskbarBadge(t *testing.T) *[]string {
	t.Helper()
	var labels []string
	previous := taskbarSetBadgeLabel
	taskbarSetBadgeLabel = func(label string) { labels = append(labels, label) }
	reset := func() {
		taskbarBadge.mu.Lock()
		taskbarBadge.label, taskbarBadge.set = "", false
		taskbarBadge.mu.Unlock()
	}
	reset()
	t.Cleanup(func() {
		taskbarSetBadgeLabel = previous
		reset()
	})
	return &labels
}

func TestTraySetUnreadShowsAndClearsTheTaskbarBadge(t *testing.T) {
	stubTrayRenderer(t, func(bool, bool) error { return nil })
	stubDockBadge(t)
	labels := stubTaskbarBadge(t)
	a := &App{}

	for _, payload := range []map[string]any{
		{"unread": true, "count": float64(3), "taskbarBadge": true},
		{"unread": true, "count": float64(3), "taskbarBadge": true},
		{"unread": true, "count": float64(120), "taskbarBadge": true},
		{"unread": true, "count": float64(120), "taskbarBadge": false},
		{"unread": true, "count": float64(5), "taskbarBadge": false},
		{"unread": true, "count": float64(5), "taskbarBadge": true},
		{"unread": false, "count": float64(0), "taskbarBadge": true},
	} {
		if _, err := a.traySetUnread(payload); err != nil {
			t.Fatal(err)
		}
	}

	if want := []string{"3", "99+", "", "5", ""}; !reflect.DeepEqual(*labels, want) {
		t.Fatalf("taskbar labels = %q, want %q", *labels, want)
	}
}

func TestTraySetUnreadWithoutTaskbarBadgeFlagDefaultsToEnabled(t *testing.T) {
	stubTrayRenderer(t, func(bool, bool) error { return nil })
	stubDockBadge(t)
	labels := stubTaskbarBadge(t)
	a := &App{}

	if _, err := a.traySetUnread(map[string]any{"unread": true, "count": float64(7)}); err != nil {
		t.Fatal(err)
	}
	if want := []string{"7"}; !reflect.DeepEqual(*labels, want) {
		t.Fatalf("taskbar labels = %q, want %q", *labels, want)
	}
}

func TestRefreshTaskbarBadgeReappliesNonEmptyLabel(t *testing.T) {
	labels := stubTaskbarBadge(t)

	// Nothing set yet: no-op.
	refreshTaskbarBadge()
	if len(*labels) != 0 {
		t.Fatalf("taskbar labels = %q, want none", *labels)
	}

	setTaskbarBadge(4, true)
	refreshTaskbarBadge()
	setTaskbarBadge(0, true)
	refreshTaskbarBadge()

	if want := []string{"4", "4", ""}; !reflect.DeepEqual(*labels, want) {
		t.Fatalf("taskbar labels = %q, want %q", *labels, want)
	}
}

func TestTaskbarBadgeLabelFormatting(t *testing.T) {
	for _, tc := range []struct {
		count   any
		enabled bool
		want    string
		wantOK  bool
	}{
		{float64(1), true, "1", true},
		{float64(99), true, "99", true},
		{float64(100), true, "99+", true},
		{float64(0), true, "", true},
		{float64(-2), true, "", true},
		{math.NaN(), true, "", true},
		{float64(42), false, "", true},
		{"7", true, "", false},
	} {
		label, ok := taskbarBadgeLabel(tc.count, tc.enabled)
		if ok != tc.wantOK || label != tc.want {
			t.Fatalf("taskbarBadgeLabel(%v, %v) = %q, %v; want %q, %v", tc.count, tc.enabled, label, ok, tc.want, tc.wantOK)
		}
	}
}

func TestRenderTaskbarBadgePNG(t *testing.T) {
	for _, label := range []string{"1", "42", "99+"} {
		data, err := renderTaskbarBadgePNG(label)
		if err != nil {
			t.Fatalf("renderTaskbarBadgePNG(%q): %v", label, err)
		}
		img, err := png.Decode(bytes.NewReader(data))
		if err != nil {
			t.Fatalf("png.Decode(%q): %v", label, err)
		}
		if b := img.Bounds(); b.Dx() != 32 || b.Dy() != 32 {
			t.Fatalf("badge %q bounds = %v, want 32x32", label, b)
		}
		// Must contain both red circle pixels and white glyph pixels.
		var hasRed, hasWhite bool
		for y := 0; y < 32; y++ {
			for x := 0; x < 32; x++ {
				r, g, b, a := img.At(x, y).RGBA()
				if a > 0xF000 && r > 0xD000 && g < 0x5000 && b < 0x5000 {
					hasRed = true
				}
				if a > 0xF000 && r > 0xF000 && g > 0xF000 && b > 0xF000 {
					hasWhite = true
				}
			}
		}
		if !hasRed || !hasWhite {
			t.Fatalf("badge %q missing expected pixels: hasRed=%v hasWhite=%v", label, hasRed, hasWhite)
		}
	}
}
