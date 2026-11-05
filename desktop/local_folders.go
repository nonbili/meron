package main

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	wailsRuntime "meron/internal/desktopruntime"
)

// localAccountID is the one account whose folders live on this computer
// rather than on a server (the core's archive). It is routed like a mail
// account; the core serves its mail from the archive.
const localAccountID = "local"

func isLocalAccountID(id string) bool {
	return strings.TrimSpace(id) == localAccountID
}

// accountAddLocal creates the local account, if it does not exist yet.
func (a *App) accountAddLocal(payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	name, _ := payload["name"].(string)
	folder, _ := payload["folder"].(string)
	if _, err := a.sidecar.Call("account.addLocal", map[string]any{"name": name, "folder": folder}); err != nil {
		return nil, err
	}
	a.logf("account.addLocal: local folders ready")
	accounts, err := a.accountList()
	if err != nil {
		return nil, err
	}
	for _, account := range accounts.(map[string]any)["accounts"].([]Account) {
		if account.ID == localAccountID {
			return map[string]any{"account": account}, nil
		}
	}
	return map[string]any{"ok": true}, nil
}

// accountLocalStats reports how much mail the local folders keep and where.
func (a *App) accountLocalStats() (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	return a.sidecar.Call("account.localStats", map[string]any{})
}

// accountLocalBackup saves a consistent copy of the local folders' database
// where the user picks, encrypted with the passphrase they chose rather than
// this computer's keychain key, so it restores anywhere.
func (a *App) accountLocalBackup(payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	passphrase, _ := payload["passphrase"].(string)
	if passphrase == "" {
		return nil, errors.New("a passphrase is required")
	}
	dest, err := saveFileDialog(a.ctx, wailsRuntime.SaveDialogOptions{
		Title:                "Back up local folders",
		DefaultFilename:      fmt.Sprintf("meron-local-folders-%s.db", time.Now().Format("2006-01-02")),
		CanCreateDirectories: true,
		Filters: []wailsRuntime.FileFilter{
			{DisplayName: "Meron local folders (*.db)", Pattern: "*.db"},
		},
	})
	if err != nil {
		return nil, err
	}
	if dest == "" {
		return map[string]any{"saved": false}, nil // user cancelled
	}
	if isLiveDatabasePath(dest) {
		return nil, fmt.Errorf("choose another file: %s is in use by Meron", dest)
	}
	// The core writes the snapshot into a private staging directory and the
	// bridge copies it out: under Flatpak the save portal grants access to the
	// chosen file only, not to a temporary sibling the core would write first.
	tmp, err := os.MkdirTemp("", "meron-local-backup-")
	if err != nil {
		return nil, err
	}
	defer os.RemoveAll(tmp)
	staged := filepath.Join(tmp, "archive.db")
	if _, err := a.sidecar.Call("account.localBackup", map[string]any{"path": staged, "passphrase": passphrase}); err != nil {
		return nil, err
	}
	if err := copyFileTo(staged, dest, 0o600); err != nil {
		return nil, fmt.Errorf("save local folders backup: %w", err)
	}
	a.logf("account.localBackup: wrote %s", dest)
	return map[string]any{"saved": true, "path": dest}, nil
}

// wrongLocalBackupPassphrase is the core's marker (store::WRONG_BACKUP_PASSPHRASE)
// for a passphrase that does not open the chosen backup.
const wrongLocalBackupPassphrase = "wrong passphrase for this local folders backup"

// accountLocalRestore adds the mail of a local folders backup. The first call
// picks the file; a wrong passphrase comes back with that `path`, so the retry
// does not make the user pick the same file again.
func (a *App) accountLocalRestore(payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	passphrase, _ := payload["passphrase"].(string)
	if passphrase == "" {
		return nil, errors.New("a passphrase is required")
	}
	src, _ := payload["path"].(string)
	if src == "" {
		chosen, err := openFileDialog(a.ctx, wailsRuntime.OpenDialogOptions{
			Title: "Restore local folders",
			Filters: []wailsRuntime.FileFilter{
				{DisplayName: "Meron local folders (*.db)", Pattern: "*.db"},
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
	if isLiveDatabasePath(src) {
		return nil, fmt.Errorf("choose a backup file: %s is in use by Meron", src)
	}
	// A private copy: a portal-granted file may be read-only to SQLite, which
	// wants to write beside a database it opens.
	tmp, err := os.MkdirTemp("", "meron-local-restore-")
	if err != nil {
		return nil, err
	}
	defer os.RemoveAll(tmp)
	staged := filepath.Join(tmp, "archive.db")
	if err := copyFileTo(src, staged, 0o600); err != nil {
		return nil, fmt.Errorf("read local folders backup: %w", err)
	}
	res, err := a.sidecar.Call("account.localRestore", map[string]any{"path": staged, "passphrase": passphrase})
	if err != nil {
		if strings.Contains(err.Error(), wrongLocalBackupPassphrase) {
			return map[string]any{"restored": 0, "wrongPassphrase": true, "path": src}, nil
		}
		return nil, err
	}
	resMap, _ := res.(map[string]any)
	a.logf("account.localRestore: restored %v messages from %s", resMap["restored"], src)
	return map[string]any{"restored": resMap["restored"], "path": src}, nil
}

// isLiveDatabasePath reports whether path names one of the databases Meron has
// open (or their WAL/shared-memory files): writing a backup over one would
// corrupt it.
func isLiveDatabasePath(path string) bool {
	resolve := func(p string) string {
		dir, name := filepath.Split(filepath.Clean(p))
		if resolved, err := filepath.EvalSymlinks(dir); err == nil {
			dir = resolved
		}
		return filepath.Join(dir, name)
	}
	target := resolve(path)
	// An existing destination may be a symlink or hard link to a live file,
	// which a name comparison cannot see; the copy would follow it.
	targetInfo, statErr := os.Stat(path)
	for _, db := range []string{"meron.db", "archive.db"} {
		for _, suffix := range []string{"", "-wal", "-shm"} {
			live := filepath.Join(appConfigDir(), db+suffix)
			if target == resolve(live) {
				return true
			}
			if statErr != nil {
				continue
			}
			if liveInfo, err := os.Stat(live); err == nil && os.SameFile(targetInfo, liveInfo) {
				return true
			}
		}
	}
	return false
}
