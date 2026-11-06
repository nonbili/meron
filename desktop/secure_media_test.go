package main

import (
	"context"
	"os"
	"path/filepath"
	"strings"
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

// Without XDG_RUNTIME_DIR (no logind session on Linux; macOS; Windows)
// decrypted attachments go to a fixed directory in the per-user cache: private
// (no other local user can write there, unlike the shared temp dir) and the same
// in every run, so the next start can clear what a crash left.
func TestSecureMediaDirFallbackIsStableAndPrivate(t *testing.T) {
	t.Setenv("XDG_RUNTIME_DIR", "")
	t.Setenv("XDG_CACHE_HOME", t.TempDir())
	tmp := t.TempDir()
	t.Setenv("TMPDIR", tmp)
	dir := secureMediaDir()
	if dir != filepath.Join(appCacheDir(), "secure-media") {
		t.Fatalf("secure media dir %s is not in the per-user cache", dir)
	}
	if strings.HasPrefix(dir, tmp+string(filepath.Separator)) {
		t.Fatalf("secure media dir %s is in the shared temp dir", dir)
	}
	if again := secureMediaDir(); again != dir {
		t.Fatalf("secure media dir moved between calls: %s, then %s", dir, again)
	}
	if err := prepareSecureMedia(); err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(dir)
	if err != nil {
		t.Fatalf("secure media dir not created: %v", err)
	}
	if perm := info.Mode().Perm(); perm != 0o700 {
		t.Fatalf("secure media dir is %o, want 0700", perm)
	}
}

// Something other than a directory where the secure dir belongs is refused.
func TestPrepareSecureMediaRefusesASymlink(t *testing.T) {
	t.Setenv("XDG_RUNTIME_DIR", "")
	t.Setenv("XDG_CACHE_HOME", t.TempDir())
	target := t.TempDir()
	if err := os.MkdirAll(filepath.Dir(secureMediaDir()), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(target, secureMediaDir()); err != nil {
		t.Skip("no symlinks here")
	}
	// The wipe removes the link itself (never what it points to), so the dir is
	// recreated as a real, private directory.
	if err := prepareSecureMedia(); err != nil {
		t.Fatal(err)
	}
	info, err := os.Lstat(secureMediaDir())
	if err != nil || info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		t.Fatalf("secure media dir is not a real directory: %v %v", info, err)
	}
	if _, err := os.Stat(target); err != nil {
		t.Fatalf("the link's target was touched: %v", err)
	}
}

// A run that crashes or is killed never wipes at quit. The next start must
// still clear what it left: the secure dir has to be the same path in every run.
func TestStartupClearsWhatACrashedRunLeft(t *testing.T) {
	t.Setenv("XDG_RUNTIME_DIR", "")
	t.Setenv("XDG_CACHE_HOME", t.TempDir())
	t.Setenv("TMPDIR", t.TempDir())
	// Run 1 writes a decrypted attachment and is killed.
	left := filepath.Join(secureMediaDir(), "acc", "INBOX", "7", "0.pdf")
	if err := os.MkdirAll(filepath.Dir(left), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(left, []byte("plaintext"), 0o600); err != nil {
		t.Fatal(err)
	}
	// Run 2 starts.
	if err := prepareSecureMedia(); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(left); !os.IsNotExist(err) {
		t.Fatalf("a crashed run's decrypted file survived the next start: %v", err)
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
