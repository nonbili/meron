package main

import (
	"reflect"
	"testing"
)

// messagesOf pulls the []Message out of a threadMessagesJSON/messageJSON result.
func messagesOf(t *testing.T, out any) []Message {
	t.Helper()
	obj, ok := out.(map[string]any)
	if !ok {
		t.Fatalf("result is %T, want map[string]any", out)
	}
	list, ok := obj["messages"].([]Message)
	if !ok {
		t.Fatalf("messages field is %T, want []Message", obj["messages"])
	}
	return list
}

func entry(uid int, folder string, message map[string]any) map[string]any {
	e := map[string]any{"uid": float64(uid), "message": message}
	if folder != "" {
		e["folder"] = folder
	}
	return e
}

func TestThreadMessagesJSONSubjectFilterDropsOtherBranches(t *testing.T) {
	raw := map[string]any{
		"messages": []any{
			entry(10, "", map[string]any{"subject": "Budget"}),
			entry(11, "", map[string]any{"subject": "Re: Budget"}),       // same grouping subject, kept
			entry(12, "", map[string]any{"subject": "[EXT] Re: Budget"}), // gateway tag stripped, kept
			entry(13, "", map[string]any{"subject": "Re: Lunch"}),        // different branch, dropped
		},
	}

	filter := threadGroupingSubject("Budget")
	got := messagesOf(t, threadMessagesJSON("acc", "tid", "INBOX", raw, filter))
	if len(got) != 3 {
		t.Fatalf("got %d messages, want 3 (only the matching branch): %#v", len(got), got)
	}
	for _, m := range got {
		if threadGroupingSubject(m.Subject) != filter {
			t.Errorf("kept message with subject %q not matching filter %q", m.Subject, filter)
		}
	}
}

func TestThreadMessagesJSONEmptyFilterKeepsAll(t *testing.T) {
	raw := map[string]any{
		"messages": []any{
			entry(10, "", map[string]any{"subject": "A"}),
			entry(11, "", map[string]any{"subject": "B"}),
		},
	}
	got := messagesOf(t, threadMessagesJSON("acc", "tid", "INBOX", raw, ""))
	if len(got) != 2 {
		t.Fatalf("got %d messages, want 2 (empty filter keeps all)", len(got))
	}
}

func TestThreadMessagesJSONPerMessageFolderAndFields(t *testing.T) {
	raw := map[string]any{
		"messages": []any{
			entry(10, "Sent", map[string]any{
				"subject":     "Hi",
				"from_addr":   "me@x.com",
				"attachments": []any{map[string]any{"name": "a.pdf"}},
			}),
			entry(11, "", map[string]any{"subject": "Hi"}), // no folder -> nominal folder
		},
	}

	got := messagesOf(t, threadMessagesJSON("acc", "tid", "INBOX", raw, ""))
	if got[0].FolderID != "Sent" {
		t.Errorf("message 0 folder = %q, want Sent (per-message source folder)", got[0].FolderID)
	}
	if got[1].FolderID != "INBOX" {
		t.Errorf("message 1 folder = %q, want INBOX (fallback to nominal folder)", got[1].FolderID)
	}
	if got[0].ID != "tid#10" {
		t.Errorf("message id = %q, want tid#10", got[0].ID)
	}
	if !got[0].HasAttachments {
		t.Error("message 0 should report HasAttachments")
	}
	if got[1].HasAttachments {
		t.Error("message 1 has no attachments")
	}
}

func TestThreadMessagesJSONUnreadFromSeenFlag(t *testing.T) {
	raw := map[string]any{
		"messages": []any{
			func() map[string]any { e := entry(10, "", map[string]any{"subject": "x"}); e["seen"] = false; return e }(),
			func() map[string]any { e := entry(11, "", map[string]any{"subject": "x"}); e["seen"] = true; return e }(),
		},
	}
	got := messagesOf(t, threadMessagesJSON("acc", "tid", "INBOX", raw, ""))
	if !got[0].Unread {
		t.Error("message with seen=false should be Unread")
	}
	if got[1].Unread {
		t.Error("message with seen=true should not be Unread")
	}
}

func TestMessageJSONShapesSingleMessage(t *testing.T) {
	raw := map[string]any{
		"message": map[string]any{
			"from_name": "Ann",
			"subject":   "Hello",
			"body":      "hi there",
		},
	}
	got := messagesOf(t, messageJSON("acc", "tid", "INBOX", raw))
	if len(got) != 1 {
		t.Fatalf("got %d messages, want 1", len(got))
	}
	m := got[0]
	if m.ID != "tid" || m.ThreadID != "tid" || m.FolderID != "INBOX" || m.FromName != "Ann" || m.Subject != "Hello" {
		t.Errorf("unexpected message shape: %#v", m)
	}
}

func TestImapUIDsFromPayload(t *testing.T) {
	tid := formatImapThreadID("acc", "INBOX", "k1#Topic")
	payload := map[string]any{
		"message_ids": []any{
			tid + "#42",   // prefixed UID
			"77",          // bare UID
			tid,           // full thread id resolving to no UID -> dropped
			"acc#INBOX#7", // standalone thread id with UID
			"not-a-uid",   // garbage -> dropped
		},
	}
	got := imapUIDsFromPayload(tid, payload)
	want := []uint32{42, 77, 7}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("imapUIDsFromPayload = %v, want %v", got, want)
	}
}

func TestRSSItemKeysFromPayload(t *testing.T) {
	tid := "acc#rss#sub1"
	payload := map[string]any{
		"message_ids": []any{
			tid + "#item-a",
			tid + "#item-b",
			"other#rss#sub#item-c", // wrong prefix -> dropped
			tid + "#",              // empty key -> dropped
		},
	}
	got := rssItemKeysFromPayload(tid, payload)
	want := []string{"item-a", "item-b"}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("rssItemKeysFromPayload = %v, want %v", got, want)
	}
}

func TestParseRSSThreadID(t *testing.T) {
	cases := []struct {
		in      string
		account string
		sub     string
		ok      bool
	}{
		{"acc#rss#sub1#item", "acc", "sub1", true},
		{"acc#rss#sub1", "acc", "sub1", true},
		{"acc#rss#", "", "", false},   // empty rest
		{"#rss#sub1", "", "", false},  // empty account
		{"acc#sub1", "", "", false},   // not an rss id
		{"acc#rss#sub1#", "acc", "sub1", true},
	}
	for _, c := range cases {
		acc, sub, ok := parseRSSThreadID(c.in)
		if acc != c.account || sub != c.sub || ok != c.ok {
			t.Errorf("parseRSSThreadID(%q) = (%q,%q,%v), want (%q,%q,%v)", c.in, acc, sub, ok, c.account, c.sub, c.ok)
		}
	}
}
