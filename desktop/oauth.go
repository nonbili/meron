package main

import (
	"context"
	"crypto/sha256"
	"encoding/base64"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"strings"
	"sync/atomic"
	"time"

	wailsRuntime "meron/internal/desktopruntime"
)

// oauthMailbox is where a provider's OAuth account connects.
type oauthMailbox struct {
	label    string
	authType string
	imapHost string
	smtpHost string
	smtpPort uint16
	// SMTP over implicit TLS (Gmail) or STARTTLS (Outlook).
	smtpTLS bool
}

var oauthMailboxes = map[string]oauthMailbox{
	"gmail":   {label: "Google", authType: "gmail_oauth", imapHost: "imap.gmail.com", smtpHost: "smtp.gmail.com", smtpPort: 465, smtpTLS: true},
	"outlook": {label: "Outlook", authType: "outlook_oauth", imapHost: "outlook.office365.com", smtpHost: "smtp-mail.outlook.com", smtpPort: 587},
}

func (a *App) accountAddGmailOAuth(payload map[string]any) (any, error) {
	if googleClientID() == "" {
		return nil, errors.New("Google client ID missing")
	}
	if googleClientSecret() == "" {
		return nil, errors.New("Google client secret missing")
	}
	return a.accountAddOAuth("gmail", payload)
}

func (a *App) accountAddOutlookOAuth(payload map[string]any) (any, error) {
	if outlookClientID() == "" {
		return nil, errors.New("Outlook client ID missing")
	}
	return a.accountAddOAuth("outlook", payload)
}

// accountAddOAuth saves an OAuth account from the tokens the redirect
// exchanged. A new account, or one without an avatar, takes the provider's
// profile photo; an avatar already set, possibly chosen by the user, stays.
func (a *App) accountAddOAuth(provider string, payload map[string]any) (any, error) {
	box := oauthMailboxes[provider]
	var req AddOAuthRequest
	if err := decode(payload, &req); err != nil {
		return nil, err
	}
	if req.AccessToken == "" || req.RefreshToken == "" {
		return nil, fmt.Errorf("%s sign-in returned no tokens; sign in again", box.label)
	}
	if !strings.Contains(req.Email, "@") {
		return nil, errors.New("invalid email")
	}

	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	id, existing, err := a.accountSaveID(req.ID, req.Email)
	if err != nil {
		return nil, err
	}
	switch {
	case existing != nil && existing.AvatarURL != "":
		// The connect below keeps the stored avatar when it's sent none.
		req.AvatarURL = existing.AvatarURL
	case req.AvatarURL != "":
		req.AvatarURL = a.downloadAndSaveAvatar(id, req.AvatarURL)
	default:
		if photo, err := a.fetchOAuthPhoto(provider, req.AccessToken, req.RefreshToken); err != nil {
			a.logf("account.add%sOAuth: profile photo unavailable: %v", box.label, err)
		} else {
			req.AvatarURL = a.saveAvatar(id, photo)
		}
	}
	expiresAt := time.Now().Unix() + req.ExpiresIn
	a.logf("account.add%sOAuth: connecting account=%s email=%s token_present=%t refresh_present=%t", box.label, id, req.Email, req.AccessToken != "", req.RefreshToken != "")
	if _, err := a.sidecar.Call("account.connect", map[string]any{
		"id":               id,
		"host":             box.imapHost,
		"port":             993,
		"user":             req.Email,
		"password":         "",
		"tls":              true,
		"smtp_host":        box.smtpHost,
		"smtp_port":        box.smtpPort,
		"smtp_tls":         box.smtpTLS,
		"smtp_starttls":    !box.smtpTLS,
		"auth_type":        box.authType,
		"access_token":     req.AccessToken,
		"refresh_token":    req.RefreshToken,
		"token_expires_at": expiresAt,
		"validate":         false,
		"email":            req.Email,
		"display_name":     req.DisplayName,
		"sender_name":      req.SenderName,
		"avatar_url":       req.AvatarURL,
		"provider":         provider,
	}); err != nil {
		return nil, err
	}
	_, _ = a.sidecar.Call("watch.start", map[string]any{"account": id})

	account := Account{
		ID:                id,
		Email:             req.Email,
		DisplayName:       req.DisplayName,
		SenderName:        req.SenderName,
		AvatarURL:         req.AvatarURL,
		Provider:          provider,
		AuthType:          box.authType,
		IncludedInUnified: true,
		IMAPHost:          box.imapHost,
		IMAPPort:          993,
		SMTPHost:          box.smtpHost,
		SMTPPort:          box.smtpPort,
		TLS:               true,
		SMTPTLS:           box.smtpTLS,
		SMTPStartTLS:      !box.smtpTLS,
		AccessToken:       req.AccessToken,
		RefreshToken:      req.RefreshToken,
		TokenExpiresAt:    expiresAt,
	}
	a.logf("account.add%sOAuth: saved account=%s", box.label, id)
	return map[string]any{"account": account}, nil
}

func (a *App) gmailBegin() (any, error) {
	if googleClientID() == "" {
		return nil, errors.New("Google client ID missing")
	}
	if googleClientSecret() == "" {
		return nil, errors.New("Google client secret missing")
	}
	return a.beginOAuth("gmail")
}

func (a *App) outlookBegin() (any, error) {
	if outlookClientID() == "" {
		return nil, errors.New("Outlook client ID missing")
	}
	return a.beginOAuth("outlook")
}

// beginOAuth starts a mailbox sign-in whose result the account dialog polls.
func (a *App) beginOAuth(provider string) (any, error) {
	state := randomBase64URL(32)
	a.oauthMu.Lock()
	a.oauthState = state
	a.oauthProvider = provider
	a.oauthProfile = nil
	a.oauthMu.Unlock()
	return a.startOAuth(provider, state, "", nil, func(profile *ExchangedProfile, err error) {
		if err != nil {
			a.logf("oauth callback failed (provider=%s): %v", provider, err)
			return
		}
		a.oauthMu.Lock()
		defer a.oauthMu.Unlock()
		if a.oauthState == state {
			a.oauthProfile = profile
		}
	})
}

func (a *App) oauthPollProfile(provider string) any {
	a.oauthMu.Lock()
	defer a.oauthMu.Unlock()
	if a.oauthProvider != provider || a.oauthProfile == nil {
		return map[string]any{"exchanged": false}
	}
	profile := a.oauthProfile
	a.oauthProfile = nil
	return map[string]any{"exchanged": true, "profile": profile}
}

// startOAuth opens the browser and binds PKCE state to this attempt's
// listener. Calendar consent and mailbox sign-in can run independently.
// scopes are the Google services sharing the account's grant (see
// connectGoogleService); nil for a mailbox sign-in.
func (a *App) startOAuth(provider, state, loginHint string, scopes []any, complete func(*ExchangedProfile, error)) (any, error) {
	verifier := randomBase64URL(32)
	challengeBytes := sha256.Sum256([]byte(verifier))
	challenge := base64.RawURLEncoding.EncodeToString(challengeBytes[:])
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}

	listener, err := net.Listen("tcp", oauthLoopbackHost+":0")
	if err != nil {
		return nil, fmt.Errorf("start OAuth callback listener: %w", err)
	}
	redirectURI := "http://" + listener.Addr().String()
	res, err := a.sidecar.Call("oauth.authorizeUrl", map[string]any{
		"provider":       provider,
		"redirect_uri":   redirectURI,
		"state":          state,
		"code_challenge": challenge,
		"login_hint":     loginHint,
		"scopes":         scopes,
	})
	if err != nil {
		listener.Close()
		return nil, err
	}
	authURL, _ := res.(map[string]any)["url"].(string)
	go a.serveOAuthRedirect(listener, provider, state, verifier, redirectURI, complete)
	if a.ctx != nil {
		wailsRuntime.BrowserOpenURL(a.ctx, authURL)
	}
	return map[string]any{"url": authURL, "needs_external_browser": true}, nil
}

func (a *App) serveOAuthRedirect(listener net.Listener, provider, state, verifier, redirectURI string, complete func(*ExchangedProfile, error)) {
	defer listener.Close()

	server := &http.Server{ReadHeaderTimeout: 10 * time.Second}
	var claimed atomic.Bool
	timer := time.AfterFunc(3*time.Minute, func() {
		if claimed.CompareAndSwap(false, true) {
			_ = server.Close()
			complete(nil, errors.New("Sign-in timed out. Try again."))
		}
	})
	defer timer.Stop()

	server.Handler = http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet || r.URL.Path != "/" || r.URL.Query().Get("state") != state {
			http.Error(w, "Authorization state did not match this sign-in.", http.StatusBadRequest)
			return
		}
		if !claimed.CompareAndSwap(false, true) {
			http.Error(w, "This sign-in has already ended.", http.StatusBadRequest)
			return
		}
		timer.Stop()
		defer func() { go server.Shutdown(context.Background()) }()

		if r.URL.Query().Get("error") != "" {
			http.Error(w, "The provider did not authorize the request.", http.StatusBadRequest)
			complete(nil, errors.New("Sign-in was declined"))
			return
		}
		code := r.URL.Query().Get("code")
		if code == "" {
			http.Error(w, "Missing authorization code.", http.StatusBadRequest)
			complete(nil, errors.New("Sign-in returned no authorization code"))
			return
		}
		profile, err := a.exchangeOAuthCode(provider, code, verifier, redirectURI)
		if err != nil {
			http.Error(w, "Sign-in failed. Return to Meron and try again.", http.StatusBadGateway)
			complete(nil, err)
			return
		}
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		_, _ = io.WriteString(w, "<html><body style=\"font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;text-align:center;padding-top:100px;background-color:#f8fafc;color:#1e293b;\"><h1>Authenticated</h1><p>You can close this tab and return to Meron.</p></body></html>")
		complete(profile, nil)
	})
	err := server.Serve(listener)
	if claimed.CompareAndSwap(false, true) {
		complete(nil, fmt.Errorf("OAuth callback listener: %w", err))
	}
}

// exchangeOAuthCode swaps an authorization code for tokens and the account
// profile, in the core shared with mobile (`oauth.exchangeCode`).
func (a *App) exchangeOAuthCode(provider, code, verifier, redirectURI string) (*ExchangedProfile, error) {
	if verifier == "" {
		return nil, errors.New("OAuth code verifier missing")
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	res, err := a.sidecar.Call("oauth.exchangeCode", map[string]any{
		"provider":      provider,
		"code":          code,
		"code_verifier": verifier,
		"redirect_uri":  redirectURI,
	})
	if err != nil {
		return nil, err
	}
	obj, _ := res.(map[string]any)
	var profile ExchangedProfile
	if err := decode(obj, &profile); err != nil {
		return nil, err
	}
	return &profile, nil
}

// fetchOAuthPhoto downloads the account's profile photo through the core:
// Google's userinfo picture, or Microsoft's from Graph.
func (a *App) fetchOAuthPhoto(provider, accessToken, refreshToken string) ([]byte, error) {
	res, err := a.sidecar.Call("oauth.fetchPhoto", map[string]any{
		"provider":      provider,
		"access_token":  accessToken,
		"refresh_token": refreshToken,
	})
	if err != nil {
		return nil, err
	}
	obj, _ := res.(map[string]any)
	data, _ := obj["data"].(string)
	return base64.StdEncoding.DecodeString(data)
}
