package main

import "testing"

// The lock on an encrypted thread card and a message's OpenPGP state survive
// the bridge's typed transport struct, on every path that decodes into it.
func TestBridgeKeepsEncryptionState(t *testing.T) {
	raw := map[string]any{"threads": []any{map[string]any{"id": "a#INBOX#t", "encrypted": "verified"}}}
	messages := threadsJSON("a", "INBOX", raw).(map[string]any)["threads"].([]Message)
	if messages[0].Encrypted != "verified" {
		t.Fatalf("lock lost on the thread path: %q", messages[0].Encrypted)
	}

	cards := map[string]any{"cards": []any{map[string]any{"thread_key": "t", "encrypted": "known"}}}
	messages = threadsJSON("a", "INBOX", cards).(map[string]any)["threads"].([]Message)
	if messages[0].Encrypted != "known" {
		t.Fatalf("lock lost on the card path: %q", messages[0].Encrypted)
	}

	crypto := map[string]any{"encrypted": true, "trust": "known"}
	read := map[string]any{"message": map[string]any{"subject": "s"}, "crypto": crypto}
	shaped := messageJSON("a", "a#INBOX#u", "INBOX", read).(map[string]any)["messages"].([]Message)
	got, _ := shaped[0].Crypto.(map[string]any)
	if got["encrypted"] != true {
		t.Fatalf("crypto lost on the single-message read: %v", shaped[0].Crypto)
	}
}
