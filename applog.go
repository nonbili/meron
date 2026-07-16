package main

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

// maxLogViewLines caps how much of meron.log the in-app viewer and the export
// include; the newest lines matter for troubleshooting.
const maxLogViewLines = 1000

var logEmailRegexp = regexp.MustCompile(`[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}`)

// redactLogEmails masks the local part of every email address so the log keeps
// the domain for context but never the full address, e.g. "j***@gmail.com".
// Mirrors the mobile diagnostic log's redaction.
func redactLogEmails(text string) string {
	return logEmailRegexp.ReplaceAllStringFunc(text, func(email string) string {
		at := strings.Index(email, "@")
		if at <= 0 {
			return "***"
		}
		return email[:1] + "***" + email[at:]
	})
}

// appLogTail returns the newest maxLogViewLines of meron.log with email
// addresses redacted, ready to show in the Settings log viewer or export.
func appLogTail() (string, error) {
	data, err := os.ReadFile(filepath.Join(appConfigDir(), "meron.log"))
	if err != nil {
		if os.IsNotExist(err) {
			return "", nil
		}
		return "", err
	}
	lines := strings.Split(strings.TrimRight(string(data), "\n"), "\n")
	if len(lines) > maxLogViewLines {
		lines = lines[len(lines)-maxLogViewLines:]
	}
	return redactLogEmails(strings.Join(lines, "\n")), nil
}

func (a *App) logRead() (any, error) {
	tail, err := appLogTail()
	if err != nil {
		return nil, err
	}
	return map[string]any{"log": tail}, nil
}

// logExport writes the redacted log tail to a user-chosen path via a native
// save dialog, with a disclosure header matching the mobile share flow.
func (a *App) logExport() (any, error) {
	tail, err := appLogTail()
	if err != nil {
		return nil, err
	}
	dest, err := wailsRuntime.SaveFileDialog(a.ctx, wailsRuntime.SaveDialogOptions{
		Title:                "Export log",
		DefaultFilename:      "meron-log.txt",
		CanCreateDirectories: true,
		Filters: []wailsRuntime.FileFilter{
			{DisplayName: "Text files (*.txt)", Pattern: "*.txt"},
		},
	})
	if err != nil {
		return nil, err
	}
	if dest == "" {
		return map[string]any{"saved": false}, nil // user cancelled
	}
	disclosure := "Account emails below are masked to only the first letter and domain (e.g. j***@gmail.com).\n" +
		"Review before sharing.\n\n"
	if err := os.WriteFile(dest, []byte(disclosure+tail), 0o644); err != nil {
		return nil, fmt.Errorf("write log: %w", err)
	}
	a.logf("log.export: wrote %s", dest)
	return map[string]any{"saved": true, "path": dest}, nil
}
