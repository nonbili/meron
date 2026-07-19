package main

import (
	"testing"
)

// threadsByID indexes the threadsJSON result by thread id for assertions.
func threadsByID(t *testing.T, out any) map[string]Message {
	t.Helper()
	obj, ok := out.(map[string]any)
	if !ok {
		t.Fatalf("threadsJSON returned %T, want map[string]any", out)
	}
	list, ok := obj["threads"].([]Message)
	if !ok {
		t.Fatalf("threads field is %T, want []Message", obj["threads"])
	}
	byID := make(map[string]Message, len(list))
	for _, m := range list {
		byID[m.ID] = m
	}
	return byID
}

// Grouping semantics (subject branching, root titles, unread aggregation) live
// in the core (store::group_thread_cards, covered by the Rust tests); the
// bridge only maps ready cards into Messages and mints thread ids.
func TestThreadsJSONMapsCardsAndMintsIDs(t *testing.T) {
	raw := map[string]any{
		"cards": []any{
			map[string]any{
				"thread_key":          "k1#Budget split",
				"original_thread_key": "k1#Quarterly report",
				"folder":              "INBOX",
				"from_name":           "Ann",
				"from_addr":           "ann@example.com",
				"subject":             "Quarterly report",
				"date":                float64(200),
				"unread":              true,
				"unread_count":        float64(2),
				"starred":             true,
				"has_draft":           true,
				"recipient_overflow":  float64(1),
			},
			map[string]any{
				"thread_key": "gmthrid:123",
				"subject":    "Atomic",
			},
		},
	}

	byID := threadsByID(t, threadsJSON("acc", "INBOX", raw))
	if len(byID) != 2 {
		t.Fatalf("got %d threads, want 2: %#v", len(byID), byID)
	}

	branchID := formatImapThreadID("acc", "INBOX", "k1#Budget split")
	branch, ok := byID[branchID]
	if !ok {
		t.Fatalf("missing thread %q in %#v", branchID, byID)
	}
	if branch.OriginalThreadID != formatImapThreadID("acc", "INBOX", "k1#Quarterly report") {
		t.Errorf("OriginalThreadID = %q, want the root card's id", branch.OriginalThreadID)
	}
	if branch.Subject != "Quarterly report" || branch.FromName != "Ann" {
		t.Errorf("card fields not mapped: %#v", branch)
	}
	if !branch.Unread || branch.UnreadCount != 2 || !branch.Starred {
		t.Errorf("flag fields not mapped: %#v", branch)
	}
	if !branch.HasDraft {
		t.Errorf("HasDraft = false, want true: %#v", branch)
	}

	atomic := byID[formatImapThreadID("acc", "INBOX", "gmthrid:123")]
	if atomic.OriginalThreadID != "" {
		t.Errorf("unbranched card should have empty OriginalThreadID, got %q", atomic.OriginalThreadID)
	}
}

func TestThreadsJSONFallsBackToRequestFolder(t *testing.T) {
	raw := map[string]any{
		"cards": []any{
			map[string]any{"thread_key": "k1", "subject": "Hi"},
		},
	}
	byID := threadsByID(t, threadsJSON("acc", "Sent", raw))
	want := formatImapThreadID("acc", "Sent", "k1")
	if _, ok := byID[want]; !ok {
		t.Fatalf("missing thread %q (request-folder fallback) in %#v", want, byID)
	}
}

func TestThreadsJSONSkipsKeylessCardsAndPassesCursor(t *testing.T) {
	raw := map[string]any{
		"cards": []any{
			map[string]any{"subject": "Dropped"}, // no thread_key
			map[string]any{"thread_key": "k2", "subject": "Kept"},
		},
		"next_cursor":   "cursor-token",
		"folder_unread": float64(3),
	}

	out := threadsJSON("acc", "INBOX", raw)
	byID := threadsByID(t, out)
	if len(byID) != 1 {
		t.Fatalf("got %d threads, want 1 (keyless card dropped)", len(byID))
	}
	if got := out.(map[string]any)["next_cursor"]; got != "cursor-token" {
		t.Errorf("next_cursor = %v, want cursor-token", got)
	}
	if got := out.(map[string]any)["folder_unread"]; got != uint32(3) {
		t.Errorf("folder_unread = %v, want 3", got)
	}
}

func TestThreadsJSONOmitsCursorWhenEmpty(t *testing.T) {
	raw := map[string]any{
		"cards":       []any{map[string]any{"thread_key": "k1", "subject": "Hi"}},
		"next_cursor": "",
	}
	out := threadsJSON("acc", "INBOX", raw).(map[string]any)
	if _, present := out["next_cursor"]; present {
		t.Errorf("next_cursor should be omitted when empty, got %#v", out["next_cursor"])
	}
}

func TestThreadsJSONUsesCoreAccountForUnifiedCards(t *testing.T) {
	raw := map[string]any{
		"cards": []any{
			map[string]any{"account_id": "first@example.com", "folder": "INBOX", "thread_key": "one"},
			map[string]any{"account_id": "second@example.com", "folder": "INBOX", "thread_key": "two"},
		},
		"folder_unreads": map[string]any{"first@example.com": float64(1), "second@example.com": float64(2)},
		"failures":       []any{map[string]any{"account_id": "offline@example.com", "message": "offline"}},
	}
	out := threadsJSON("unified", "INBOX", raw).(map[string]any)
	byID := threadsByID(t, out)
	for account, key := range map[string]string{"first@example.com": "one", "second@example.com": "two"} {
		id := formatImapThreadID(account, "INBOX", key)
		if byID[id].AccountID != account {
			t.Errorf("thread %q AccountID = %q, want %q", id, byID[id].AccountID, account)
		}
	}
	if out["folder_unreads"] == nil || out["failures"] == nil {
		t.Fatalf("unified metadata was dropped: %#v", out)
	}
}
