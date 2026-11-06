package main

import (
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"testing"
)

// Exercise the actual shell script without requesting root access or opening an
// authentication dialog. The fake pkexec forwards to /bin/sh as the test user.
func fakePolicyKit(t *testing.T, body string) {
	t.Helper()
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "pkexec"), []byte("#!/bin/sh\n"+body), 0o755); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", dir)
}

func TestPolicyKitUpdateAtomicReplacement(t *testing.T) {
	fakePolicyKit(t, "[ \"$1\" = --disable-internal-agent ] || exit 1\nshift\nexec \"$@\"\n")
	dir := t.TempDir()
	// Shell syntax in a path must stay literal, including when passed to root.
	target := filepath.Join(dir, "Meron ' $(exit 9); image")
	payload := filepath.Join(t.TempDir(), "payload")
	if err := os.WriteFile(target, []byte("old binary"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(payload, []byte("new binary\x00\xff"), 0o600); err != nil {
		t.Fatal(err)
	}
	old, err := os.Open(target)
	if err != nil {
		t.Fatal(err)
	}
	defer old.Close()
	before, _ := old.Stat()
	if err := installWithPolicyKit(payload, target); err != nil {
		t.Fatal(err)
	}
	got, _ := os.ReadFile(target)
	if string(got) != "new binary\x00\xff" {
		t.Fatalf("installed contents = %q", got)
	}
	oldContents := make([]byte, 10)
	if _, err := old.Read(oldContents); err != nil || string(oldContents) != "old binary" {
		t.Fatalf("old running inode was modified: %q, %v", oldContents, err)
	}
	after, _ := os.Stat(target)
	oldOwner := before.Sys().(*syscall.Stat_t)
	newOwner := after.Sys().(*syscall.Stat_t)
	if oldOwner.Uid != newOwner.Uid || oldOwner.Gid != newOwner.Gid || after.Mode().Perm() != 0o755 {
		t.Fatalf("ownership or executable permissions changed: %v", after)
	}
	assertNoLeftovers(t, dir, filepath.Base(target))
}

func TestPolicyKitUpdateAuthorizationFailures(t *testing.T) {
	for _, tc := range []struct {
		name, body, message string
	}{
		{"cancelled", "exit 126\n", "cancelled"},
		{"no agent", "exit 127\n", "authentication agent"},
		{"helper failed", "echo disk-full >&2\nexit 1\n", "disk-full"},
		{"missing pkexec", "", "install it or install the update manually"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			if tc.body == "" {
				t.Setenv("PATH", t.TempDir())
			} else {
				fakePolicyKit(t, tc.body)
			}
			target := filepath.Join(t.TempDir(), "meron")
			if err := os.WriteFile(target, []byte("old binary"), 0o755); err != nil {
				t.Fatal(err)
			}
			err := installWithPolicyKit("/dev/null", target)
			if err == nil || !strings.Contains(err.Error(), tc.message) {
				t.Fatalf("error = %v, want %q", err, tc.message)
			}
			got, _ := os.ReadFile(target)
			if string(got) != "old binary" {
				t.Fatalf("target modified: %q", got)
			}
		})
	}
}

func TestPolicyKitUpdateRejectsSymlink(t *testing.T) {
	fakePolicyKit(t, "shift\nexec \"$@\"\n")
	dir := t.TempDir()
	real := filepath.Join(dir, "real")
	target := filepath.Join(dir, "meron")
	if err := os.WriteFile(real, []byte("old binary"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(real, target); err != nil {
		t.Fatal(err)
	}
	if err := installWithPolicyKit("/dev/null", target); err == nil {
		t.Fatal("expected symlink rejection")
	}
	got, _ := os.ReadFile(real)
	if string(got) != "old binary" {
		t.Fatalf("symlink destination modified: %q", got)
	}
	assertNoLeftovers(t, dir, "real", "meron")
}

func TestPolicyKitHelperReservesAuthorizationExitCodes(t *testing.T) {
	for _, status := range []int{126, 127} {
		t.Run(strconv.Itoa(status), func(t *testing.T) {
			dir := t.TempDir()
			target := filepath.Join(dir, "meron")
			if err := os.WriteFile(target, []byte("old binary"), 0o755); err != nil {
				t.Fatal(err)
			}
			// Simulate a command missing or not executable after staging starts.
			script := strings.Replace(policyKitUpdateScript, `cat > "$staging"`, "exit "+strconv.Itoa(status), 1)
			cmd := exec.Command("/bin/sh", "-c", script, "meron-update", target)
			output, err := cmd.CombinedOutput()
			exit, ok := err.(*exec.ExitError)
			if !ok || exit.ExitCode() != 1 {
				t.Fatalf("helper error = %v, output = %q; want exit 1", err, output)
			}
			got, _ := os.ReadFile(target)
			if string(got) != "old binary" {
				t.Fatalf("target modified: %q", got)
			}
			assertNoLeftovers(t, dir, "meron")
		})
	}
}
