package main

import (
	"reflect"
	"testing"
)

func stubDockBadge(t *testing.T) *[]string {
	t.Helper()
	var labels []string
	previous := dockSetBadgeLabel
	dockSetBadgeLabel = func(label string) { labels = append(labels, label) }
	reset := func() {
		dockBadge.mu.Lock()
		dockBadge.label, dockBadge.set = "", false
		dockBadge.mu.Unlock()
	}
	reset()
	t.Cleanup(func() {
		dockSetBadgeLabel = previous
		reset()
	})
	return &labels
}

func TestTraySetUnreadShowsTheInboxCountOnTheDockIcon(t *testing.T) {
	stubTrayRenderer(t, func(bool, bool) error { return nil })
	labels := stubDockBadge(t)
	a := &App{}

	for _, count := range []float64{3, 3, 12, 0, 0} {
		if _, err := a.traySetUnread(map[string]any{"unread": count > 0, "count": count}); err != nil {
			t.Fatal(err)
		}
	}

	// Repeated counts reach AppKit once; zero clears the badge.
	if want := []string{"3", "12", ""}; !reflect.DeepEqual(*labels, want) {
		t.Fatalf("dock labels = %q, want %q", *labels, want)
	}
}

func TestTraySetUnreadWithoutACountLeavesTheDockBadgeAlone(t *testing.T) {
	stubTrayRenderer(t, func(bool, bool) error { return nil })
	labels := stubDockBadge(t)
	a := &App{}

	if _, err := a.traySetUnread(map[string]any{"unread": true}); err != nil {
		t.Fatal(err)
	}
	if len(*labels) != 0 {
		t.Fatalf("dock labels = %q, want none", *labels)
	}
}

func TestDockBadgeLabelRejectsNonsenseCounts(t *testing.T) {
	for _, tc := range []struct {
		count any
		want  string
	}{
		{float64(1), "1"},
		{float64(-4), ""},
		{float64(2.7), "2"},
		{"7", ""},
		{float64(9999), "9999"},
		{float64(10000), "9999+"},
		{float64(1e12), "9999+"},
	} {
		label, ok := dockBadgeLabel(tc.count)
		if tc.count == "7" {
			if ok {
				t.Fatalf("dockBadgeLabel(%q) accepted a string", tc.count)
			}
			continue
		}
		if !ok || label != tc.want {
			t.Fatalf("dockBadgeLabel(%v) = %q, %v; want %q", tc.count, label, ok, tc.want)
		}
	}
}
