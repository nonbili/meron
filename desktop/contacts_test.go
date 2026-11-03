package main

import "testing"

func TestContactNotesRequireRunningEngine(t *testing.T) {
	app := &App{}
	for _, command := range []string{"contacts.noteGet", "contacts.noteSet"} {
		if _, err := app.Invoke(command, map[string]any{}); err == nil {
			t.Fatalf("%s must not silently succeed without an engine", command)
		}
	}
}

func TestContactThreadsFinalShapePreservesIdentity(t *testing.T) {
	raw := map[string]any{"threads": []any{map[string]any{
		"id": "acct#Archive#t.a2V5", "thread_id": "acct#Archive#t.a2V5",
		"account_id": "acct", "folder_id": "Archive", "subject": "Hello",
	}}, "next_cursor": "cursor"}
	result := threadsJSON("", "INBOX", raw).(map[string]any)
	threads := result["threads"].([]Message)
	if len(threads) != 1 || threads[0].FolderID != "Archive" || threads[0].ThreadID != "acct#Archive#t.a2V5" {
		t.Fatalf("final contact history was dropped or retargeted: %#v", result)
	}
	if result["next_cursor"] != "cursor" {
		t.Fatal("cursor lost")
	}
}
