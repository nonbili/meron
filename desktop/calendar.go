package main

import (
	"errors"
	"strings"
)

// calendarInvoke forwards one calendar command to the core unchanged, like
// tasksInvoke: the core's method names are the frontend's.
func (a *App) calendarInvoke(command string, payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	return a.sidecar.Call(command, payload)
}

// connectGoogleCalendar keeps the calendar grant out of the webview.
func (a *App) connectGoogleCalendar(payload map[string]any) (any, error) {
	return a.connectGoogleDAV(payload, "google_calendar", "caldav.addGoogle")
}

// connectGoogleDAV keeps DAV credentials out of the webview.
func (a *App) connectGoogleDAV(payload map[string]any, provider, method string) (any, error) {
	if !gmailOAuthConfigured() {
		return nil, errors.New("Google sign-in is not configured")
	}
	accountID, _ := payload["account_id"].(string)
	loginHint, _ := payload["login_hint"].(string)
	type result struct {
		profile *ExchangedProfile
		err     error
	}
	done := make(chan result, 1)
	_, err := a.startOAuth(provider, randomBase64URL(32), loginHint, func(profile *ExchangedProfile, err error) {
		done <- result{profile: profile, err: err}
	})
	if err != nil {
		return nil, err
	}
	answer := <-done
	if answer.err != nil {
		return nil, answer.err
	}
	profile := answer.profile
	if email := strings.TrimSpace(loginHint); email != "" && !strings.EqualFold(email, strings.TrimSpace(profile.Email)) {
		return nil, errors.New("Sign in with the Google account shown in Meron")
	}
	return a.calendarInvoke(method, map[string]any{
		"account_id":    accountID,
		"email":         profile.Email,
		"access_token":  profile.AccessToken,
		"refresh_token": profile.RefreshToken,
		"expires_in":    profile.ExpiresIn,
	})
}
