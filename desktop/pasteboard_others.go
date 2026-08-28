//go:build !darwin

package main

import "errors"

// The darwin clipboard and file-opening helpers are reached through a
// runtime.GOOS check rather than build tags, so the other platforms need
// stubs to compile. None of these are ever called off macOS.

func readClipboardImageDarwin() ([]byte, string, error) {
	return nil, "", nil
}

func copyImageToClipboardDarwin(path, mime string) error {
	return errors.New("not supported on this platform")
}

func openSystemFileDarwin(path string) error {
	return errors.New("not supported on this platform")
}
