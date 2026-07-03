package main

import (
	"encoding/base64"
	"fmt"
	"regexp"
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

func threadsJSON(accountID, folder string, raw any) any {
	object, _ := raw.(map[string]any)
	list, _ := object["messages"].([]any)
	type threadGroup struct {
		message Message
	}
	groups := make(map[string]*threadGroup)
	order := make([]string, 0, len(list))

	// Determine the original root message's subject for each threadKey. The
	// display variant preserves real tags (used for the card title); the grouping
	// variant strips gateway tags (used to match compound keys / branch back-pointers).
	oldestDisplaySubject := make(map[string]string)
	oldestGroupSubject := make(map[string]string)
	oldestUID := make(map[string]int64)
	for _, item := range list {
		msg, _ := item.(map[string]any)
		uid := jsonNumber(msg["uid"])
		if uid <= 0 {
			continue
		}
		msgFolder := jsonString(msg["folder"])
		if msgFolder == "" {
			msgFolder = folder
		}
		threadKey := jsonString(msg["thread_key"])
		if threadKey == "" {
			threadKey = fmt.Sprintf("uid:%d", uid)
		}
		currOldestUID, exists := oldestUID[threadKey]
		if !exists || uid < currOldestUID {
			oldestUID[threadKey] = uid
			oldestDisplaySubject[threadKey] = normalizeThreadSubject(jsonString(msg["subject"]))
			oldestGroupSubject[threadKey] = threadGroupingSubject(jsonString(msg["subject"]))
		}
	}

	for _, item := range list {
		msg, _ := item.(map[string]any)
		uid := jsonNumber(msg["uid"])
		if uid <= 0 {
			continue
		}
		msgFolder := jsonString(msg["folder"])
		if msgFolder == "" {
			msgFolder = folder
		}
		threadKey := jsonString(msg["thread_key"])
		if threadKey == "" {
			threadKey = fmt.Sprintf("uid:%d", uid)
		}
		compoundKey := threadKey
		normSub := ""
		if shouldBranchThreadBySubject(threadKey) {
			normSub = threadGroupingSubject(jsonString(msg["subject"]))
			compoundKey = threadKey + "#" + normSub
		}
		group, exists := groups[compoundKey]
		if !exists {
			threadID := formatImapThreadID(accountID, msgFolder, compoundKey)

			// Compute OriginalThreadID if this is a branched thread
			var originalThreadID string
			if shouldBranchThreadBySubject(threadKey) {
				origSubject := oldestGroupSubject[threadKey]
				if normSub != origSubject {
					originalThreadID = formatImapThreadID(accountID, msgFolder, threadKey+"#"+origSubject)
				}
			}

			// Thread title is the *root* message's normalized subject (without
			// "Re:"/"Fwd:" prefixes) so the list shows the original conversation
			// title regardless of which message we encountered first. Falls back
			// to this message's raw subject only when normalization stripped
			// everything (e.g. an isolated "Re:" with nothing after).
			titleSubject := oldestDisplaySubject[threadKey]
			if titleSubject == "" {
				titleSubject = jsonString(msg["subject"])
			}
			group = &threadGroup{message: Message{
				ID:                threadID,
				AccountID:         accountID,
				FolderID:          msgFolder,
				ThreadID:          threadID,
				FromName:          jsonString(msg["from_name"]),
				FromAddr:          jsonString(msg["from_addr"]),
				Subject:           titleSubject,
				Date:              jsonNumber(msg["date"]),
				OriginalThreadID:  originalThreadID,
				RecipientOverflow: uint32(jsonNumber(msg["recipient_overflow"])),
			}}
			groups[compoundKey] = group
			order = append(order, compoundKey)
		}
		if !jsonBool(msg["seen"]) {
			group.message.Unread = true
			group.message.UnreadCount++
		}
		if jsonBool(msg["starred"]) {
			group.message.Starred = true
		}
	}
	messages := make([]Message, 0, len(order))
	for _, key := range order {
		messages = append(messages, groups[key].message)
	}
	out := map[string]any{"threads": messages}
	if cursor, _ := object["next_cursor"].(string); cursor != "" {
		out["next_cursor"] = cursor
	}
	return out
}

func threadMessagesJSON(accountID, threadID, folder string, raw any, subjectFilter string) any {
	object, _ := raw.(map[string]any)
	list, _ := object["messages"].([]any)
	messages := make([]Message, 0, len(list))
	for _, item := range list {
		entry, _ := item.(map[string]any)
		msg, _ := entry["message"].(map[string]any)
		if subjectFilter != "" {
			normSub := threadGroupingSubject(jsonString(msg["subject"]))
			if normSub != subjectFilter {
				continue
			}
		}
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

func shouldBranchThreadBySubject(threadKey string) bool {
	return !strings.HasPrefix(threadKey, "uid:") && !strings.HasPrefix(threadKey, "gmthrid:")
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

var subjectPrefixRegex = regexp.MustCompile(`(?i)^(?:\[[^\]]+\]\s*)*(?:re|fw|fwd|aw|sv|vs|rv|res|tr|antw|wg|答复|回复|转发)(?:\[[0-9]+\]|\([0-9]+\))?[:：]\s*`)

func normalizeThreadSubject(subject string) string {
	subject = strings.TrimSpace(subject)
	for {
		loc := subjectPrefixRegex.FindStringIndex(subject)
		if loc == nil || loc[0] != 0 {
			break
		}
		subject = strings.TrimSpace(subject[loc[1]:])
	}
	return subject
}

var leadingBracketTagRegex = regexp.MustCompile(`^\s*\[[^\]]*\]\s*`)

// threadGroupingSubject normalizes a subject for thread grouping/matching only.
// In addition to Re:/Fwd: prefixes it strips ANY leading bracketed tag (e.g.
// gateway-injected "[EXTERNAL]", "[CAUTION]") so a tagged inbound copy threads
// with its untagged Sent counterpart. Not used for display — see
// normalizeThreadSubject for the title-facing variant that preserves real tags.
func threadGroupingSubject(subject string) string {
	subject = strings.TrimSpace(subject)
	for {
		if loc := subjectPrefixRegex.FindStringIndex(subject); loc != nil && loc[0] == 0 {
			subject = strings.TrimSpace(subject[loc[1]:])
			continue
		}
		if loc := leadingBracketTagRegex.FindStringIndex(subject); loc != nil && loc[0] == 0 {
			subject = strings.TrimSpace(subject[loc[1]:])
			continue
		}
		break
	}
	return subject
}
