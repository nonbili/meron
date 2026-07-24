package main

import (
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

func (a *App) mailSync(payload map[string]any) (any, error) {
	accountID, _ := payload["account_id"].(string)
	folder, _ := payload["folder"].(string)
	if folder == "" {
		folder = "inbox"
	}
	engine := "meron_mail"
	params := map[string]any{"account": accountID, "folder": folder, "limit": 250}
	if isRSSAccountID(accountID) {
		engine = "rss"
		delete(params, "folder")
	}
	online := false
	if a.sidecar != nil && a.sidecar.Started() {
		// The sidecar routes messages.sync to the right engine by account.
		if _, err := a.sidecar.Call("messages.sync", params); err == nil {
			online = true
		}
	}
	return map[string]any{"ok": true, "synced_at": "now", "engine": engine, "online": online}, nil
}

func (a *App) watchFolder(command string, payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"ok": true, "online": false}, nil
	}
	return a.sidecar.Call(command, payload)
}

func (a *App) folderList(payload map[string]any) (any, error) {
	var req FolderListRequest
	_ = decode(payload, &req)
	if req.AccountID == "" {
		req.AccountID, _ = payload["account_id"].(string)
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"folders": []Folder{}}, nil
	}
	res, err := a.sidecar.Call("folders.list", map[string]any{"account": req.AccountID, "refresh": req.Refresh})
	if err != nil {
		return nil, err
	}
	// RSS folders arrive already in final bridge shape; mail returns raw rows.
	if isRSSAccountID(req.AccountID) {
		return res, nil
	}
	return foldersJSON(req.AccountID, res), nil
}

func (a *App) folderCreate(payload map[string]any) (any, error) {
	var req FolderCreateRequest
	_ = decode(payload, &req)
	if req.AccountID == "" {
		req.AccountID, _ = payload["account_id"].(string)
	}
	if req.Name == "" {
		req.Name, _ = payload["name"].(string)
	}
	if req.AccountID == "" {
		return nil, errors.New("account_id is required")
	}
	if strings.TrimSpace(req.Name) == "" {
		return nil, errors.New("Folder name is required")
	}
	if isRSSAccountID(req.AccountID) {
		return nil, errors.New("RSS accounts do not support folders")
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	res, err := a.sidecar.Call("folders.create", map[string]any{"account": req.AccountID, "name": req.Name})
	if err != nil {
		return nil, err
	}
	return foldersJSON(req.AccountID, res), nil
}

func (a *App) threadList(payload map[string]any) (any, error) {
	var req ThreadListRequest
	_ = decode(payload, &req)
	if req.AccountID == "" {
		req.AccountID, _ = payload["account_id"].(string)
	}
	if req.FolderID == "" {
		req.FolderID = "inbox"
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"threads": []Message{}}, nil
	}
	if req.Filter == "starred" {
		a.logf("threadList starred: account=%s folder=%s query=%q refresh=%t", req.AccountID, req.FolderID, req.Query, req.Refresh)
	}
	method := "messages.recent"
	if req.AccountID == "unified" {
		method = "messages.unifiedRecent"
	}
	res, err := a.sidecar.Call(method, map[string]any{
		"account":       req.AccountID,
		"folder":        req.FolderID,
		"query":         req.Query,
		"filter":        req.Filter,
		"before_cursor": req.BeforeCursor,
		"limit":         50,
		"refresh":       req.Refresh,
		// Thread grouping (subject branching, root titles, unread counts)
		// runs in the core, shared with mobile; the bridge only mints ids.
		"group": true,
	})
	if err != nil {
		return nil, err
	}
	// RSS threads arrive already in final bridge shape; mail returns raw rows.
	if isRSSAccountID(req.AccountID) {
		return res, nil
	}
	out := threadsJSON(req.AccountID, req.FolderID, res)
	if req.Filter == "starred" {
		rawCount := 0
		if obj, ok := res.(map[string]any); ok {
			if list, ok := obj["cards"].([]any); ok {
				rawCount = len(list)
			}
		}
		threadCount := 0
		if obj, ok := out.(map[string]any); ok {
			if list, ok := obj["threads"].([]Message); ok {
				threadCount = len(list)
			} else if list, ok := obj["threads"].([]any); ok {
				threadCount = len(list)
			}
		}
		a.logf("threadList starred: raw_messages=%d grouped_threads=%d", rawCount, threadCount)
	}
	return out, nil
}

// starredItems passes through the core's final cross-account item model. The
// bridge only decodes it into its typed transport struct.
func (a *App) starredItems(payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"items": []Message{}}, nil
	}
	res, err := a.sidecar.Call("starred.items", payload)
	if err != nil {
		return nil, err
	}
	object, _ := res.(map[string]any)
	rows, _ := object["items"].([]any)
	encoded, _ := json.Marshal(rows)
	items := make([]Message, 0, len(rows))
	_ = json.Unmarshal(encoded, &items)
	out := map[string]any{"items": items}
	if cursor, _ := object["next_cursor"].(string); cursor != "" {
		out["next_cursor"] = cursor
	}
	return out, nil
}

func (a *App) threadRead(payload map[string]any) (any, error) {
	threadID, _ := payload["thread_id"].(string)
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"messages": []Message{}}, nil
	}
	// Pagination is opt-in from the frontend: when `limit` is omitted the
	// engine returns the full thread (used by markRead's scan path).
	limit, hasLimit := payload["limit"]
	beforeCursor, _ := payload["before_cursor"].(string)
	if _, _, ok := parseRSSThreadID(threadID); ok {
		params := map[string]any{"thread_id": threadID}
		if hasLimit {
			params["limit"] = limit
		}
		if beforeCursor != "" {
			params["before_cursor"] = beforeCursor
		}
		res, err := a.sidecar.Call("rss.thread", params)
		if err != nil {
			return nil, err
		}
		// Already final bridge-shaped Message JSON.
		return res, nil
	}
	if ids, ok := parseImapThreadID(threadID); ok {
		method := "messages.thread"
		// The sidecar splits branch-compound keys and scopes the thread to the
		// subject branch itself; the bridge passes the key through untouched.
		params := map[string]any{"account": ids.Account, "folder": ids.Folder, "thread_key": ids.ThreadKey}
		if ids.ThreadKey == "" && ids.UID > 0 {
			method = "messages.read"
			params = map[string]any{"account": ids.Account, "folder": ids.Folder, "uid": ids.UID}
		}
		if method == "messages.thread" {
			// The sidecar shapes final bridge-ready message JSON and needs the
			// frontend's exact thread id for the messages' id/thread_id fields.
			params["thread_id"] = threadID
			if hasLimit {
				params["limit"] = limit
			}
			if beforeCursor != "" {
				params["before_cursor"] = beforeCursor
			}
		}
		res, err := a.sidecar.Call(method, params)
		if err != nil {
			return nil, err
		}
		if method == "messages.thread" {
			// Already final bridge-shaped Message JSON (same contract as
			// rss.thread above).
			return res, nil
		}
		return messageJSON(ids.Account, threadID, ids.Folder, res), nil
	}
	return map[string]any{"messages": []Message{}}, nil
}

func (a *App) mailDelete(payload map[string]any) (any, error) {
	threadID, _ := payload["thread_id"].(string)
	if threadID == "" {
		return map[string]any{"ok": true}, nil
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, _, ok := parseRSSThreadID(threadID); ok {
		return nil, errors.New("feed items cannot be moved to trash")
	}
	if ids, ok := parseImapThreadID(threadID); ok {
		sourceFolder := deleteFolder(payload, ids.Folder)
		// Branch-compound thread keys pass through untouched; the sidecar
		// splits them and scopes the delete to the subject branch.
		params := map[string]any{"account": ids.Account, "folder": sourceFolder, "thread_key": ids.ThreadKey}
		if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
			params["uids"] = uids
			delete(params, "thread_key")
			res, err := a.sidecar.Call("messages.delete", params)
			if err != nil {
				return nil, err
			}
			return withMovedThreadLocation(res, ids, "trash"), nil
		}
		if ids.ThreadKey == "" && ids.UID > 0 {
			params = map[string]any{"account": ids.Account, "folder": sourceFolder, "uid": ids.UID}
		}
		res, err := a.sidecar.Call("messages.delete", params)
		if err != nil {
			return nil, err
		}
		return withMovedThreadLocation(res, ids, "trash"), nil
	}
	return map[string]any{"ok": true, "deleted": 0}, nil
}

func (a *App) mailMove(payload map[string]any) (any, error) {
	threadID, _ := payload["thread_id"].(string)
	targetFolder, _ := payload["target_folder_id"].(string)
	if threadID == "" || targetFolder == "" {
		return map[string]any{"ok": true}, nil
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, _, ok := parseRSSThreadID(threadID); ok {
		return nil, errors.New("feed items cannot be moved between folders")
	}
	if ids, ok := parseImapThreadID(threadID); ok {
		if canonThreadFolder(ids.Folder) == canonThreadFolder(targetFolder) {
			return map[string]any{"ok": true, "moved": 0}, nil
		}
		// Branch-compound thread keys pass through untouched; the sidecar
		// splits them and scopes the move to the subject branch.
		params := map[string]any{"account": ids.Account, "folder": ids.Folder, "target_folder": targetFolder, "thread_key": ids.ThreadKey}
		if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
			params["uids"] = uids
			delete(params, "thread_key")
			return a.sidecar.Call("messages.move", params)
		}
		if ids.ThreadKey == "" && ids.UID > 0 {
			params = map[string]any{"account": ids.Account, "folder": ids.Folder, "target_folder": targetFolder, "uid": ids.UID}
		}
		return a.sidecar.Call("messages.move", params)
	}
	return map[string]any{"ok": true}, nil
}

func (a *App) mailCopy(payload map[string]any) (any, error) {
	threadID, _ := payload["thread_id"].(string)
	targetAccount, _ := payload["target_account_id"].(string)
	targetFolder, _ := payload["target_folder_id"].(string)
	if threadID == "" || targetAccount == "" || targetFolder == "" {
		return map[string]any{"ok": true}, nil
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, _, ok := parseRSSThreadID(threadID); ok {
		return nil, errors.New("feed items cannot be copied between folders")
	}
	if ids, ok := parseImapThreadID(threadID); ok {
		// Branch-compound thread keys pass through untouched; the sidecar
		// splits them and scopes the copy to the subject branch.
		params := map[string]any{
			"account":        ids.Account,
			"folder":         ids.Folder,
			"target_account": targetAccount,
			"target_folder":  targetFolder,
			"thread_key":     ids.ThreadKey,
		}
		if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
			params["uids"] = uids
			delete(params, "thread_key")
			return a.sidecar.Call("messages.copy", params)
		}
		if ids.ThreadKey == "" && ids.UID > 0 {
			params = map[string]any{
				"account":        ids.Account,
				"folder":         ids.Folder,
				"target_account": targetAccount,
				"target_folder":  targetFolder,
				"uid":            ids.UID,
			}
		}
		res, err := a.sidecar.Call("messages.copy", params)
		if err != nil {
			return nil, err
		}
		out, _ := res.(map[string]any)
		if out == nil {
			out = map[string]any{"ok": true}
		}
		if ids.ThreadKey != "" {
			out["target_thread_id"] = formatImapThreadID(targetAccount, targetFolder, ids.ThreadKey)
		}
		return out, nil
	}
	return map[string]any{"ok": true}, nil
}

func (a *App) mailArchive(payload map[string]any) (any, error) {
	threadID, _ := payload["thread_id"].(string)
	if threadID == "" {
		return map[string]any{"ok": true}, nil
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, _, ok := parseRSSThreadID(threadID); ok {
		return nil, errors.New("feed items cannot be archived")
	}
	ids, ok := parseImapThreadID(threadID)
	if !ok {
		return map[string]any{"ok": true}, nil
	}
	res, err := a.sidecar.Call("folders.archive", map[string]any{"account": ids.Account})
	if err != nil {
		return nil, err
	}
	obj, _ := res.(map[string]any)
	targetFolder, _ := obj["folder"].(string)
	if targetFolder == "" {
		return nil, errors.New("Archive folder not found for this account")
	}
	payload["target_folder_id"] = targetFolder
	moveRes, err := a.mailMove(payload)
	if err != nil {
		return nil, err
	}
	out, _ := moveRes.(map[string]any)
	if out == nil {
		out = map[string]any{"ok": true}
	}
	out["folder"] = targetFolder
	out["thread_id"] = formatParsedImapThreadIDInFolder(ids, targetFolder)
	return out, nil
}

func (a *App) markRead(payload map[string]any) (any, error) {
	threadID, _ := payload["thread_id"].(string)
	if threadID == "" {
		return map[string]any{"ok": true}, nil
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"ok": true}, nil
	}
	// Defaults to read; pass seen:false to mark a thread unread.
	seen := true
	if v, ok := payload["seen"].(bool); ok {
		seen = v
	}
	if _, _, ok := parseRSSThreadID(threadID); ok {
		if itemKeys := rssItemKeysFromPayload(threadID, payload); len(itemKeys) > 0 {
			return a.sidecar.Call("rss.markRead", map[string]any{"thread_id": threadID, "item_keys": itemKeys, "seen": seen})
		}
		return a.sidecar.Call("rss.markRead", map[string]any{"thread_id": threadID, "seen": seen})
	}
	if ids, ok := parseImapThreadID(threadID); ok {
		// Branch-compound thread keys pass through untouched; the sidecar
		// splits them and marks only the subject branch (both on the server
		// and in the local store).
		params := map[string]any{"account": ids.Account, "folder": ids.Folder, "thread_key": ids.ThreadKey, "seen": seen}
		if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
			params["uids"] = uids
			delete(params, "thread_key")
			return a.sidecar.Call("messages.markRead", params)
		}
		if ids.ThreadKey == "" && ids.UID > 0 {
			params = map[string]any{"account": ids.Account, "folder": ids.Folder, "uid": ids.UID, "seen": seen}
		}
		return a.sidecar.Call("messages.markRead", params)
	}
	return map[string]any{"ok": true}, nil
}

func (a *App) markStarred(payload map[string]any) (any, error) {
	threadID, _ := payload["thread_id"].(string)
	if threadID == "" {
		return map[string]any{"ok": true}, nil
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"ok": true}, nil
	}
	starred := true
	if v, ok := payload["starred"].(bool); ok {
		starred = v
	}
	if _, _, ok := parseRSSThreadID(threadID); ok {
		if itemKeys := rssItemKeysFromPayload(threadID, payload); len(itemKeys) > 0 {
			return a.sidecar.Call("rss.markStarred", map[string]any{"thread_id": threadID, "item_keys": itemKeys, "starred": starred})
		}
		return a.sidecar.Call("rss.markStarred", map[string]any{"thread_id": threadID, "starred": starred})
	}
	if ids, ok := parseImapThreadID(threadID); ok {
		// Branch-compound thread keys pass through untouched; the sidecar
		// splits them and stars only the subject branch (both on the server
		// and in the local store).
		params := map[string]any{"account": ids.Account, "folder": ids.Folder, "thread_key": ids.ThreadKey, "starred": starred}
		if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
			params["uids"] = uids
			delete(params, "thread_key")
			return a.sidecar.Call("messages.markStarred", params)
		}
		if ids.ThreadKey == "" && ids.UID > 0 {
			params = map[string]any{"account": ids.Account, "folder": ids.Folder, "uid": ids.UID, "starred": starred}
		}
		return a.sidecar.Call("messages.markStarred", params)
	}
	return map[string]any{"ok": true}, nil
}

// markAllRead marks every message in one mail folder as read. RSS accounts have
// no folder-wide flag, so the frontend marks those threads individually and this
// path is a no-op for them.
func (a *App) markAllRead(payload map[string]any) (any, error) {
	accountID, _ := payload["account_id"].(string)
	folderID, _ := payload["folder_id"].(string)
	if folderID == "" {
		folderID = "inbox"
	}
	if a.sidecar == nil || !a.sidecar.Started() || accountID == "" {
		return map[string]any{"ok": true}, nil
	}
	if isRSSAccountID(accountID) {
		return map[string]any{"ok": true}, nil
	}
	if accountID == "unified" {
		return a.sidecar.Call("messages.markAllReadUnified", map[string]any{"folder": folderID})
	}
	return a.sidecar.Call("messages.markAllRead", map[string]any{
		"account": accountID,
		"folder":  folderID,
	})
}

// saveAttachment copies an attachment the sidecar already wrote under the media
// dir (served at /media/<key>) to a user-chosen path via a native save dialog.
// The key is path-cleaned and confined to the media root to block traversal.
func (a *App) saveAttachment(payload map[string]any) (any, error) {
	key, _ := payload["key"].(string)
	filename, _ := payload["filename"].(string)
	if key == "" {
		return nil, errors.New("missing attachment key")
	}

	root := mediaDir()
	// Cleaning a rooted "/"+key strips any "..", then Join confines it to root.
	src := filepath.Join(root, filepath.Clean("/"+key))
	if rel, err := filepath.Rel(root, src); err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(os.PathSeparator)) {
		return nil, errors.New("invalid attachment key")
	}
	if filename == "" {
		filename = filepath.Base(src)
	}

	dest, err := wailsRuntime.SaveFileDialog(a.ctx, wailsRuntime.SaveDialogOptions{
		Title:                "Save attachment",
		DefaultFilename:      filename,
		CanCreateDirectories: true,
	})
	if err != nil {
		return nil, err
	}
	if dest == "" {
		return map[string]any{"saved": false}, nil // user cancelled
	}

	in, err := os.Open(src)
	if err != nil {
		return nil, fmt.Errorf("open attachment: %w", err)
	}
	defer in.Close()
	out, err := os.Create(dest)
	if err != nil {
		return nil, fmt.Errorf("create destination: %w", err)
	}
	if _, err := io.Copy(out, in); err != nil {
		out.Close()
		return nil, fmt.Errorf("write attachment: %w", err)
	}
	if err := out.Close(); err != nil {
		return nil, fmt.Errorf("write attachment: %w", err)
	}
	return map[string]any{"saved": true, "path": dest}, nil
}

// readAttachment reads an attachment the sidecar already wrote under the media
// dir (served at /media/<key>) and returns it base64-encoded, for pulling a
// stored attachment back into the composer (e.g. "Edit as New Message"). The key
// is path-cleaned and confined to the media root to block traversal.
func (a *App) readAttachment(payload map[string]any) (any, error) {
	key, _ := payload["key"].(string)
	if key == "" {
		return nil, errors.New("missing attachment key")
	}

	root := mediaDir()
	// Cleaning a rooted "/"+key strips any "..", then Join confines it to root.
	src := filepath.Join(root, filepath.Clean("/"+key))
	if rel, err := filepath.Rel(root, src); err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(os.PathSeparator)) {
		return nil, errors.New("invalid attachment key")
	}

	data, err := os.ReadFile(src)
	if err != nil {
		return nil, fmt.Errorf("read attachment: %w", err)
	}
	mime := http.DetectContentType(data)
	return map[string]any{
		"data": base64.StdEncoding.EncodeToString(data),
		"mime": mime,
		"size": len(data),
	}, nil
}

// deleteFolder lets delete callers override the thread id's nominal folder with
// the message/thread row's current folder. Thread ids can outlive a move, and a
// UID/thread key is only meaningful within its source mailbox.
func deleteFolder(payload map[string]any, fallback string) string {
	if folder, ok := payload["folder"].(string); ok && folder != "" {
		return folder
	}
	return fallback
}

func imapUIDsFromPayload(threadID string, payload map[string]any) []uint32 {
	raw, _ := payload["message_ids"].([]any)
	uids := make([]uint32, 0, len(raw))
	prefix := threadID + "#"
	for _, item := range raw {
		messageID, _ := item.(string)
		uidPart := messageID
		if rest, ok := strings.CutPrefix(messageID, prefix); ok {
			uidPart = rest
		}
		uid, err := strconv.ParseUint(uidPart, 10, 32)
		if err == nil {
			uids = append(uids, uint32(uid))
			continue
		}
		if ids, ok := parseImapThreadID(messageID); ok && ids.UID > 0 {
			uids = append(uids, ids.UID)
		}
	}
	return uids
}

func (a *App) mailSend(payload map[string]any) (any, error) {
	var req SendMailRequest
	if err := decode(payload, &req); err != nil {
		return nil, err
	}
	if req.To == "" {
		return nil, errors.New("invalid message")
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, err := a.sidecar.Call("send", map[string]any{
		"account":     req.AccountID,
		"to":          req.To,
		"cc":          req.Cc,
		"bcc":         req.Bcc,
		"subject":     req.Subject,
		"body":        req.Body,
		"html":        req.Html,
		"in_reply_to": req.InReplyTo,
		"references":  req.References,
		"reply_to":    req.ReplyTo,
		"from":        req.From,
		"message_id":  req.MessageID,
		"attachments": req.Attachments,
	}); err != nil {
		return nil, err
	}
	return map[string]any{"ok": true, "queued": false}, nil
}

func (a *App) mailSaveDraft(payload map[string]any) (any, error) {
	var req SendMailRequest
	if err := decode(payload, &req); err != nil {
		return nil, err
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, err := a.sidecar.Call("save_draft", map[string]any{
		"account":     req.AccountID,
		"to":          req.To,
		"cc":          req.Cc,
		"bcc":         req.Bcc,
		"subject":     req.Subject,
		"body":        req.Body,
		"html":        req.Html,
		"in_reply_to": req.InReplyTo,
		"references":  req.References,
		"reply_to":    req.ReplyTo,
		"from":        req.From,
		"draft_id":    req.DraftID,
		"attachments": req.Attachments,
	}); err != nil {
		return nil, err
	}
	return map[string]any{"ok": true}, nil
}

func (a *App) mailDiscardDraft(payload map[string]any) (any, error) {
	accountID, _ := payload["account_id"].(string)
	draftID, _ := payload["draft_id"].(string)
	if accountID == "" || draftID == "" {
		return map[string]any{"ok": true, "deleted": 0}, nil
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	params := map[string]any{
		"account":  accountID,
		"draft_id": draftID,
	}
	if threadID, _ := payload["thread_id"].(string); threadID != "" {
		if ids, ok := parseImapThreadID(threadID); ok && ids.Account == accountID {
			params["thread_key"] = ids.ThreadKey
			if ids.UID > 0 && ids.ThreadKey == "" {
				params["thread_key"] = fmt.Sprintf("uid:%d", ids.UID)
			}
		}
	}
	return a.sidecar.Call("discard_draft", params)
}
