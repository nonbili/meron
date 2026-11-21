package main

import (
	"crypto/sha256"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"unicode/utf8"

	wailsRuntime "meron/internal/desktopruntime"
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
	return a.connectGoogleService(payload, "google_calendar", "caldav.addGoogle")
}

// connectGoogleService keeps Google service credentials out of the webview.
func (a *App) connectGoogleService(payload map[string]any, provider, method string) (any, error) {
	if !gmailOAuthConfigured() {
		return nil, errors.New("Google sign-in is not configured")
	}
	accountID, _ := payload["account_id"].(string)
	loginHint, _ := payload["login_hint"].(string)
	// A Google grant this account already holds may cover the service:
	// then no browser sign-in is needed. Otherwise the core names every
	// scope the sign-in must cover, the other connected services' included.
	var scopes []any
	if email := strings.TrimSpace(loginHint); email != "" {
		res, err := a.calendarInvoke(method, map[string]any{"account_id": accountID, "email": email})
		if err != nil {
			return nil, err
		}
		answer, _ := res.(map[string]any)
		if answer["needs_sign_in"] != true {
			return res, nil
		}
		scopes, _ = answer["scopes"].([]any)
	}
	type result struct {
		profile *ExchangedProfile
		err     error
	}
	done := make(chan result, 1)
	_, err := a.startOAuth(provider, randomBase64URL(32), loginHint, scopes, func(profile *ExchangedProfile, err error) {
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
		"scope":         profile.Scope,
	})
}

const maxCalendarFileBytes = 8 * 1024 * 1024

func calendarFileFilters() []wailsRuntime.FileFilter {
	return []wailsRuntime.FileFilter{{DisplayName: "iCalendar (*.ics)", Pattern: "*.ics"}}
}

func (a *App) importCalendarFile(command string, payload map[string]any) (any, error) {
	preview := command == "calendar.previewImport"
	path, _ := payload["path"].(string)
	if preview {
		var err error
		path, err = openFileDialog(a.ctx, wailsRuntime.OpenDialogOptions{
			Title: "Import calendar file", Filters: calendarFileFilters(),
		})
		if err != nil {
			return nil, err
		}
		if path == "" {
			return map[string]any{"cancelled": true}, nil
		}
	}

	info, err := os.Stat(path)
	if err != nil {
		return nil, fmt.Errorf("read calendar file metadata: %w", err)
	}
	if !info.Mode().IsRegular() {
		return nil, errors.New("choose a regular calendar file")
	}
	file, err := os.Open(path)
	if err != nil {
		return nil, fmt.Errorf("open calendar file: %w", err)
	}
	defer file.Close()
	data, err := io.ReadAll(io.LimitReader(file, maxCalendarFileBytes+1))
	if err != nil {
		return nil, fmt.Errorf("read calendar file: %w", err)
	}
	if len(data) > maxCalendarFileBytes {
		return nil, errors.New("calendar file exceeds 8 MiB")
	}
	if !utf8.Valid(data) {
		return nil, errors.New("calendar file must use UTF-8")
	}
	digest := fmt.Sprintf("%x", sha256.Sum256(data))
	expected, _ := payload["digest"].(string)
	if !preview && expected != digest {
		return nil, errors.New("calendar file changed since preview. Choose it again")
	}
	res, err := a.calendarInvoke(command, map[string]any{"ics": string(data)})
	if err != nil {
		return nil, err
	}
	if preview {
		result, ok := res.(map[string]any)
		if !ok {
			return nil, errors.New("invalid calendar import preview")
		}
		result["path"] = path
		result["filename"] = filepath.Base(path)
		result["digest"] = digest
	}
	return res, nil
}

func (a *App) exportCalendarFile(payload map[string]any) (any, error) {
	res, err := a.calendarInvoke("calendar.exportIcs", payload)
	if err != nil {
		return nil, err
	}
	result, _ := res.(map[string]any)
	document, _ := result["ics"].(string)
	if document == "" {
		return nil, errors.New("nothing to export")
	}

	filename := "meron-calendar.ics"
	if eventID, _ := payload["event_id"].(string); eventID != "" {
		filename = "meron-event.ics"
	}
	dest, err := saveFileDialog(a.ctx, wailsRuntime.SaveDialogOptions{
		Title: "Export calendar file", DefaultFilename: filename, CanCreateDirectories: true,
		Filters: calendarFileFilters(),
	})
	if err != nil {
		return nil, err
	}
	if dest == "" {
		return map[string]any{"saved": false}, nil
	}
	data := []byte(document)
	err = writeFileAtomic(dest, data, 0o600, 0o700)
	if errors.Is(err, os.ErrPermission) {
		// A sandboxed file chooser can grant write access only to the file.
		err = os.WriteFile(dest, data, 0o600)
	}
	if err != nil {
		return nil, fmt.Errorf("write calendar file: %w", err)
	}
	return map[string]any{"saved": true}, nil
}
