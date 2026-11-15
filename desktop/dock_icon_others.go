//go:build !darwin && !linux && !windows

package main

func appIconStyleSupported() bool { return false }

func platformSetAppIcon(string, []byte) {}
