//go:build integration

package main

import (
	"fmt"
	"io"
	"strconv"
	"strings"
	"testing"
	"time"
)

// Keep the accounts in separate profiles: Bob must decrypt with his own key,
// not Alice's sender key (which can also decrypt the outgoing message).
func TestIntegrationOpenPGPMailFlow(t *testing.T) {
	server := startMailServer(t)
	alice, _ := startSidecar(t)
	connectAccount(t, alice, server, "alice", "alice@mail.test")
	// startSidecar snapshots the environment for each child. Starting Bob
	// changes the host's profile paths; use RPC, not host-side path helpers,
	// to inspect either profile below.
	bob, _ := startSidecar(t)
	connectAccount(t, bob, server, "bob", "bob@mail.test")

	createKey := func(sidecar *Sidecar, account string) string {
		t.Helper()
		result := callMap(t, sidecar, "pgp.account.create", map[string]any{"account": account})
		key, _ := result["key"].(map[string]any)
		fingerprint := str(key, "fingerprint")
		if fingerprint == "" {
			t.Fatalf("pgp.account.create returned no fingerprint: %v", result)
		}
		return fingerprint
	}
	aliceFingerprint := createKey(alice, "alice")
	bobFingerprint := createKey(bob, "bob")
	nonce := strconv.FormatInt(time.Now().UnixNano(), 10)
	t.Run("missing recipient key refuses delivery", func(t *testing.T) {
		_, err := alice.Call("send", map[string]any{
			"account": "alice", "to": "bob@mail.test", "subject": "Missing key " + nonce,
			"body": "must not be delivered", "message_id": "itest-missing-key-" + nonce + "@mail.test", "encrypt": true,
		})
		if err == nil || !strings.Contains(err.Error(), "bob@mail.test") {
			t.Fatalf("want missing Bob key error, got %v", err)
		}
		// Check the mailbox itself for a bounded interval, not just the local
		// cache or a subject search (an encrypted subject would be hidden).
		// Allow delivery time even though the expected rejection is synchronous:
		// this assertion must also catch a regression that submits mail anyway.
		client := dialIMAP(t, server.imapPort, "bob@mail.test", testPassword)
		defer client.close()
		client.selectFolder("INBOX")
		deadline := time.Now().Add(5 * time.Second)
		for {
			if uids := parseIMAPSearch(client.do("UID SEARCH ALL")); len(uids) != 0 {
				t.Fatalf("refused send delivered server messages: %v", uids)
			}
			if time.Now().After(deadline) {
				break
			}
			time.Sleep(200 * time.Millisecond)
		}
	})
	for _, exchange := range []struct {
		from, to    *Sidecar
		account     string
		fingerprint string
	}{
		{alice, bob, "alice", aliceFingerprint},
		{bob, alice, "bob", bobFingerprint},
	} {
		public := callMap(t, exchange.from, "pgp.account.exportPublic", map[string]any{"account": exchange.account})
		imported := callMap(t, exchange.to, "pgp.peers.import", map[string]any{
			"data": str(public, "armored"), "fingerprints": []string{exchange.fingerprint},
		})
		if num(imported, "imported") != 1 {
			t.Fatalf("peer key import = %v, want one key", imported)
		}
	}

	subject := "OpenPGP private subject " + nonce
	body := "OpenPGP private body " + nonce
	messageID := "itest-openpgp-" + nonce + "@mail.test"
	sent := callMap(t, alice, "send", map[string]any{
		"account": "alice", "to": "bob@mail.test", "subject": subject,
		"body": body, "message_id": messageID, "encrypt": true,
	})
	if !boolValue(sent, "ok") {
		t.Fatalf("encrypted send failed: %v", sent)
	}
	if undelivered, _ := sent["undelivered"].([]any); len(undelivered) != 0 {
		t.Fatalf("encrypted send left undelivered recipients: %v", undelivered)
	}

	checkMessage := func(t *testing.T, sidecar *Sidecar, account, folder, subject, body, messageID, signer string) {
		t.Helper()
		row := pollFolder(t, sidecar, account, folder, func(m map[string]any) bool {
			return str(m, "subject") == subject
		})
		thread := callMap(t, sidecar, "messages.thread", map[string]any{
			"account": account, "folder": folder, "thread_key": str(row, "thread_key"),
		})
		// Replies share a thread with the original; don't assume the desired
		// message is the first item in the thread.
		var message map[string]any
		rows, _ := thread["messages"].([]any)
		for _, item := range rows {
			candidate, _ := item.(map[string]any)
			if str(candidate, "subject") == subject {
				message = candidate
				break
			}
		}
		if str(message, "subject") != subject || !strings.Contains(str(message, "body"), body) {
			t.Fatalf("decrypted subject/body not restored: %v", message)
		}
		crypto, _ := message["crypto"].(map[string]any)
		if !boolValue(crypto, "encrypted") || !boolValue(crypto, "signed") ||
			str(crypto, "signer") != signer || crypto["problem"] != nil ||
			boolValue(crypto, "partial") || boolValue(crypto, "weak_hash") {
			t.Fatalf("want encrypted mail signed by %s, got %v", signer, crypto)
		}

		// Read server truth, bypassing Meron's decrypted cache. PEEK also avoids
		// changing the message's read flag while checking the ciphertext.
		uid := num(row, "uid")
		if uid == 0 {
			t.Fatalf("delivered encrypted message has no UID: %v", row)
		}
		raw := imapRawMessage(t, server.imapPort, account+"@mail.test", testPassword, folder, uid)
		for _, hidden := range []string{subject, body} {
			if strings.Contains(string(raw), hidden) {
				t.Fatalf("server message leaked plaintext %q", hidden)
			}
		}
		for _, want := range []string{"multipart/encrypted", "application/pgp-encrypted", "-----BEGIN PGP MESSAGE-----", messageID} {
			if !strings.Contains(string(raw), want) {
				t.Fatalf("server message missing %q", want)
			}
		}
	}
	t.Run("recipient decrypts and verifies", func(t *testing.T) {
		checkMessage(t, bob, "bob", "INBOX", subject, body, messageID, aliceFingerprint)
	})
	t.Run("sender Sent copy stays encrypted", func(t *testing.T) {
		checkMessage(t, alice, "alice", "Sent", subject, body, messageID, aliceFingerprint)
	})
	t.Run("reply stays encrypted without explicit flag", func(t *testing.T) {
		// Make this subtest runnable with -run on its own: reply protection
		// consults Bob's cached original, not just the In-Reply-To header.
		pollInbox(t, bob, "bob", func(m map[string]any) bool {
			return str(m, "subject") == subject
		})
		replySubject := "Re: " + subject
		replyBody := "Private automatic reply " + nonce
		replyID := "itest-openpgp-reply-" + nonce + "@mail.test"
		result := callMap(t, bob, "send", map[string]any{
			"account": "bob", "to": "alice@mail.test", "subject": replySubject,
			"body": replyBody, "message_id": replyID, "in_reply_to": messageID, "references": messageID,
		})
		if !boolValue(result, "ok") {
			t.Fatalf("reply send failed: %v", result)
		}
		checkMessage(t, alice, "alice", "INBOX", replySubject, replyBody, replyID, bobFingerprint)
	})
}

// Fetch an IMAP literal by length, rather than treating its MIME lines as IMAP
// responses. The existing line-only command helper is not suitable for bodies.
func imapRawMessage(t *testing.T, port int, user, password, folder string, uid uint32) []byte {
	t.Helper()
	client := dialIMAP(t, port, user, password)
	defer client.close()
	client.selectFolder(folder)
	client.tag++
	tag := fmt.Sprintf("t%d", client.tag)
	client.write([]byte(fmt.Sprintf("%s UID FETCH %d (BODY.PEEK[])\r\n", tag, uid)))
	var raw []byte
	for {
		line, err := client.reader.ReadString('\n')
		if err != nil {
			t.Fatalf("read raw FETCH: %v", err)
		}
		if strings.HasPrefix(line, tag+" ") {
			if !imapTaggedOK(line, tag) || len(raw) == 0 {
				t.Fatalf("raw FETCH returned no message or failed: %s", line)
			}
			return raw
		}
		line = strings.TrimRight(line, "\r\n")
		if start := strings.LastIndex(line, "{"); start >= 0 && strings.HasSuffix(line, "}") {
			size, err := strconv.Atoi(line[start+1 : len(line)-1])
			if err != nil || size <= 0 || size > 1024*1024 {
				t.Fatalf("unexpected FETCH literal size: %q", line)
			}
			raw = make([]byte, size)
			if _, err := io.ReadFull(client.reader, raw); err != nil {
				t.Fatalf("read FETCH literal: %v", err)
			}
		}
	}
}
