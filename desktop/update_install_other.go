//go:build !darwin && !linux && !windows

package main

import "fmt"

func applyUpdate(channel updateChannel, payload string) error {
	return fmt.Errorf("update: in-app updates are not supported on this platform")
}
