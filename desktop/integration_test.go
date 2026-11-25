//go:build integration

package main

// End-to-end tests over the real stack: sidecar → SMTP submission → mail server →
// IMAP fetch → SQLite store. See mailserver_harness_test.go for the setup.

import (
	"fmt"
	"testing"
	"time"
)

// pollFolder refreshes an account folder until predicate matches a message row
func TestIMAPResponseParsing(t *testing.T) {
	t.Run("tagged status is case insensitive", func(t *testing.T) {
		if !imapTaggedOK("t4 ok completed", "t4") {
			t.Fatal("lowercase tagged OK was rejected")
		}
		if imapTaggedOK("t4 NO failed", "t4") {
			t.Fatal("tagged NO was accepted")
		}
		if imapTaggedOK("t40 OK completed", "t4") {
			t.Fatal("different tag was accepted")
		}
	})

	t.Run("uidvalidity tolerates response text and casing", func(t *testing.T) {
		got, ok := parseIMAPUIDValidity([]string{
			"* FLAGS (\\Seen)",
			"* ok [uidvalidity 4294967295] UIDs valid",
		})
		if !ok || got != ^uint32(0) {
			t.Fatalf("UIDVALIDITY = %d, %v; want %d, true", got, ok, ^uint32(0))
		}
		for _, lines := range [][]string{
			{"* OK mailbox selected"},
			{"* OK [UIDVALIDITY 0] invalid"},
			{"* OK [UIDVALIDITY nope] invalid"},
		} {
			if got, ok := parseIMAPUIDValidity(lines); ok || got != 0 {
				t.Fatalf("invalid UIDVALIDITY parsed as %d, %v from %v", got, ok, lines)
			}
		}
	})

	t.Run("flags tolerate reordered fetch fields", func(t *testing.T) {
		lines := []string{
			"* 3 FETCH (FLAGS (\\Seen custom) RFC822.SIZE 42 uid 77)",
			"* 4 FETCH (UID 78 FLAGS (\\Flagged))",
		}
		if got := parseIMAPFlags(lines, 77); got != `\Seen custom` {
			t.Fatalf("flags for UID 77 = %q", got)
		}
		if got := parseIMAPFlags(lines, 78); got != `\Flagged` {
			t.Fatalf("flags for UID 78 = %q", got)
		}
		if got := parseIMAPFlags(lines, 79); got != "" {
			t.Fatalf("flags for absent UID = %q", got)
		}
	})

	t.Run("search handles empty and malformed fields", func(t *testing.T) {
		got := parseIMAPSearch([]string{
			"* SEARCH 2 nope 3x 0 17",
			"* VANISHED 99",
		})
		if fmt.Sprint(got) != "[2 17]" {
			t.Fatalf("SEARCH UIDs = %v, want [2 17]", got)
		}
		if got := parseIMAPSearch([]string{"* SEARCH"}); len(got) != 0 {
			t.Fatalf("empty SEARCH = %v", got)
		}
	})
}

func TestIntegrationMailFlow(t *testing.T) {
	server := startMailServer(t)
	sidecar, events := startSidecar(t)

	connectAccount(t, sidecar, server, "alice", "alice@mail.test")
	connectAccount(t, sidecar, server, "bob", "bob@mail.test")
	f := &mailFlow{server: server, sidecar: sidecar, events: events}

	t.Run("folders", f.folders)

	nonce := fmt.Sprintf("%d", time.Now().UnixNano())
	subject := "Meron integration " + nonce
	// Bare id (no angle brackets) — the app convention: the frontend mints bare
	// ids and the backend wraps them when emitting headers.
	messageID := fmt.Sprintf("itest-%s@mail.test", nonce)
	f.nonce, f.subject, f.messageID = nonce, subject, messageID

	t.Run("send and receive", f.sendAndReceive)
	t.Run("threading", f.threading)
	t.Run("move and flags", f.moveAndFlags)
	t.Run("failed imap write does not falsely mark read locally", f.failedImapWriteDoesNotFalselyMarkReadLocally)
	t.Run("search and starred items", f.searchAndStarredItems)
	t.Run("draft lifecycle", f.draftLifecycle)
	t.Run("quick reply draft lifecycle", f.quickReplyDraftLifecycle)
	t.Run("attachments", f.attachments)
	t.Run("delete moves to trash", f.deleteMovesToTrash)
	t.Run("empty trash", f.emptyTrash)
	t.Run("copy keeps original", f.copyKeepsOriginal)
	t.Run("mark all read", f.markAllRead)
	t.Run("contacts suggest from history", f.contactsSuggestFromHistory)
	t.Run("archive from the archive folder takes the inbox copies", f.archiveFromTheArchiveFolderTakesTheInboxCopies)
	t.Run("grouped thread cards", f.groupedThreadCards)
	t.Run("external expunge prunes the local row", f.externalExpungePrunesTheLocalRow)
	t.Run("external flag changes reconcile locally", f.externalFlagChangesReconcileLocally)
	t.Run("uidvalidity change clears the cached folder", f.uidvalidityChangeClearsTheCachedFolder)
	t.Run("idle pushes folder updates while watched", f.idlePushesFolderUpdatesWhileWatched)
	t.Run("unified inbox merges and writes across accounts", f.unifiedInboxMergesAndWritesAcrossAccounts)
	t.Run("unified mark-all-read reports a partial failure", f.unifiedMarkAllReadReportsAPartialFailure)
	t.Run("unified cursors advance each account independently", f.unifiedCursorsAdvanceEachAccountIndependently)
	t.Run("encoded subject and quoted-printable body decode", f.encodedSubjectAndQuotedPrintableBodyDecode)
	t.Run("multipart alternative keeps both text and html", f.multipartAlternativeKeepsBothTextAndHtml)
	t.Run("nested multipart keeps inline and downloadable attachments", f.nestedMultipartKeepsInlineAndDownloadableAttachments)
	t.Run("message without a message-id still threads", f.messageWithoutAMessageIdStillThreads)
	t.Run("empty junk", f.emptyJunk)
	t.Run("clearing the attachment cache refetches the bytes", f.clearingTheAttachmentCacheRefetchesTheBytes)
	t.Run("cold sync from an empty profile sees server state", f.coldSyncFromAnEmptyProfileSeesServerState)
	t.Run("sent copy honors the account override", f.sentCopyHonorsTheAccountOverride)
	t.Run("connect rejects bad credentials", f.connectRejectsBadCredentials)
	t.Run("sync recovers after a server restart", f.syncRecoversAfterAServerRestart)
	t.Run("flag writes recover from a stale pooled connection", f.flagWritesRecoverFromAStalePooledConnection)

	// Last on purpose: no later subtest sends from alice, so nothing but the
	// piggyback under test can refresh her Sent folder.
	t.Run("sent from another client surfaces via inbox sync", f.sentFromAnotherClientSurfacesViaInboxSync)
}

// mailFlow is what TestIntegrationMailFlow's subtests share: one mail server and
// sidecar with alice and bob connected, and the message the flow sends first.
type mailFlow struct {
	server    *mailServer
	sidecar   *Sidecar
	events    *eventLog
	nonce     string
	subject   string
	messageID string
}
