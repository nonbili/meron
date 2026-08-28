package main

import (
	"encoding/json"
	"errors"
	"strings"
)

func (a *App) accountAddPassword(payload map[string]any) (any, error) {
	var req AddPasswordAccountRequest
	if err := decode(payload, &req); err != nil {
		return nil, err
	}
	if !strings.Contains(req.Email, "@") {
		return nil, errors.New("invalid email")
	}
	if req.IMAPHost == "" || req.SMTPHost == "" {
		return nil, errors.New("server required")
	}
	if req.Username == "" || req.Password == "" {
		return nil, errors.New("credentials required")
	}
	if req.IMAPPort == 0 {
		req.IMAPPort = 993
	}
	if req.SMTPPort == 0 {
		req.SMTPPort = 465
	}
	id := accountID(req.Email)
	a.logf("account.addPassword: connecting account=%s imap=%s:%d smtp=%s:%d", id, req.IMAPHost, req.IMAPPort, req.SMTPHost, req.SMTPPort)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	legacyTLS := true
	if req.TLS != nil {
		legacyTLS = *req.TLS
	}
	var imapExplicitTLS *bool
	if req.StartTLS != nil {
		imapExplicitTLS = req.TLS
	}
	imapTLS, imapStartTLS := selectedTLSMode(legacyTLS, imapExplicitTLS, req.StartTLS, req.IMAPPort)
	smtpTLS, smtpStartTLS := selectedTLSMode(legacyTLS, req.SMTPTLS, req.SMTPStartTLS, req.SMTPPort)
	connect := map[string]any{
		"id":            id,
		"host":          req.IMAPHost,
		"port":          req.IMAPPort,
		"user":          req.Username,
		"password":      req.Password,
		"tls":           imapTLS,
		"starttls":      imapStartTLS,
		"smtp_host":     req.SMTPHost,
		"smtp_port":     req.SMTPPort,
		"smtp_tls":      smtpTLS,
		"smtp_starttls": smtpStartTLS,
		"email":         req.Email,
		"display_name":  req.DisplayName,
		"sender_name":   req.SenderName,
		"provider":      "custom",
	}
	if pin := strings.TrimSpace(req.CertPin); pin != "" {
		connect["cert_pin"] = pin
	}
	if pin := strings.TrimSpace(req.SMTPCertPin); pin != "" {
		connect["smtp_cert_pin"] = pin
	}
	if _, err := a.sidecar.Call("account.connect", connect); err != nil {
		return nil, err
	}
	_, _ = a.sidecar.Call("watch.start", map[string]any{"account": id})

	account := Account{
		ID:                id,
		Email:             req.Email,
		DisplayName:       req.DisplayName,
		SenderName:        req.SenderName,
		Provider:          "custom",
		AuthType:          "password",
		IncludedInUnified: true,
		IMAPHost:          req.IMAPHost,
		IMAPPort:          req.IMAPPort,
		SMTPHost:          req.SMTPHost,
		SMTPPort:          req.SMTPPort,
		TLS:               imapTLS,
		StartTLS:          imapStartTLS,
		SMTPTLS:           smtpTLS,
		SMTPStartTLS:      smtpStartTLS,
	}
	return map[string]any{"account": account}, nil
}

// accountProbeCert fetches the certificate a mail server presents so the
// account dialog can show it and let the user pin it. Servers with a
// self-signed certificate (Proton Mail Bridge on 127.0.0.1, for one) are
// unreachable until their exact certificate is accepted.
func (a *App) accountProbeCert(payload map[string]any) (any, error) {
	var req struct {
		Host     string `json:"host"`
		Port     uint16 `json:"port"`
		Protocol string `json:"protocol"`
		StartTLS bool   `json:"starttls"`
		// The account's proxy choice, passed through untyped like everywhere
		// else. Absent follows the app-wide proxy, which is also what a
		// connection with no account override does.
		Proxy any `json:"proxy"`
	}
	if err := decode(payload, &req); err != nil {
		return nil, err
	}
	if strings.TrimSpace(req.Host) == "" {
		return nil, errors.New("server required")
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	return a.sidecar.Call("account.probeCert", map[string]any{
		"host":     req.Host,
		"port":     req.Port,
		"protocol": req.Protocol,
		"starttls": req.StartTLS,
		"proxy":    req.Proxy,
	})
}

// accountSetCertPin stores a certificate the user accepted for an account that
// already exists — a send refused by a submission server whose certificate
// cannot be validated, say. Only the server named by the payload is pinned; an
// omitted key leaves that server's stored pin alone.
func (a *App) accountSetCertPin(payload map[string]any) (any, error) {
	var req struct {
		ID          string `json:"id"`
		CertPin     string `json:"cert_pin"`
		SMTPCertPin string `json:"smtp_cert_pin"`
	}
	if err := decode(payload, &req); err != nil {
		return nil, err
	}
	if strings.TrimSpace(req.ID) == "" {
		return nil, errors.New("account required")
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	params := map[string]any{"id": req.ID}
	if pin := strings.TrimSpace(req.CertPin); pin != "" {
		params["cert_pin"] = pin
	}
	if pin := strings.TrimSpace(req.SMTPCertPin); pin != "" {
		params["smtp_cert_pin"] = pin
	}
	return a.sidecar.Call("account.setCertPin", params)
}

func selectedTLSMode(legacyTLS bool, explicitTLS, explicitStartTLS *bool, port uint16) (bool, bool) {
	// A lone false STARTTLS flag does not identify the intended alternative;
	// retain legacy port inference unless its paired TLS flag is also present.
	if explicitTLS == nil && (explicitStartTLS == nil || !*explicitStartTLS) {
		return tlsMode(legacyTLS, port)
	}
	tls := legacyTLS
	if explicitTLS != nil {
		tls = *explicitTLS
	}
	starttls := explicitStartTLS != nil && *explicitStartTLS
	if starttls {
		tls = false
	}
	return tls, starttls
}

func (a *App) accountRemove(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	// The unified DB is the single source of truth: the sidecar deletes the
	// account row and cascades its cached state (mail folders/messages or rss
	// subscriptions/items), plus the keychain secret.
	if a.sidecar != nil && a.sidecar.Started() {
		if _, err := a.sidecar.Call("account.remove", map[string]any{"id": id}); err != nil {
			a.logf("account.remove: sidecar cleanup failed for %s: %v", id, err)
		}
	}
	return map[string]any{"ok": true}, nil
}

func (a *App) accountSetImages(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	enabled, _ := payload["enabled"].(bool)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	if _, err := a.sidecar.Call("account.setImages", map[string]any{"account": id, "enabled": enabled}); err != nil {
		return nil, err
	}
	a.logf("account.setImages: account=%s enabled=%t", id, enabled)
	return map[string]any{"ok": true}, nil
}

func (a *App) accountReorder(payload map[string]any) (any, error) {
	accounts, _ := payload["accounts"].([]any)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	if _, err := a.sidecar.Call("account.reorder", map[string]any{"accounts": accounts}); err != nil {
		return nil, err
	}
	return map[string]any{"ok": true}, nil
}

// accountSetProxy points one account at its own proxy, at the app-wide one, or
// at a direct connection. The shape of "proxy" is validated in the engine.
func (a *App) accountSetProxy(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	proxy, _ := payload["proxy"].(map[string]any)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	if _, err := a.sidecar.Call("account.setProxy", map[string]any{"account": id, "proxy": proxy}); err != nil {
		return nil, err
	}
	// Credentials are never logged; the mode alone is enough to debug routing.
	a.logf("account.setProxy: account=%s mode=%v", id, proxy["mode"])
	return map[string]any{"ok": true}, nil
}

func (a *App) accountSetName(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	name, _ := payload["name"].(string)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	if _, err := a.sidecar.Call("account.setName", map[string]any{"account": id, "name": name}); err != nil {
		return nil, err
	}
	a.logf("account.setName: account=%s name=%s", id, name)
	return map[string]any{"ok": true}, nil
}

func (a *App) accountSetSenderName(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	name, _ := payload["name"].(string)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	if _, err := a.sidecar.Call("account.setSenderName", map[string]any{"account": id, "name": name}); err != nil {
		return nil, err
	}
	a.logf("account.setSenderName: account=%s name=%s", id, name)
	return map[string]any{"ok": true}, nil
}

func (a *App) accountSetAvatar(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	avatarURL, _ := payload["avatar_url"].(string)
	avatarURL = a.downloadAndSaveAvatar(id, avatarURL)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	if _, err := a.sidecar.Call("account.setAvatar", map[string]any{"account": id, "avatar_url": strings.TrimSpace(avatarURL)}); err != nil {
		return nil, err
	}
	a.logf("account.setAvatar: account=%s", id)
	return map[string]any{"ok": true}, nil
}

// accountSetAliases replaces the whole send-as alias list for an account. The
// sidecar normalizes entries (trim, drop blank emails, dedupe).
func (a *App) accountSetAliases(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	aliases := payload["aliases"]
	if aliases == nil {
		aliases = []any{}
	}
	if _, err := a.sidecar.Call("account.setAliases", map[string]any{"account": id, "aliases": aliases}); err != nil {
		return nil, err
	}
	a.logf("account.setAliases: account=%s", id)
	return map[string]any{"ok": true}, nil
}

// accountSetSignature sets or clears one account's signature override. A nil
// "signature" clears it, so the account follows the app-wide signature; the
// engine validates the shape.
func (a *App) accountSetSignature(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	signature, _ := payload["signature"].(map[string]any)
	if _, err := a.sidecar.Call("account.setSignature", map[string]any{"account": id, "signature": signature}); err != nil {
		return nil, err
	}
	// The signature text is user content; only the chosen mode is logged.
	a.logf("account.setSignature: account=%s mode=%v", id, signature["mode"])
	return map[string]any{"ok": true}, nil
}

// accountSetPref forwards a boolean per-account toggle (unified-inbox inclusion,
// mute, pause) to the sidecar, which persists it and applies any side effects
// (e.g. pausing stops the IDLE watcher). `method` is the sidecar method name.
func (a *App) accountSetPref(payload map[string]any, method string) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	enabled, _ := payload["enabled"].(bool)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	if _, err := a.sidecar.Call(method, map[string]any{"account": id, "enabled": enabled}); err != nil {
		return nil, err
	}
	a.logf("%s: account=%s enabled=%t", method, id, enabled)
	return map[string]any{"ok": true}, nil
}

func (a *App) accountSetSaveSentCopy(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	value, ok := payload["value"]
	if !ok {
		value = nil
	}
	switch value.(type) {
	case nil, bool:
	default:
		return nil, errors.New("value must be true, false, or null")
	}
	if _, err := a.sidecar.Call("account.setSaveSentCopy", map[string]any{"account": id, "value": value}); err != nil {
		return nil, err
	}
	a.logf("account.setSaveSentCopy: account=%s value=%v", id, value)
	return map[string]any{"ok": true}, nil
}

func (a *App) accountSetChatWallpaper(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	wallpaper, ok := payload["wallpaper"]
	if !ok {
		wallpaper = nil
	}
	if _, err := a.sidecar.Call("account.setChatWallpaper", map[string]any{"account": id, "wallpaper": wallpaper}); err != nil {
		return nil, err
	}
	a.logf("account.setChatWallpaper: account=%s", id)
	return map[string]any{"ok": true}, nil
}

func (a *App) accountSetRSSSyncInterval(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	minutes, ok := numberPayload(payload["minutes"])
	if !ok {
		return nil, errors.New("minutes required")
	}
	if minutes < 5 {
		minutes = 5
	} else if minutes > 1440 {
		minutes = 1440
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	if _, err := a.sidecar.Call("account.setRSSSyncInterval", map[string]any{"account": id, "minutes": minutes}); err != nil {
		return nil, err
	}
	a.logf("account.setRSSSyncInterval: account=%s minutes=%d", id, minutes)
	return map[string]any{"ok": true, "minutes": minutes}, nil
}

func numberPayload(v any) (int, bool) {
	switch n := v.(type) {
	case int:
		return n, true
	case int64:
		return int(n), true
	case float64:
		return int(n), true
	case float32:
		return int(n), true
	default:
		return 0, false
	}
}

func (a *App) suggestContacts(payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"contacts": []any{}}, nil
	}
	return a.sidecar.Call("contacts.suggest", payload)
}

// accountList returns every account (mail + rss) from the sidecar's unified DB,
// the single source of truth.
func (a *App) accountList() (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"accounts": []Account{}}, nil
	}
	res, err := a.sidecar.Call("account.list", map[string]any{})
	if err != nil {
		return nil, err
	}
	return map[string]any{"accounts": decodeAccounts(res)}, nil
}

func decodeAccount(raw any) Account {
	obj, _ := raw.(map[string]any)
	return accountFromMap(obj["account"])
}

func decodeAccounts(raw any) []Account {
	obj, _ := raw.(map[string]any)
	list, _ := obj["accounts"].([]any)
	out := make([]Account, 0, len(list))
	for _, item := range list {
		out = append(out, accountFromMap(item))
	}
	return out
}

func accountFromMap(item any) Account {
	var acc Account
	data, _ := json.Marshal(item)
	_ = json.Unmarshal(data, &acc)
	return acc
}
