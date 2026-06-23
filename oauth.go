package main

import (
	"context"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"strings"
	"time"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

func (a *App) accountAddGmailOAuth(payload map[string]any) (any, error) {
	var req AddGmailOAuthRequest
	if err := decode(payload, &req); err != nil {
		return nil, err
	}
	if googleClientID() == "" {
		return nil, errors.New("Google client ID missing")
	}
	if googleClientSecret() == "" {
		return nil, errors.New("Google client secret missing")
	}
	if req.AccessToken == "" || req.RefreshToken == "" {
		a.logf("account.addGmailOAuth: exchanging auth code")
		tokens, err := a.exchangeGmailOAuthCode(req.AuthCode)
		if err != nil {
			return nil, err
		}
		req.AccessToken = tokens.AccessToken
		req.RefreshToken = tokens.RefreshToken
		req.ExpiresIn = tokens.ExpiresIn
	}
	if req.Email == "" || req.AvatarURL == "" {
		profile, err := fetchGoogleUserInfo(req.AccessToken)
		if err != nil {
			return nil, err
		}
		if req.Email == "" {
			req.Email = profile.Email
		}
		if req.DisplayName == "" {
			req.DisplayName = profile.Name
		}
		if req.AvatarURL == "" {
			req.AvatarURL = profile.Picture
		}
	}
	if !strings.Contains(req.Email, "@") {
		return nil, errors.New("invalid email")
	}

	id := accountID(req.Email)
	if req.AvatarURL != "" {
		req.AvatarURL = a.downloadAndSaveAvatar(id, req.AvatarURL)
	}
	expiresAt := time.Now().Unix() + req.ExpiresIn
	a.logf("account.addGmailOAuth: connecting account=%s email=%s token_present=%t refresh_present=%t", id, req.Email, req.AccessToken != "", req.RefreshToken != "")
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, err := a.sidecar.Call("account.connect", map[string]any{
		"id":               id,
		"host":             "imap.gmail.com",
		"port":             993,
		"user":             req.Email,
		"password":         "",
		"tls":              true,
		"smtp_host":        "smtp.gmail.com",
		"smtp_port":        465,
		"smtp_tls":         true,
		"auth_type":        "gmail_oauth",
		"access_token":     req.AccessToken,
		"refresh_token":    req.RefreshToken,
		"token_expires_at": expiresAt,
		"validate":         false,
		"email":            req.Email,
		"display_name":     req.DisplayName,
		"sender_name":      req.SenderName,
		"avatar_url":       req.AvatarURL,
		"provider":         "gmail",
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
		Provider:          "gmail",
		AuthType:          "gmail_oauth",
		IncludedInUnified: true,
		IMAPHost:          "imap.gmail.com",
		IMAPPort:          993,
		SMTPHost:          "smtp.gmail.com",
		SMTPPort:          465,
		TLS:               true,
		AccessToken:       req.AccessToken,
		RefreshToken:      req.RefreshToken,
		TokenExpiresAt:    expiresAt,
	}
	a.logf("account.addGmailOAuth: saved account=%s", id)
	return map[string]any{"account": account}, nil
}

func (a *App) accountAddOutlookOAuth(payload map[string]any) (any, error) {
	var req AddOutlookOAuthRequest
	if err := decode(payload, &req); err != nil {
		return nil, err
	}
	if outlookClientID() == "" {
		return nil, errors.New("Outlook client ID missing")
	}
	if req.AccessToken == "" || req.RefreshToken == "" {
		a.logf("account.addOutlookOAuth: exchanging auth code")
		a.oauthMu.Lock()
		verifier := a.oauthVerifier
		redirectURI := a.oauthRedirectURI
		a.oauthVerifier = ""
		a.oauthMu.Unlock()
		tokens, err := exchangeOutlookOAuthCodeWithVerifier(req.AuthCode, verifier, redirectURI)
		if err != nil {
			return nil, err
		}
		req.AccessToken = tokens.AccessToken
		req.RefreshToken = tokens.RefreshToken
		req.ExpiresIn = tokens.ExpiresIn
		if req.Email == "" {
			email, name := parseIDTokenClaims(tokens.IDToken)
			req.Email = email
			if req.DisplayName == "" {
				req.DisplayName = name
			}
		}
	}
	if !strings.Contains(req.Email, "@") {
		return nil, errors.New("invalid email")
	}

	id := accountID(req.Email)
	expiresAt := time.Now().Unix() + req.ExpiresIn
	a.logf("account.addOutlookOAuth: connecting account=%s email=%s token_present=%t refresh_present=%t", id, req.Email, req.AccessToken != "", req.RefreshToken != "")
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, err := a.sidecar.Call("account.connect", map[string]any{
		"id":               id,
		"host":             "outlook.office365.com",
		"port":             993,
		"user":             req.Email,
		"password":         "",
		"tls":              true,
		"smtp_host":        "smtp-mail.outlook.com",
		"smtp_port":        587,
		"smtp_tls":         true,
		"smtp_starttls":    true,
		"auth_type":        "outlook_oauth",
		"access_token":     req.AccessToken,
		"refresh_token":    req.RefreshToken,
		"token_expires_at": expiresAt,
		"validate":         false,
		"email":            req.Email,
		"display_name":     req.DisplayName,
		"sender_name":      req.SenderName,
		"provider":         "outlook",
	}); err != nil {
		return nil, err
	}
	_, _ = a.sidecar.Call("watch.start", map[string]any{"account": id})

	account := Account{
		ID:                id,
		Email:             req.Email,
		DisplayName:       req.DisplayName,
		SenderName:        req.SenderName,
		Provider:          "outlook",
		AuthType:          "outlook_oauth",
		IncludedInUnified: true,
		IMAPHost:          "outlook.office365.com",
		IMAPPort:          993,
		SMTPHost:          "smtp-mail.outlook.com",
		SMTPPort:          587,
		TLS:               true,
		AccessToken:       req.AccessToken,
		RefreshToken:      req.RefreshToken,
		TokenExpiresAt:    expiresAt,
	}
	a.logf("account.addOutlookOAuth: saved account=%s", id)
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

// beginOAuth runs the shared PKCE authorization-code setup for any provider:
// it generates state + verifier, arms the loopback callback listener, builds
// the provider's authorization URL, and opens the system browser. The active
// provider is recorded so the (shared) callback handler knows which token
// endpoint and profile source to use.
func (a *App) beginOAuth(provider string) (any, error) {
	state := randomBase64URL(32)
	verifier := randomBase64URL(32)
	challengeBytes := sha256.Sum256([]byte(verifier))
	challenge := base64.RawURLEncoding.EncodeToString(challengeBytes[:])

	// Bind an ephemeral loopback port up front so the redirect URI carries the
	// real port (RFC 8252). The same URI must be used at the token exchange, so
	// it's stored on the App for the callback handler / fallback path to read.
	listener, err := net.Listen("tcp", oauthLoopbackHost+":0")
	if err != nil {
		return nil, fmt.Errorf("start OAuth callback listener: %w", err)
	}
	redirectURI := "http://" + listener.Addr().String()

	a.oauthMu.Lock()
	a.oauthState = state
	a.oauthVerifier = verifier
	a.oauthProvider = provider
	a.oauthRedirectURI = redirectURI
	a.oauthProfile = nil
	a.oauthMu.Unlock()

	a.logf("oauth.begin: provider=%s callback listening on %s", provider, redirectURI)
	go a.serveOAuthRedirect(listener)

	values := url.Values{}
	values.Set("response_type", "code")
	values.Set("redirect_uri", redirectURI)
	values.Set("state", state)
	values.Set("code_challenge", challenge)
	values.Set("code_challenge_method", "S256")

	var authBase string
	switch provider {
	case "outlook":
		values.Set("client_id", outlookClientID())
		values.Set("scope", outlookScopes)
		authBase = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize"
	default: // gmail
		values.Set("client_id", googleClientID())
		values.Set("scope", "https://mail.google.com/ https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile")
		values.Set("access_type", "offline")
		values.Set("prompt", "consent")
		authBase = "https://accounts.google.com/o/oauth2/v2/auth"
	}
	authURL := authBase + "?" + values.Encode()
	if a.ctx != nil {
		wailsRuntime.BrowserOpenURL(a.ctx, authURL)
	}
	a.logf("oauth.begin: provider=%s opened external browser", provider)
	return map[string]any{"url": authURL, "needs_external_browser": true}, nil
}

func (a *App) oauthPollProfile() any {
	a.oauthMu.Lock()
	defer a.oauthMu.Unlock()
	if a.oauthProfile == nil {
		return map[string]any{"exchanged": false}
	}
	profile := a.oauthProfile
	a.oauthProfile = nil
	a.logf("oauth.pollProfile: returning exchanged profile for %s", profile.Email)
	return map[string]any{"exchanged": true, "profile": profile}
}

func (a *App) serveOAuthRedirect(listener net.Listener) {
	defer listener.Close()

	server := &http.Server{ReadHeaderTimeout: 10 * time.Second}
	server.Handler = http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		a.oauthMu.Lock()
		expectedState := a.oauthState
		verifier := a.oauthVerifier
		provider := a.oauthProvider
		redirectURI := a.oauthRedirectURI
		a.oauthState = ""
		a.oauthVerifier = ""
		a.oauthMu.Unlock()

		if r.URL.Query().Get("error") != "" {
			a.logf("oauth callback returned error: %s", r.URL.Query().Get("error"))
			http.Error(w, "The provider did not authorize the request.", http.StatusBadRequest)
			go server.Shutdown(context.Background())
			return
		}
		if expectedState == "" || r.URL.Query().Get("state") != expectedState {
			a.logf("oauth callback rejected: state mismatch")
			http.Error(w, "Authorization state did not match this sign-in.", http.StatusBadRequest)
			go server.Shutdown(context.Background())
			return
		}
		code := r.URL.Query().Get("code")
		if code == "" || verifier == "" {
			a.logf("oauth callback rejected: code_present=%t verifier_present=%t", code != "", verifier != "")
			http.Error(w, "Missing authorization code.", http.StatusBadRequest)
			go server.Shutdown(context.Background())
			return
		}
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		_, _ = io.WriteString(w, "<html><body style=\"font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;text-align:center;padding-top:100px;background-color:#f8fafc;color:#1e293b;\"><h1>Authenticated</h1><p>You can close this tab and return to Meron.</p></body></html>")
		go server.Shutdown(context.Background())

		profile, err := exchangeOAuthCode(provider, code, verifier, redirectURI)
		if err != nil {
			a.logf("oauth exchange failed (provider=%s): %v", provider, err)
			fmt.Fprintf(os.Stderr, "meron: OAuth exchange failed: %v\n", err)
			return
		}
		profile.AuthCode = code
		a.oauthMu.Lock()
		a.oauthProfile = profile
		a.oauthMu.Unlock()
		a.logf("oauth callback exchanged profile for %s (provider=%s)", profile.Email, provider)
	})
	_ = server.Serve(listener)
}

// exchangeOAuthCode swaps an authorization code for tokens and resolves the
// account profile for the given provider. Gmail calls Google's userinfo
// endpoint; Outlook reads the OIDC id_token returned alongside the tokens (no
// extra request — Microsoft Graph is a separate resource the IMAP/SMTP token
// can't address).
func exchangeOAuthCode(provider, code, verifier, redirectURI string) (*ExchangedProfile, error) {
	switch provider {
	case "outlook":
		tokens, err := exchangeOutlookOAuthCodeWithVerifier(code, verifier, redirectURI)
		if err != nil {
			return nil, err
		}
		email, name := parseIDTokenClaims(tokens.IDToken)
		return &ExchangedProfile{
			Email:        email,
			DisplayName:  name,
			AccessToken:  tokens.AccessToken,
			RefreshToken: tokens.RefreshToken,
			ExpiresIn:    tokens.ExpiresIn,
		}, nil
	default: // gmail
		tokens, err := exchangeGmailOAuthCodeWithVerifier(code, verifier, redirectURI)
		if err != nil {
			return nil, err
		}
		user, err := fetchGoogleUserInfo(tokens.AccessToken)
		if err != nil {
			return nil, fmt.Errorf("Google userinfo failed: %w", err)
		}
		return &ExchangedProfile{
			Email:        user.Email,
			DisplayName:  user.Name,
			AvatarURL:    user.Picture,
			AccessToken:  tokens.AccessToken,
			RefreshToken: tokens.RefreshToken,
			ExpiresIn:    tokens.ExpiresIn,
		}, nil
	}
}

// exchangeOutlookOAuthCodeWithVerifier exchanges an authorization code at the
// Microsoft token endpoint. Public client (PKCE), so no client secret is sent.
func exchangeOutlookOAuthCodeWithVerifier(code, verifier, redirectURI string) (*TokenExchangeResult, error) {
	if verifier == "" {
		return nil, errors.New("Outlook OAuth code verifier missing")
	}
	values := url.Values{}
	values.Set("code", code)
	values.Set("client_id", outlookClientID())
	values.Set("code_verifier", verifier)
	values.Set("redirect_uri", redirectURI)
	values.Set("grant_type", "authorization_code")
	values.Set("scope", outlookScopes)
	res, err := http.PostForm(outlookTokenURL, values)
	if err != nil {
		return nil, err
	}
	defer res.Body.Close()
	body, _ := io.ReadAll(res.Body)
	if res.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("Outlook OAuth exchange failed: %s", strings.TrimSpace(string(body)))
	}
	var out TokenExchangeResult
	if err := json.Unmarshal(body, &out); err != nil {
		return nil, err
	}
	if out.RefreshToken == "" {
		return nil, errors.New("Outlook OAuth response did not include a refresh token")
	}
	return &out, nil
}

// parseIDTokenClaims decodes the (unverified) payload of an OIDC id_token to
// extract the account email and display name. The token was just received over
// TLS straight from Microsoft's token endpoint, so we read its claims without
// re-verifying the signature. Returns empty strings on any parse failure; the
// caller validates that an email is present.
func parseIDTokenClaims(idToken string) (email, name string) {
	parts := strings.Split(idToken, ".")
	if len(parts) != 3 {
		return "", ""
	}
	payload, err := base64.RawURLEncoding.DecodeString(parts[1])
	if err != nil {
		return "", ""
	}
	var claims struct {
		Email             string `json:"email"`
		PreferredUsername string `json:"preferred_username"`
		Name              string `json:"name"`
	}
	if err := json.Unmarshal(payload, &claims); err != nil {
		return "", ""
	}
	email = claims.Email
	if email == "" {
		email = claims.PreferredUsername
	}
	return email, claims.Name
}

func (a *App) exchangeGmailOAuthCode(code string) (*TokenExchangeResult, error) {
	a.oauthMu.Lock()
	verifier := a.oauthVerifier
	redirectURI := a.oauthRedirectURI
	a.oauthVerifier = ""
	a.oauthMu.Unlock()
	return exchangeGmailOAuthCodeWithVerifier(code, verifier, redirectURI)
}

func exchangeGmailOAuthCodeWithVerifier(code, verifier, redirectURI string) (*TokenExchangeResult, error) {
	if verifier == "" {
		return nil, errors.New("Google OAuth code verifier missing")
	}
	values := url.Values{}
	values.Set("code", code)
	values.Set("client_id", googleClientID())
	values.Set("client_secret", googleClientSecret())
	values.Set("code_verifier", verifier)
	values.Set("redirect_uri", redirectURI)
	values.Set("grant_type", "authorization_code")
	res, err := http.PostForm("https://oauth2.googleapis.com/token", values)
	if err != nil {
		return nil, err
	}
	defer res.Body.Close()
	body, _ := io.ReadAll(res.Body)
	if res.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("Google OAuth exchange failed: %s", strings.TrimSpace(string(body)))
	}
	var out TokenExchangeResult
	if err := json.Unmarshal(body, &out); err != nil {
		return nil, err
	}
	if out.RefreshToken == "" {
		return nil, errors.New("Google OAuth response did not include a refresh token")
	}
	return &out, nil
}

func fetchGoogleUserInfo(accessToken string) (*GoogleUserInfo, error) {
	req, _ := http.NewRequest(http.MethodGet, "https://www.googleapis.com/oauth2/v3/userinfo", nil)
	req.Header.Set("Authorization", "Bearer "+accessToken)
	res, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer res.Body.Close()
	body, _ := io.ReadAll(res.Body)
	if res.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("Google userinfo failed: %s", strings.TrimSpace(string(body)))
	}
	var out GoogleUserInfo
	return &out, json.Unmarshal(body, &out)
}
