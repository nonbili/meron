package main

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
)

// applyUpdate installs a downloaded .dmg over the running .app bundle and
// queues a relaunch. The caller quits immediately afterwards.
//
// The DMG is mounted read-only, the bundle inside is checked against the
// running app's code signature, copied to the same directory as the installed
// bundle (so the swap is a rename on one volume), and then swapped in.
func applyUpdate(channel updateChannel, payload string) error {
	if channel.Kind != channelDMG {
		return fmt.Errorf("update: unsupported channel %q", channel.Kind)
	}
	bundle := channel.Target
	parent := filepath.Dir(bundle)
	if err := ensureWritableDir(parent); err != nil {
		return fmt.Errorf("update: %s is not writable — install the update manually: %w", parent, err)
	}

	mount, err := os.MkdirTemp("", "meron-update-mount-")
	if err != nil {
		return err
	}
	defer os.Remove(mount)

	if out, err := exec.Command("hdiutil", "attach", payload,
		"-nobrowse", "-readonly", "-noautoopen", "-mountpoint", mount).CombinedOutput(); err != nil {
		return fmt.Errorf("update: mount failed: %v: %s", err, strings.TrimSpace(string(out)))
	}
	defer func() { _ = exec.Command("hdiutil", "detach", mount, "-force").Run() }()

	source, err := bundleInside(mount)
	if err != nil {
		return err
	}
	if err := verifyBundleSignature(source, bundle); err != nil {
		return err
	}

	staging := filepath.Join(parent, fmt.Sprintf(".meron-update-%d.app", os.Getpid()))
	_ = os.RemoveAll(staging)
	// ditto preserves the bundle's symlinks, resource forks and signature;
	// a plain copy would break codesigning.
	if out, err := exec.Command("ditto", source, staging).CombinedOutput(); err != nil {
		_ = os.RemoveAll(staging)
		return fmt.Errorf("update: copy failed: %v: %s", err, strings.TrimSpace(string(out)))
	}

	previous := filepath.Join(parent, fmt.Sprintf(".meron-old-%d.app", os.Getpid()))
	_ = os.RemoveAll(previous)
	if err := os.Rename(bundle, previous); err != nil {
		_ = os.RemoveAll(staging)
		return fmt.Errorf("update: could not move the old app aside: %w", err)
	}
	if err := os.Rename(staging, bundle); err != nil {
		// Put the old bundle back so the user is not left without an app.
		_ = os.Rename(previous, bundle)
		_ = os.RemoveAll(staging)
		return fmt.Errorf("update: could not move the new app into place: %w", err)
	}
	_ = os.RemoveAll(previous)

	// -n forces a new instance rather than reactivating the dying one.
	return scheduleRelaunch("/usr/bin/open", "-n", bundle)
}

// bundleInside finds the single .app at the root of the mounted DMG.
func bundleInside(mount string) (string, error) {
	entries, err := os.ReadDir(mount)
	if err != nil {
		return "", err
	}
	for _, entry := range entries {
		if strings.HasSuffix(entry.Name(), ".app") {
			return filepath.Join(mount, entry.Name()), nil
		}
	}
	return "", fmt.Errorf("update: no .app found in the downloaded disk image")
}

var teamIdentifierPattern = regexp.MustCompile(`(?m)^TeamIdentifier=(.+)$`)

// verifyBundleSignature is the compensating control for not signing the update
// manifest ourselves: the downloaded bundle must carry a valid signature from
// the same Apple team as the copy that is running. An unsigned running app
// (a local build) has nothing to compare against, so only the validity check
// applies there.
func verifyBundleSignature(candidate, current string) error {
	if out, err := exec.Command("codesign", "--verify", "--deep", "--strict", candidate).CombinedOutput(); err != nil {
		return fmt.Errorf("update: the downloaded app is not correctly signed: %s", strings.TrimSpace(string(out)))
	}
	expected := teamIdentifier(current)
	if expected == "" || expected == "not set" {
		return nil
	}
	if got := teamIdentifier(candidate); got != expected {
		return fmt.Errorf("update: the downloaded app is signed by %q, expected %q", got, expected)
	}
	return nil
}

func teamIdentifier(bundle string) string {
	// codesign writes its display output to stderr.
	out, err := exec.Command("codesign", "-dv", "--verbose=4", bundle).CombinedOutput()
	if err != nil {
		return ""
	}
	match := teamIdentifierPattern.FindSubmatch(out)
	if match == nil {
		return ""
	}
	return strings.TrimSpace(string(match[1]))
}
