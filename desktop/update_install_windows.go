package main

import (
	"archive/zip"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
)

// oldExeSuffix marks the displaced copy of a running exe. Windows lets a
// running image be renamed but not overwritten, so the portable channel renames
// itself out of the way and the leftover is swept up on the next launch.
const oldExeSuffix = ".old"

// applyUpdate installs the downloaded payload and queues a relaunch. The caller
// quits immediately afterwards.
func applyUpdate(channel updateChannel, payload string) error {
	switch channel.Kind {
	case channelNSIS:
		return runSilentInstaller(payload, channel.Target)
	case channelPortable:
		return replacePortableExe(payload, channel.Target)
	default:
		return fmt.Errorf("update: unsupported channel %q", channel.Kind)
	}
}

// runSilentInstaller launches the NSIS installer in silent mode and starts the
// app again once it finishes. The installer may raise a UAC prompt for a
// per-machine install; the UI warns about that before this is called.
func runSilentInstaller(installer, exe string) error {
	// One detached cmd does both halves so the relaunch survives our exit:
	// /wait blocks until the installer is done, then start re-opens the app.
	script := fmt.Sprintf(`start "" /wait "%s" /S && start "" "%s"`, installer, exe)
	cmd := exec.Command("cmd", "/c", script)
	cmd.SysProcAttr = &syscall.SysProcAttr{HideWindow: true, CreationFlags: detachedProcess | createNewProcessGroup}
	return cmd.Start()
}

const (
	detachedProcess       = 0x00000008
	createNewProcessGroup = 0x00000200
)

// replacePortableExe swaps the loose meron.exe from the portable zip.
func replacePortableExe(archive, exe string) error {
	parent := filepath.Dir(exe)
	if err := ensureWritableDir(parent); err != nil {
		return fmt.Errorf("update: %s is not writable — install the update manually: %w", parent, err)
	}

	staging := filepath.Join(parent, fmt.Sprintf(".meron-update-%d.exe", os.Getpid()))
	_ = os.Remove(staging)
	if err := extractExeFromZip(archive, filepath.Base(exe), staging); err != nil {
		_ = os.Remove(staging)
		return err
	}

	previous := exe + oldExeSuffix
	_ = os.Remove(previous)
	if err := os.Rename(exe, previous); err != nil {
		_ = os.Remove(staging)
		return fmt.Errorf("update: could not move the old executable aside: %w", err)
	}
	if err := os.Rename(staging, exe); err != nil {
		_ = os.Rename(previous, exe)
		_ = os.Remove(staging)
		return fmt.Errorf("update: could not move the new executable into place: %w", err)
	}

	// A short delay covers process teardown; the old exe still holds the
	// single-instance lock for a moment after Quit returns.
	script := fmt.Sprintf(`timeout /t 3 /nobreak >nul & start "" "%s"`, exe)
	cmd := exec.Command("cmd", "/c", script)
	cmd.SysProcAttr = &syscall.SysProcAttr{HideWindow: true, CreationFlags: detachedProcess | createNewProcessGroup}
	return cmd.Start()
}

func extractExeFromZip(archive, name, dest string) error {
	reader, err := zip.OpenReader(archive)
	if err != nil {
		return err
	}
	defer reader.Close()
	for _, entry := range reader.File {
		if entry.FileInfo().IsDir() || !strings.EqualFold(filepath.Base(entry.Name), name) {
			continue
		}
		source, err := entry.Open()
		if err != nil {
			return err
		}
		defer source.Close()
		out, err := os.OpenFile(dest, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, 0o755)
		if err != nil {
			return err
		}
		if _, err := io.Copy(out, source); err != nil {
			out.Close()
			return err
		}
		return out.Close()
	}
	return fmt.Errorf("update: %s not found in the downloaded archive", name)
}

// sweepReplacedExecutable deletes the previous exe left behind by a portable
// update. Called on startup, once the new copy is the one running.
func sweepReplacedExecutable() {
	exe, err := os.Executable()
	if err != nil {
		return
	}
	_ = os.Remove(exe + oldExeSuffix)
}
