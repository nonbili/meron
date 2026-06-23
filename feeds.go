package main

import (
	"errors"
	"fmt"
	"os"
	"strings"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

func (a *App) accountAddRSS(payload map[string]any) (any, error) {
	feedURL, _ := payload["feed_url"].(string)
	displayName, _ := payload["display_name"].(string)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	res, err := a.sidecar.Call("account.addRss", map[string]any{
		"feed_url":     feedURL,
		"display_name": displayName,
	})
	if err != nil {
		return nil, err
	}
	account := decodeAccount(res)
	a.logf("account.addRSS: saved feed account=%s", account.ID)
	return map[string]any{"account": account}, nil
}

func (a *App) feedAdd(payload map[string]any) (any, error) {
	accountID, _ := payload["account_id"].(string)
	feedURL, _ := payload["feed_url"].(string)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	res, err := a.sidecar.Call("feed.add", map[string]any{"account": accountID, "feed_url": feedURL})
	if err != nil {
		return nil, err
	}
	a.logf("rss.addFeed: added feed to account=%s", accountID)
	return res, nil
}

func (a *App) feedRemove(payload map[string]any) (any, error) {
	threadID, _ := payload["thread_id"].(string)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	res, err := a.sidecar.Call("feed.remove", map[string]any{"thread_id": threadID})
	if err != nil {
		return nil, err
	}
	a.logf("rss.removeFeed: removed feed thread=%s", threadID)
	return res, nil
}

func (a *App) feedMove(payload map[string]any) (any, error) {
	threadID, _ := payload["thread_id"].(string)
	targetAccount, _ := payload["target_account_id"].(string)
	if targetAccount == "" {
		targetAccount, _ = payload["target_account"].(string)
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	res, err := a.sidecar.Call("feed.move", map[string]any{"thread_id": threadID, "target_account": targetAccount})
	if err != nil {
		return nil, err
	}
	a.logf("rss.moveFeed: moved feed thread=%s target=%s", threadID, targetAccount)
	return res, nil
}

// exportOpml asks the sidecar for an OPML document of one RSS account's feeds,
// then writes it to a user-chosen path via a native save dialog.
func (a *App) exportOpml(payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	account, _ := payload["account"].(string)
	if account == "" {
		return nil, errors.New("account id required")
	}
	res, err := a.sidecar.Call("rss.exportOpml", map[string]any{"account": account})
	if err != nil {
		return nil, err
	}
	resMap, _ := res.(map[string]any)
	opml, _ := resMap["opml"].(string)
	if opml == "" {
		return nil, errors.New("no feeds to export")
	}

	dest, err := wailsRuntime.SaveFileDialog(a.ctx, wailsRuntime.SaveDialogOptions{
		Title:                "Export feeds",
		DefaultFilename:      "meron-feeds.opml",
		CanCreateDirectories: true,
		Filters: []wailsRuntime.FileFilter{
			{DisplayName: "OPML files (*.opml)", Pattern: "*.opml"},
		},
	})
	if err != nil {
		return nil, err
	}
	if dest == "" {
		return map[string]any{"saved": false}, nil // user cancelled
	}
	if err := os.WriteFile(dest, []byte(opml), 0o644); err != nil {
		return nil, fmt.Errorf("write OPML: %w", err)
	}
	a.logf("rss.exportOpml: wrote %s", dest)
	return map[string]any{"saved": true, "path": dest}, nil
}

// importOpml reads a user-chosen OPML file via a native open dialog and hands its
// contents to the sidecar, which adds the feeds to the given RSS account. Returns
// the number of feeds added.
func (a *App) importOpml(payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	account, _ := payload["account"].(string)
	if account == "" {
		return nil, errors.New("account id required")
	}
	src, err := wailsRuntime.OpenFileDialog(a.ctx, wailsRuntime.OpenDialogOptions{
		Title: "Import feeds",
		Filters: []wailsRuntime.FileFilter{
			{DisplayName: "OPML files (*.opml, *.xml)", Pattern: "*.opml;*.xml"},
		},
	})
	if err != nil {
		return nil, err
	}
	if src == "" {
		return map[string]any{"imported": 0, "cancelled": true}, nil // user cancelled
	}
	data, err := os.ReadFile(src)
	if err != nil {
		return nil, fmt.Errorf("read OPML: %w", err)
	}

	res, err := a.sidecar.Call("rss.importOpml", map[string]any{"opml": string(data), "account": account})
	if err != nil {
		return nil, err
	}
	resMap, _ := res.(map[string]any)
	imported := 0
	if n, ok := resMap["imported"].(float64); ok {
		imported = int(n)
	}
	a.logf("rss.importOpml: imported %d feeds from %s", imported, src)
	return map[string]any{"imported": imported}, nil
}

func rssItemKeysFromPayload(threadID string, payload map[string]any) []string {
	raw, _ := payload["message_ids"].([]any)
	itemKeys := make([]string, 0, len(raw))
	prefix := threadID + "#"
	for _, item := range raw {
		messageID, _ := item.(string)
		itemKey, ok := strings.CutPrefix(messageID, prefix)
		if ok && itemKey != "" {
			itemKeys = append(itemKeys, itemKey)
		}
	}
	return itemKeys
}

// isRSSAccountID reports whether an account id belongs to the RSS engine. RSS ids
// are minted with an "rss-" prefix (see the sidecar's rss module), so routing
// stays stateless — no per-call engine lookup in the bridge.
func isRSSAccountID(id string) bool {
	return strings.HasPrefix(id, "rss-")
}

// parseRSSThreadID splits an RSS thread id ("<account>#rss#<sub>[#item]").
func parseRSSThreadID(threadID string) (string, string, bool) {
	accountID, rest, ok := strings.Cut(threadID, "#rss#")
	if !ok || accountID == "" || rest == "" {
		return "", "", false
	}
	subID, _, _ := strings.Cut(rest, "#")
	if subID == "" {
		return "", "", false
	}
	return accountID, subID, true
}
