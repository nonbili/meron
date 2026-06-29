package main

import (
	"encoding/base64"
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
	res, err := a.sidecar.Call("messages.recent", map[string]any{
		"account":       req.AccountID,
		"folder":        req.FolderID,
		"query":         req.Query,
		"filter":        req.Filter,
		"before_cursor": req.BeforeCursor,
		"limit":         50,
		"refresh":       req.Refresh,
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
			if list, ok := obj["messages"].([]any); ok {
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

// starredItems returns every starred item across all accounts as a flat list of
// per-message cards. Mail rows arrive raw and are shaped here (thread/message
// id minting lives in the bridge); RSS rows arrive final-shaped from the engine.
func (a *App) starredItems(payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"items": []Message{}}, nil
	}
	res, err := a.sidecar.Call("starred.items", payload)
	if err != nil {
		return nil, err
	}
	object, _ := res.(map[string]any)
	mailRows, _ := object["mail"].([]any)
	rssRows, _ := object["rss"].([]any)
	items := make([]any, 0, len(mailRows)+len(rssRows))
	for _, item := range mailRows {
		msg, _ := item.(map[string]any)
		uid := jsonNumber(msg["uid"])
		if uid <= 0 {
			continue
		}
		accountID := jsonString(msg["account"])
		folder := jsonString(msg["folder"])
		threadKey := jsonString(msg["thread_key"])
		if threadKey == "" {
			threadKey = fmt.Sprintf("uid:%d", uid)
		}
		compoundKey := threadKey
		if shouldBranchThreadBySubject(threadKey) {
			compoundKey = threadKey + "#" + threadGroupingSubject(jsonString(msg["subject"]))
		}
		// IDs must match threadsJSON/threadMessagesJSON exactly so opening the
		// thread and scrolling to the message line up with the conversation view.
		threadID := formatImapThreadID(accountID, folder, compoundKey)
		items = append(items, Message{
			ID:        fmt.Sprintf("%s#%d", threadID, uid),
			AccountID: accountID,
			FolderID:  folder,
			ThreadID:  threadID,
			FromName:  jsonString(msg["from_name"]),
			FromAddr:  jsonString(msg["from_addr"]),
			Subject:   jsonString(msg["subject"]),
			Date:      jsonNumber(msg["date"]),
			Unread:    !jsonBool(msg["seen"]),
			Starred:   true,
		})
	}
	items = append(items, rssRows...)
	return map[string]any{"items": items}, nil
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
		realThreadKey := ids.ThreadKey
		subjectFilter := ""
		if strings.Contains(ids.ThreadKey, "#") {
			parts := strings.SplitN(ids.ThreadKey, "#", 2)
			realThreadKey = parts[0]
			subjectFilter = parts[1]
		}
		params := map[string]any{"account": ids.Account, "folder": ids.Folder, "thread_key": realThreadKey}
		if realThreadKey == "" && ids.UID > 0 {
			method = "messages.read"
			params = map[string]any{"account": ids.Account, "folder": ids.Folder, "uid": ids.UID}
		}
		if method == "messages.thread" {
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
			return threadMessagesJSON(ids.Account, threadID, ids.Folder, res, subjectFilter), nil
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
		if strings.Contains(ids.ThreadKey, "#") {
			parts := strings.SplitN(ids.ThreadKey, "#", 2)
			realThreadKey := parts[0]
			subjectFilter := parts[1]
			if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
				res, err := a.sidecar.Call("messages.delete", map[string]any{"account": ids.Account, "folder": sourceFolder, "uids": uids})
				if err != nil {
					return nil, err
				}
				return withMovedThreadLocation(res, ids, "trash"), nil
			}
			res, err := a.sidecar.Call("messages.threadHeaders", map[string]any{
				"account":    ids.Account,
				"folder":     sourceFolder,
				"thread_key": realThreadKey,
			})
			if err != nil {
				return nil, err
			}
			uidsByFolder := uidsByFolderForSubjectThread(res, sourceFolder, subjectFilter)
			if len(uidsByFolder) == 0 {
				return map[string]any{"ok": true, "deleted": 0}, nil
			}
			deleted := 0
			trashFolder := ""
			permanent := false
			for msgFolder, uids := range uidsByFolder {
				res, err := a.sidecar.Call("messages.delete", map[string]any{"account": ids.Account, "folder": msgFolder, "uids": uids})
				if err != nil {
					return nil, err
				}
				if obj, ok := res.(map[string]any); ok {
					deleted += int(jsonNumber(obj["deleted"]))
					if trash := jsonString(obj["trash"]); trash != "" {
						trashFolder = trash
					}
					if jsonBool(obj["permanent"]) {
						permanent = true
					}
				}
			}
			out := map[string]any{"ok": true, "deleted": deleted}
			if trashFolder != "" {
				out["trash"] = trashFolder
				out["thread_id"] = formatParsedImapThreadIDInFolder(ids, trashFolder)
			}
			if permanent {
				out["permanent"] = true
			}
			return out, nil
		}
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
		if strings.Contains(ids.ThreadKey, "#") {
			parts := strings.SplitN(ids.ThreadKey, "#", 2)
			realThreadKey := parts[0]
			subjectFilter := parts[1]
			if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
				return a.sidecar.Call("messages.move", map[string]any{"account": ids.Account, "folder": ids.Folder, "target_folder": targetFolder, "uids": uids})
			}
			res, err := a.sidecar.Call("messages.threadHeaders", map[string]any{
				"account":    ids.Account,
				"folder":     ids.Folder,
				"thread_key": realThreadKey,
			})
			if err != nil {
				return nil, err
			}
			uidsByFolder := uidsByFolderForSubjectThread(res, ids.Folder, subjectFilter)
			moved := 0
			for msgFolder, uids := range uidsByFolder {
				if msgFolder == targetFolder || len(uids) == 0 {
					continue
				}
				res, err := a.sidecar.Call("messages.move", map[string]any{"account": ids.Account, "folder": msgFolder, "target_folder": targetFolder, "uids": uids})
				if err != nil {
					return nil, err
				}
				if obj, ok := res.(map[string]any); ok {
					moved += int(jsonNumber(obj["moved"]))
				}
			}
			return map[string]any{"ok": true, "moved": moved}, nil
		}
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
		if strings.Contains(ids.ThreadKey, "#") {
			parts := strings.SplitN(ids.ThreadKey, "#", 2)
			realThreadKey := parts[0]
			subjectFilter := parts[1]
			if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
				return a.sidecar.Call("messages.copy", map[string]any{
					"account":        ids.Account,
					"folder":         ids.Folder,
					"target_account": targetAccount,
					"target_folder":  targetFolder,
					"uids":           uids,
				})
			}
			res, err := a.sidecar.Call("messages.threadHeaders", map[string]any{
				"account":    ids.Account,
				"folder":     ids.Folder,
				"thread_key": realThreadKey,
			})
			if err != nil {
				return nil, err
			}
			uidsByFolder := uidsByFolderForSubjectThread(res, ids.Folder, subjectFilter)
			copied := 0
			for msgFolder, uids := range uidsByFolder {
				if len(uids) == 0 {
					continue
				}
				res, err := a.sidecar.Call("messages.copy", map[string]any{
					"account":        ids.Account,
					"folder":         msgFolder,
					"target_account": targetAccount,
					"target_folder":  targetFolder,
					"uids":           uids,
				})
				if err != nil {
					return nil, err
				}
				if obj, ok := res.(map[string]any); ok {
					copied += int(jsonNumber(obj["copied"]))
				}
			}
			return map[string]any{
				"ok":               true,
				"copied":           copied,
				"target_account":   targetAccount,
				"target_folder":    targetFolder,
				"target_thread_id": formatImapThreadID(targetAccount, targetFolder, ids.ThreadKey),
			}, nil
		}
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

func uidsByFolderForSubjectThread(res any, fallbackFolder, subjectFilter string) map[string][]uint32 {
	uidsByFolder := make(map[string][]uint32)
	if obj, ok := res.(map[string]any); ok {
		if list, ok := obj["headers"].([]any); ok {
			for _, item := range list {
				entry, _ := item.(map[string]any)
				normSub := threadGroupingSubject(jsonString(entry["subject"]))
				if normSub == subjectFilter {
					uid := uint32(jsonNumber(entry["uid"]))
					if uid > 0 {
						msgFolder := jsonString(entry["folder"])
						if msgFolder == "" {
							msgFolder = fallbackFolder
						}
						uidsByFolder[msgFolder] = append(uidsByFolder[msgFolder], uid)
					}
				}
			}
		}
		if list, ok := obj["messages"].([]any); ok {
			for _, item := range list {
				entry, _ := item.(map[string]any)
				msg, _ := entry["message"].(map[string]any)
				normSub := threadGroupingSubject(jsonString(msg["subject"]))
				if normSub == subjectFilter {
					uid := uint32(jsonNumber(entry["uid"]))
					if uid > 0 {
						msgFolder := jsonString(entry["folder"])
						if msgFolder == "" {
							msgFolder = fallbackFolder
						}
						uidsByFolder[msgFolder] = append(uidsByFolder[msgFolder], uid)
					}
				}
			}
		}
	}
	return uidsByFolder
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
		if strings.Contains(ids.ThreadKey, "#") {
			parts := strings.SplitN(ids.ThreadKey, "#", 2)
			realThreadKey := parts[0]
			subjectFilter := parts[1]
			if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
				params := map[string]any{"account": ids.Account, "folder": ids.Folder, "uids": uids, "seen": seen}
				return a.sidecar.Call("messages.markRead", params)
			}
			res, err := a.sidecar.Call("messages.thread", map[string]any{
				"account":    ids.Account,
				"folder":     ids.Folder,
				"thread_key": realThreadKey,
			})
			if err != nil {
				return nil, err
			}
			var uids []uint32
			if obj, ok := res.(map[string]any); ok {
				if list, ok := obj["messages"].([]any); ok {
					for _, item := range list {
						entry, _ := item.(map[string]any)
						// Only touch messages whose flag differs from the target.
						if jsonBool(entry["seen"]) == seen {
							continue
						}
						msg, _ := entry["message"].(map[string]any)
						normSub := threadGroupingSubject(jsonString(msg["subject"]))
						if normSub == subjectFilter {
							uid := uint32(jsonNumber(entry["uid"]))
							if uid > 0 {
								uids = append(uids, uid)
							}
						}
					}
				}
			}
			if len(uids) == 0 {
				return map[string]any{"ok": true}, nil
			}
			params := map[string]any{
				"account": ids.Account,
				"folder":  ids.Folder,
				"uids":    uids,
				"seen":    seen,
			}
			return a.sidecar.Call("messages.markRead", params)
		}
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
		if strings.Contains(ids.ThreadKey, "#") {
			parts := strings.SplitN(ids.ThreadKey, "#", 2)
			realThreadKey := parts[0]
			subjectFilter := parts[1]
			if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
				params := map[string]any{"account": ids.Account, "folder": ids.Folder, "uids": uids, "starred": starred}
				return a.sidecar.Call("messages.markStarred", params)
			}
			res, err := a.sidecar.Call("messages.thread", map[string]any{
				"account":    ids.Account,
				"folder":     ids.Folder,
				"thread_key": realThreadKey,
			})
			if err != nil {
				return nil, err
			}
			uidsByFolder := make(map[string][]uint32)
			if obj, ok := res.(map[string]any); ok {
				if list, ok := obj["messages"].([]any); ok {
					for _, item := range list {
						entry, _ := item.(map[string]any)
						if jsonBool(entry["starred"]) == starred {
							continue
						}
						msg, _ := entry["message"].(map[string]any)
						normSub := threadGroupingSubject(jsonString(msg["subject"]))
						if normSub == subjectFilter {
							uid := uint32(jsonNumber(entry["uid"]))
							if uid > 0 {
								msgFolder := jsonString(entry["folder"])
								if msgFolder == "" {
									msgFolder = ids.Folder
								}
								uidsByFolder[msgFolder] = append(uidsByFolder[msgFolder], uid)
							}
						}
					}
				}
			}
			if len(uidsByFolder) == 0 {
				return map[string]any{"ok": true}, nil
			}
			for msgFolder, uids := range uidsByFolder {
				params := map[string]any{
					"account": ids.Account,
					"folder":  msgFolder,
					"uids":    uids,
					"starred": starred,
				}
				if _, err := a.sidecar.Call("messages.markStarred", params); err != nil {
					return nil, err
				}
			}
			return map[string]any{"ok": true}, nil
		}
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
	return a.sidecar.Call("discard_draft", map[string]any{
		"account":  accountID,
		"draft_id": draftID,
	})
}
