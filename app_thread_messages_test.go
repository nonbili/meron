package main

import (
	"reflect"
	"testing"
)

// messagesOf pulls the []Message out of a messageJSON result. (Thread reads
// are shaped in the sidecar's shared thread_read module and pass through the
// bridge untouched.)
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
		{"acc#rss#", "", "", false},  // empty rest
		{"#rss#sub1", "", "", false}, // empty account
		{"acc#sub1", "", "", false},  // not an rss id
		{"acc#rss#sub1#", "acc", "sub1", true},
	}
	for _, c := range cases {
		acc, sub, ok := parseRSSThreadID(c.in)
		if acc != c.account || sub != c.sub || ok != c.ok {
			t.Errorf("parseRSSThreadID(%q) = (%q,%q,%v), want (%q,%q,%v)", c.in, acc, sub, ok, c.account, c.sub, c.ok)
		}
	}
}
