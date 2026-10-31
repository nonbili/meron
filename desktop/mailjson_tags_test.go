package main

import (
	"reflect"
	"testing"
)

func TestThreadsJSONKeepsTags(t *testing.T) {
	raw := map[string]any{"threads": []any{map[string]any{"id": "a#INBOX#t", "tags": []any{"todo", "work"}}}}
	out := threadsJSON("a", "INBOX", raw).(map[string]any)
	messages := out["threads"].([]Message)
	if !reflect.DeepEqual(messages[0].Tags, []string{"todo", "work"}) {
		t.Fatalf("tags lost on the bridge: %v", messages[0].Tags)
	}

	cards := map[string]any{"cards": []any{map[string]any{"thread_key": "t", "tags": []any{"todo"}}}}
	out = threadsJSON("a", "INBOX", cards).(map[string]any)
	messages = out["threads"].([]Message)
	if !reflect.DeepEqual(messages[0].Tags, []string{"todo"}) {
		t.Fatalf("tags lost on the card path: %v", messages[0].Tags)
	}
}
