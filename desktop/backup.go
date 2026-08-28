package main

import (
	"errors"
	"fmt"
	"os"
	"time"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

// backupFilename is the name offered in the save dialog. Dated so successive
// backups don't silently overwrite each other, and `.json` because the file is
// plain JSON whose envelope stays readable even when the payload is encrypted.
func backupFilename(now time.Time) string {
	return fmt.Sprintf("meron-backup-%s.json", now.Format("2006-01-02"))
}

// exportBackup asks the sidecar to serialize accounts, prefs, feeds and
// settings, then writes the document to a user-chosen path via a native save
// dialog. Cached mail is not included — see meron-core's `backup` module.
//
// `include_secrets` requires a passphrase; the core refuses the combination
// otherwise, so no plaintext file can ever carry a password.
func (a *App) exportBackup(payload map[string]any) (any, error) {
	// Validated before the engine check so the guard holds even with no sidecar:
	// a plaintext file carrying passwords must be unreachable by every path.
	includeSecrets, _ := payload["include_secrets"].(bool)
	passphrase, _ := payload["passphrase"].(string)
	if includeSecrets && passphrase == "" {
		return nil, errors.New("a passphrase is required to include account passwords")
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}

	res, err := a.sidecar.Call("backup.export", map[string]any{
		"include_secrets": includeSecrets,
		"passphrase":      passphrase,
		// The sidecar can't know the product version, so the envelope's
		// app_version is only meaningful if the host supplies it. The platform
		// goes with it because desktop and mobile are versioned on separate
		// tracks: "0.2.4" is a different build depending on which wrote it.
		"platform":    "desktop",
		"app_version": appVersion(),
	})
	if err != nil {
		return nil, err
	}
	resMap, _ := res.(map[string]any)
	document, _ := resMap["backup"].(string)
	if document == "" {
		return nil, errors.New("nothing to back up")
	}

	dest, err := wailsRuntime.SaveFileDialog(a.ctx, wailsRuntime.SaveDialogOptions{
		Title:                "Export backup",
		DefaultFilename:      backupFilename(time.Now()),
		CanCreateDirectories: true,
		Filters: []wailsRuntime.FileFilter{
			{DisplayName: "Meron backup (*.json)", Pattern: "*.json"},
		},
	})
	if err != nil {
		return nil, err
	}
	if dest == "" {
		return map[string]any{"saved": false}, nil // user cancelled
	}
	if err := writePrivateFile(dest, []byte(document)); err != nil {
		return nil, fmt.Errorf("write backup: %w", err)
	}
	a.logf("backup.export: wrote %s (secrets=%t)", dest, includeSecrets)
	return map[string]any{"saved": true, "path": dest, "encrypted": passphrase != ""}, nil
}

// importBackup reads a backup file and hands it to the sidecar, which restores
// accounts, feeds and settings.
//
// An encrypted file needs a passphrase the user has not typed yet, so the first
// call comes back `needsPassphrase` along with the chosen `path`. The frontend
// prompts and calls again with that path, which skips the file dialog rather
// than making the user pick the same file twice.
func (a *App) importBackup(payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	passphrase, _ := payload["passphrase"].(string)

	src, _ := payload["path"].(string)
	if src == "" {
		chosen, err := wailsRuntime.OpenFileDialog(a.ctx, wailsRuntime.OpenDialogOptions{
			Title: "Restore backup",
			Filters: []wailsRuntime.FileFilter{
				{DisplayName: "Meron backup (*.json)", Pattern: "*.json"},
			},
		})
		if err != nil {
			return nil, err
		}
		if chosen == "" {
			return map[string]any{"cancelled": true}, nil // user cancelled
		}
		src = chosen
	}

	data, err := os.ReadFile(src)
	if err != nil {
		return nil, fmt.Errorf("read backup: %w", err)
	}

	res, err := a.sidecar.Call("backup.import", map[string]any{
		"backup":     string(data),
		"passphrase": passphrase,
	})
	if err != nil {
		return nil, err
	}
	resMap, _ := res.(map[string]any)
	if needs, _ := resMap["needs_passphrase"].(bool); needs {
		a.logf("backup.import: %s is encrypted, asking for a passphrase", src)
		return map[string]any{"needsPassphrase": true, "path": src}, nil
	}

	summary := map[string]any{
		"accounts": jsonInt(resMap["accounts"]),
		"skipped":  jsonInt(resMap["skipped"]),
		"feeds":    jsonInt(resMap["feeds"]),
		"settings": jsonInt(resMap["settings"]),
		"secrets":  jsonInt(resMap["secrets"]),
	}
	a.logf("backup.import: restored %d accounts (%d skipped), %d feeds, %d settings from %s",
		summary["accounts"], summary["skipped"], summary["feeds"], summary["settings"], src)
	return summary, nil
}

// jsonInt reads a JSON number (which decodes as float64) as an int.
func jsonInt(value any) int {
	n, _ := value.(float64)
	return int(n)
}

// writePrivateFile writes data to path with owner-only permissions.
//
// 0600 matters here: even a backup without secrets lists every address the user
// reads mail for, and an encrypted one is only as private as its passphrase.
// os.WriteFile's mode argument applies *only when it creates the file*, so
// overwriting an existing world-readable file would silently keep mode 0644 —
// the save dialog happily offers existing paths. Removing it first guarantees
// we are the one creating it. A umask can only tighten 0600 further, never
// loosen it.
func writePrivateFile(path string, data []byte) error {
	// A missing file (the common case) is exactly what we want; any other
	// failure surfaces from the write below with a better message.
	if err := os.Remove(path); err != nil && !os.IsNotExist(err) {
		return err
	}
	return os.WriteFile(path, data, 0o600)
}
