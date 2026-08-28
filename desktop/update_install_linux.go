package main

import (
	"archive/tar"
	"compress/gzip"
	"fmt"
	"io"
	"os"
	"path/filepath"
)

// applyUpdate replaces the running AppImage (or the raw binary from the
// tarball) with the downloaded payload and queues a relaunch.
//
// The replacement is a rename within the target's own directory, so it is
// atomic and the running process keeps the old inode until it exits.
func applyUpdate(channel updateChannel, payload string) error {
	if err := replaceTarget(channel, payload); err != nil {
		return err
	}
	return scheduleRelaunch(channel.Target)
}

// replaceTarget is applyUpdate without the relaunch: everything that touches
// the filesystem, so it can be exercised on its own.
func replaceTarget(channel updateChannel, payload string) error {
	target := channel.Target
	if target == "" {
		return fmt.Errorf("update: unsupported channel %q", channel.Kind)
	}
	parent := filepath.Dir(target)
	if err := ensureWritableDir(parent); err != nil {
		return fmt.Errorf("update: %s is not writable — install the update manually: %w", parent, err)
	}

	staging := filepath.Join(parent, fmt.Sprintf(".meron-update-%d", os.Getpid()))
	_ = os.Remove(staging)

	var err error
	switch channel.Kind {
	case channelAppImage:
		// The payload is the new AppImage itself.
		err = copyFile(payload, staging, 0o755)
	case channelTarball:
		err = extractBinaryFromTarGz(payload, "meron", staging)
	default:
		return fmt.Errorf("update: unsupported channel %q", channel.Kind)
	}
	if err != nil {
		_ = os.Remove(staging)
		return err
	}

	if err := os.Chmod(staging, 0o755); err != nil {
		_ = os.Remove(staging)
		return err
	}
	if err := os.Rename(staging, target); err != nil {
		_ = os.Remove(staging)
		return fmt.Errorf("update: could not replace %s: %w", target, err)
	}
	return nil
}

// extractBinaryFromTarGz pulls a single entry out of the release tarball. The
// archive only ever holds the `meron` binary, but the name is matched on the
// basename so a leading directory in a future layout still works.
func extractBinaryFromTarGz(archive, name, dest string) error {
	file, err := os.Open(archive)
	if err != nil {
		return err
	}
	defer file.Close()
	gz, err := gzip.NewReader(file)
	if err != nil {
		return err
	}
	defer gz.Close()

	reader := tar.NewReader(gz)
	for {
		header, err := reader.Next()
		if err == io.EOF {
			return fmt.Errorf("update: %s not found in the downloaded archive", name)
		}
		if err != nil {
			return err
		}
		if header.Typeflag != tar.TypeReg || filepath.Base(header.Name) != name {
			continue
		}
		out, err := os.OpenFile(dest, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, 0o755)
		if err != nil {
			return err
		}
		if _, err := io.Copy(out, reader); err != nil {
			out.Close()
			return err
		}
		return out.Close()
	}
}
