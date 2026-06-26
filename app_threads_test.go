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

func msg(uid int, fields map[string]any) map[string]any {
	m := map[string]any{"uid": float64(uid)}
	for k, v := range fields {
		m[k] = v
	}
	return m
}

func TestThreadsJSONGroupsByThreadKeyAndSubject(t *testing.T) {
	raw := map[string]any{
		"messages": []any{
			msg(10, map[string]any{"thread_key": "k1", "subject": "Launch plan", "from_name": "Ann", "date": float64(100)}),
			msg(11, map[string]any{"thread_key": "k1", "subject": "Re: Launch plan", "date": float64(200)}),
			msg(20, map[string]any{"thread_key": "k2", "subject": "Lunch?", "date": float64(150)}),
		},
	}

	byID := threadsByID(t, threadsJSON("acc", "INBOX", raw))
	if len(byID) != 2 {
		t.Fatalf("got %d threads, want 2: %#v", len(byID), byID)
	}

	// Two messages with the same thread_key + grouping subject collapse to one
	// thread; its title is the oldest (lowest uid) message's normalized subject.
	id1 := formatImapThreadID("acc", "INBOX", "k1#"+threadGroupingSubject("Launch plan"))
	t1, ok := byID[id1]
	if !ok {
		t.Fatalf("missing thread %q in %#v", id1, byID)
	}
	if t1.Subject != "Launch plan" {
		t.Errorf("title = %q, want %q (oldest message's normalized subject)", t1.Subject, "Launch plan")
	}
	if t1.FromName != "Ann" {
		t.Errorf("from_name = %q, want Ann (root message)", t1.FromName)
	}
}

func TestThreadsJSONUnreadAggregation(t *testing.T) {
	raw := map[string]any{
		"messages": []any{
			msg(10, map[string]any{"thread_key": "k1", "subject": "Topic", "seen": false}),
			msg(11, map[string]any{"thread_key": "k1", "subject": "Re: Topic", "seen": false}),
			msg(12, map[string]any{"thread_key": "k1", "subject": "Re: Topic", "seen": true, "starred": true}),
		},
	}

	byID := threadsByID(t, threadsJSON("acc", "INBOX", raw))
	if len(byID) != 1 {
		t.Fatalf("got %d threads, want 1", len(byID))
	}
	var only Message
	for _, m := range byID {
		only = m
	}
	if !only.Unread {
		t.Error("thread should be Unread when any message is unseen")
	}
	if only.UnreadCount != 2 {
		t.Errorf("UnreadCount = %d, want 2", only.UnreadCount)
	}
	if !only.Starred {
		t.Error("thread should be Starred when any message is starred")
	}
}

func TestThreadsJSONBranchedThreadGetsOriginalThreadID(t *testing.T) {
	// A reply whose subject diverges from the root (different grouping subject
	// but same thread_key) forms its own card that back-points at the original.
	raw := map[string]any{
		"messages": []any{
			msg(10, map[string]any{"thread_key": "k1", "subject": "Quarterly report", "date": float64(100)}),
			msg(11, map[string]any{"thread_key": "k1", "subject": "Re: Budget split", "date": float64(200)}),
		},
	}

	byID := threadsByID(t, threadsJSON("acc", "INBOX", raw))
	if len(byID) != 2 {
		t.Fatalf("got %d threads, want 2 (branch splits into its own card): %#v", len(byID), byID)
	}

	rootSub := threadGroupingSubject("Quarterly report")
	branchSub := threadGroupingSubject("Re: Budget split")
	rootID := formatImapThreadID("acc", "INBOX", "k1#"+rootSub)
	branchID := formatImapThreadID("acc", "INBOX", "k1#"+branchSub)

	root := byID[rootID]
	if root.OriginalThreadID != "" {
		t.Errorf("root thread should have empty OriginalThreadID, got %q", root.OriginalThreadID)
	}
	branch, ok := byID[branchID]
	if !ok {
		t.Fatalf("missing branch thread %q in %#v", branchID, byID)
	}
	if branch.OriginalThreadID != rootID {
		t.Errorf("branch OriginalThreadID = %q, want %q (points at root)", branch.OriginalThreadID, rootID)
	}
}

func TestThreadsJSONDoesNotBranchGmailThreadIDBySubject(t *testing.T) {
	raw := map[string]any{
		"messages": []any{
			msg(10, map[string]any{"thread_key": "gmthrid:123", "subject": "[nonbili/Nora] Profiles bug (Issue #295)", "seen": true}),
			msg(11, map[string]any{"thread_key": "gmthrid:123", "subject": "[nonbili/Nora] Profiles bug [Linux Flatpak] (Issue #295)", "seen": false}),
		},
	}

	byID := threadsByID(t, threadsJSON("acc", "INBOX", raw))
	if len(byID) != 1 {
		t.Fatalf("got %d threads, want 1 Gmail thread despite subject drift: %#v", len(byID), byID)
	}
	wantID := formatImapThreadID("acc", "INBOX", "gmthrid:123")
	thread, ok := byID[wantID]
	if !ok {
		t.Fatalf("missing Gmail thread id %q in %#v", wantID, byID)
	}
	if thread.OriginalThreadID != "" {
		t.Errorf("Gmail thread should not get a branch OriginalThreadID, got %q", thread.OriginalThreadID)
	}
	if !thread.Unread || thread.UnreadCount != 1 {
		t.Errorf("unread aggregation = (%v, %d), want (true, 1)", thread.Unread, thread.UnreadCount)
	}
}

func TestThreadsJSONFoldsGatewayTagsIntoSameThread(t *testing.T) {
	// A gateway-tagged inbound copy must thread with its untagged counterpart.
	raw := map[string]any{
		"messages": []any{
			msg(10, map[string]any{"thread_key": "k1", "subject": "Invoice 42"}),
			msg(11, map[string]any{"thread_key": "k1", "subject": "[EXTERNAL] Re: Invoice 42"}),
		},
	}

	byID := threadsByID(t, threadsJSON("acc", "INBOX", raw))
	if len(byID) != 1 {
		t.Fatalf("got %d threads, want 1 (gateway tag must not split the thread): %#v", len(byID), byID)
	}
}

func TestThreadsJSONFallsBackToUIDKeyWithoutThreadKey(t *testing.T) {
	// Messages with no thread_key each get their own uid: key and never branch.
	raw := map[string]any{
		"messages": []any{
			msg(10, map[string]any{"subject": "One"}),
			msg(11, map[string]any{"subject": "Two"}),
		},
	}

	byID := threadsByID(t, threadsJSON("acc", "INBOX", raw))
	if len(byID) != 2 {
		t.Fatalf("got %d threads, want 2", len(byID))
	}
	for id, m := range byID {
		if m.OriginalThreadID != "" {
			t.Errorf("uid-keyed thread %q should not have OriginalThreadID, got %q", id, m.OriginalThreadID)
		}
	}
}

func TestThreadsJSONSkipsInvalidUIDsAndPassesCursor(t *testing.T) {
	raw := map[string]any{
		"messages": []any{
			msg(0, map[string]any{"thread_key": "k1", "subject": "Dropped"}), // uid<=0 skipped
			msg(10, map[string]any{"thread_key": "k2", "subject": "Kept"}),
		},
		"next_cursor": "cursor-token",
	}

	out := threadsJSON("acc", "INBOX", raw)
	byID := threadsByID(t, out)
	if len(byID) != 1 {
		t.Fatalf("got %d threads, want 1 (uid<=0 dropped)", len(byID))
	}
	if got := out.(map[string]any)["next_cursor"]; got != "cursor-token" {
		t.Errorf("next_cursor = %v, want cursor-token", got)
	}
}

func TestThreadsJSONOmitsCursorWhenEmpty(t *testing.T) {
	raw := map[string]any{
		"messages":    []any{msg(10, map[string]any{"subject": "Hi"})},
		"next_cursor": "",
	}
	out := threadsJSON("acc", "INBOX", raw).(map[string]any)
	if _, present := out["next_cursor"]; present {
		t.Errorf("next_cursor should be omitted when empty, got %#v", out["next_cursor"])
	}
}
