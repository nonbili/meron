package main

import (
	"encoding/base64"
	"fmt"
	"strconv"
	"strings"
)

func foldersJSON(accountID string, raw any) any {
	object, _ := raw.(map[string]any)
	list, _ := object["folders"].([]any)
	folders := make([]Folder, 0, len(list))
	for _, item := range list {
		folderObject, _ := item.(map[string]any)
		name, _ := folderObject["name"].(string)
		if name == "" {
			continue
		}
		delimiter, _ := folderObject["delimiter"].(string)
		role, _ := folderObject["role"].(string)
		if role == "" {
			role = "folder"
			if strings.EqualFold(name, "INBOX") {
				role = "inbox"
			} else if looksLikeArchiveFolder(name) {
				role = "archive"
			}
		}
		unread := uint32(jsonNumber(folderObject["unread"]))
		folders = append(folders, Folder{ID: name, AccountID: accountID, Name: name, Role: role, Delimiter: delimiter, Unread: unread})
	}
	return map[string]any{"folders": folders}
}

func looksLikeArchiveFolder(name string) bool {
	switch strings.ToLower(name) {
	case "archive", "archives", "all mail", "inbox.archive", "inbox.archives", "[gmail]/all mail", "[google mail]/all mail":
		return true
	default:
		return false
	}
}

// threadsJSON maps the core's ready thread cards (grouping, subject branching,
// root titles, and unread counts all live in the sidecar, shared with mobile)
// into bridge Messages; only thread-id minting happens here.
func threadsJSON(accountID, folder string, raw any) any {
	object, _ := raw.(map[string]any)
	list, _ := object["cards"].([]any)
	messages := make([]Message, 0, len(list))
	for _, item := range list {
		card, _ := item.(map[string]any)
		threadKey := jsonString(card["thread_key"])
		if threadKey == "" {
			continue
		}
		msgFolder := jsonString(card["folder"])
		if msgFolder == "" {
			msgFolder = folder
		}
		threadID := formatImapThreadID(accountID, msgFolder, threadKey)
		var originalThreadID string
		if original := jsonString(card["original_thread_key"]); original != "" {
			originalThreadID = formatImapThreadID(accountID, msgFolder, original)
		}
		messages = append(messages, Message{
			ID:                threadID,
			AccountID:         accountID,
			FolderID:          msgFolder,
			ThreadID:          threadID,
			FromName:          jsonString(card["from_name"]),
			FromAddr:          jsonString(card["from_addr"]),
			Subject:           jsonString(card["subject"]),
			Date:              jsonNumber(card["date"]),
			Unread:            jsonBool(card["unread"]),
			UnreadCount:       uint32(jsonNumber(card["unread_count"])),
			Starred:           jsonBool(card["starred"]),
			HasDraft:          jsonBool(card["has_draft"]),
			OriginalThreadID:  originalThreadID,
			RecipientOverflow: uint32(jsonNumber(card["recipient_overflow"])),
		})
	}
	out := map[string]any{
		"threads":       messages,
		"folder_unread": uint32(jsonNumber(object["folder_unread"])),
	}
	if cursor, _ := object["next_cursor"].(string); cursor != "" {
		out["next_cursor"] = cursor
	}
	return out
}

func threadMessagesJSON(accountID, threadID, folder string, raw any) any {
	object, _ := raw.(map[string]any)
	list, _ := object["messages"].([]any)
	messages := make([]Message, 0, len(list))
	for _, item := range list {
		entry, _ := item.(map[string]any)
		msg, _ := entry["message"].(map[string]any)
		uid := jsonNumber(entry["uid"])
		attachments := msg["attachments"]
		attachmentList, _ := attachments.([]any)
		messageID := fmt.Sprintf("%s#%d", threadID, uid)
		// The thread spans folders; each message keeps its own source folder so
		// per-message actions (e.g. single-message delete) target the right
		// mailbox rather than the thread's nominal folder.
		msgFolder := jsonString(entry["folder"])
		if msgFolder == "" {
			msgFolder = folder
		}
		messages = append(messages, Message{
			ID:             messageID,
			AccountID:      accountID,
			FolderID:       msgFolder,
			ThreadID:       threadID,
			Outgoing:       jsonBool(entry["outgoing"]),
			FromName:       jsonString(msg["from_name"]),
			FromAddr:       jsonString(msg["from_addr"]),
			To:             jsonString(msg["to"]),
			ReplyTo:        jsonString(msg["reply_to"]),
			Cc:             jsonString(msg["cc"]),
			Bcc:            jsonString(msg["bcc"]),
			MessageID:      jsonString(msg["message_id"]),
			References:     jsonString(msg["references"]),
			Subject:        jsonString(msg["subject"]),
			Preview:        jsonString(msg["preview"]),
			Body:           jsonString(msg["body"]),
			BodyHTML:       jsonString(msg["body_html"]),
			Date:           jsonNumber(msg["date"]),
			Unread:         !jsonBool(entry["seen"]),
			Starred:        jsonBool(entry["starred"]),
			HasAttachments: len(attachmentList) > 0,
			Attachments:    attachmentList,
		})
	}
	out := map[string]any{"messages": messages}
	if cursor, _ := object["next_cursor"].(string); cursor != "" {
		out["next_cursor"] = cursor
	}
	return out
}

func messageJSON(accountID, threadID, folder string, raw any) any {
	object, _ := raw.(map[string]any)
	msg, _ := object["message"].(map[string]any)
	attachments := msg["attachments"]
	attachmentList, _ := attachments.([]any)
	return map[string]any{"messages": []Message{{
		ID:             threadID,
		AccountID:      accountID,
		FolderID:       folder,
		ThreadID:       threadID,
		Outgoing:       jsonBool(object["outgoing"]),
		FromName:       jsonString(msg["from_name"]),
		FromAddr:       jsonString(msg["from_addr"]),
		To:             jsonString(msg["to"]),
		ReplyTo:        jsonString(msg["reply_to"]),
		Cc:             jsonString(msg["cc"]),
		Bcc:            jsonString(msg["bcc"]),
		MessageID:      jsonString(msg["message_id"]),
		References:     jsonString(msg["references"]),
		Subject:        jsonString(msg["subject"]),
		Preview:        jsonString(msg["preview"]),
		Body:           jsonString(msg["body"]),
		BodyHTML:       jsonString(msg["body_html"]),
		Date:           jsonNumber(msg["date"]),
		HasAttachments: len(attachmentList) > 0,
		Attachments:    attachmentList,
	}}}
}

func parseImapThreadID(threadID string) (ImapThreadIDs, bool) {
	first := strings.Index(threadID, "#")
	last := strings.LastIndex(threadID, "#")
	if first <= 0 || last <= first {
		return ImapThreadIDs{}, false
	}
	keyPart := threadID[last+1:]
	if encoded, ok := strings.CutPrefix(keyPart, "t."); ok {
		decoded, err := base64.RawURLEncoding.DecodeString(encoded)
		if err != nil {
			return ImapThreadIDs{}, false
		}
		return ImapThreadIDs{Account: threadID[:first], Folder: threadID[first+1 : last], ThreadKey: string(decoded)}, true
	}
	uid, err := strconv.ParseUint(keyPart, 10, 32)
	if err != nil {
		return ImapThreadIDs{}, false
	}
	return ImapThreadIDs{Account: threadID[:first], Folder: threadID[first+1 : last], UID: uint32(uid)}, true
}

// canonThreadFolder mirrors the sidecar's canon_folder so a thread always gets
// the same id no matter which path mints it. Without this the inbox is spelled
// "inbox" by the thread-list builder (the UI folder id) and "INBOX" by the
// notification/cached-row paths, so a notification-opened thread's id never
// matches its list card — selection/reply-target lookups then fall back to the
// header-less card and replies go out unthreaded.
func canonThreadFolder(folder string) string {
	if strings.EqualFold(folder, "inbox") {
		return "INBOX"
	}
	return folder
}

func formatImapThreadID(accountID, folder, threadKey string) string {
	encoded := base64.RawURLEncoding.EncodeToString([]byte(threadKey))
	return fmt.Sprintf("%s#%s#t.%s", accountID, canonThreadFolder(folder), encoded)
}

func formatParsedImapThreadIDInFolder(ids ImapThreadIDs, folder string) string {
	if ids.ThreadKey == "" && ids.UID > 0 {
		return fmt.Sprintf("%s#%s#%d", ids.Account, canonThreadFolder(folder), ids.UID)
	}
	return formatImapThreadID(ids.Account, folder, ids.ThreadKey)
}

func withMovedThreadLocation(res any, ids ImapThreadIDs, folderField string) map[string]any {
	out, _ := res.(map[string]any)
	if out == nil {
		out = map[string]any{"ok": true}
	}
	folder := jsonString(out[folderField])
	if folder != "" {
		out["thread_id"] = formatParsedImapThreadIDInFolder(ids, folder)
	}
	return out
}
