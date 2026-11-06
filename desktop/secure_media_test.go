package main

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
)

func TestDecryptedAttachmentKeysResolveInsideTheSecureDir(t *testing.T) {
	t.Setenv("XDG_RUNTIME_DIR", t.TempDir())
	path, err := mediaFilePath("e2ee/acc/INBOX/7/0.txt")
	if err != nil {
		t.Fatal(err)
	}
	if !strings.HasPrefix(path, secureMediaDir()+string(filepath.Separator)) {
		t.Fatalf("decrypted attachment resolved outside the secure dir: %s", path)
	}
	if _, err := mediaFilePath("e2ee/../../etc/passwd"); err == nil {
		if p, _ := mediaFilePath("e2ee/../../etc/passwd"); !strings.HasPrefix(p, secureMediaDir()) {
			t.Fatalf("traversal escaped the secure dir: %s", p)
		}
	}
	plain, err := mediaFilePath("acc/INBOX/7/0.txt")
	if err != nil || !strings.HasPrefix(plain, mediaDir()) {
		t.Fatalf("plain attachment moved: %s %v", plain, err)
	}
}

func TestSidecarGetsTheSecureMediaDir(t *testing.T) {
	t.Setenv("XDG_RUNTIME_DIR", t.TempDir())
	want := "MERON_SECURE_MEDIA_DIR=" + secureMediaDir()
	for _, kv := range sidecarEnv() {
		if kv == want {
			return
		}
	}
	t.Fatalf("sidecar env lacks %s", want)
}

// Without XDG_RUNTIME_DIR (no logind session on Linux; macOS;
// Windows) decrypted attachments go to a directory that is both ephemeral
// (under the temp dir, not the persistent cache) and private: an
// unguessable, owner-only parent, not the old predictable path under the
// world-writable /tmp that another local user could read or pre-plant.
func TestSecureMediaDirFallbackIsEphemeralAndPrivate(t *testing.T) {
	secureMediaOnce = &sync.Once{}
	secureMediaPath = ""
	t.Cleanup(func() { secureMediaOnce = &sync.Once{}; secureMediaPath = "" })
	tmp := t.TempDir()
	t.Setenv("XDG_RUNTIME_DIR", "")
	t.Setenv("TMPDIR", tmp)
	dir := secureMediaDir()
	if !strings.HasPrefix(dir, tmp+string(filepath.Separator)) {
		t.Fatalf("secure media dir %s is not under the temp dir %s (it must be ephemeral)", dir, tmp)
	}
	if strings.HasPrefix(dir, appCacheDir()) {
		t.Fatalf("secure media dir %s is in the persistent cache", dir)
	}
	// Not the old guessable shared path another local user could pre-plant.
	if dir == filepath.Join(os.TempDir(), appDirName(), "secure-media") {
		t.Fatalf("secure media dir is a predictable shared path: %s", dir)
	}
	// Its parent is owner-only (0700), so no other user can read the plaintext
	// or plant a symlink there.
	info, err := os.Stat(filepath.Dir(dir))
	if err != nil {
		t.Fatalf("secure media parent not created: %v", err)
	}
	if perm := info.Mode().Perm(); perm&0o077 != 0 {
		t.Fatalf("secure media parent %o is accessible to other users", perm)
	}
}

// Decrypted files live only while Meron runs. The wipe empties the
// secure dir and touches nothing beside it.
func TestWipeSecureMediaRemovesDecryptedFilesOnly(t *testing.T) {
	runtimeDir := t.TempDir()
	t.Setenv("XDG_RUNTIME_DIR", runtimeDir)
	secret := filepath.Join(secureMediaDir(), "acc", "INBOX", "7", "0.pdf")
	if err := os.MkdirAll(filepath.Dir(secret), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(secret, []byte("plaintext"), 0o600); err != nil {
		t.Fatal(err)
	}
	beside := filepath.Join(filepath.Dir(secureMediaDir()), "other")
	if err := os.WriteFile(beside, []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}
	wipeSecureMedia()
	if _, err := os.Stat(secureMediaDir()); !os.IsNotExist(err) {
		t.Fatalf("secure media dir survived the wipe: %v", err)
	}
	if _, err := os.Stat(beside); err != nil {
		t.Fatalf("the wipe removed a file beside the secure dir: %v", err)
	}
	wipeSecureMedia() // nothing there: no panic, no error
}

// The copy handed to another app for a decrypted attachment stays in
// the secure dir (wiped), not in the persistent cache like a plain one.
func TestDecryptedAttachmentsAreOpenedFromTheSecureDir(t *testing.T) {
	t.Setenv("XDG_RUNTIME_DIR", t.TempDir())
	if root := attachmentOpenRoot("e2ee/acc/INBOX/7/0.pdf"); !strings.HasPrefix(root, secureMediaDir()+string(filepath.Separator)) {
		t.Fatalf("decrypted attachment opened from %s", root)
	}
	if root := attachmentOpenRoot("acc/INBOX/7/0.pdf"); root != filepath.Join(appCacheDir(), attachmentOpenDirName) {
		t.Fatalf("plain attachment opened from %s", root)
	}
}

// Quitting wipes the secure dir.
func TestShutdownWipesSecureMedia(t *testing.T) {
	t.Setenv("XDG_RUNTIME_DIR", t.TempDir())
	if err := os.MkdirAll(secureMediaDir(), 0o700); err != nil {
		t.Fatal(err)
	}
	(&App{}).Shutdown(context.Background())
	if _, err := os.Stat(secureMediaDir()); !os.IsNotExist(err) {
		t.Fatalf("secure media dir survived shutdown: %v", err)
	}
}
