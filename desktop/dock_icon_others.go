//go:build !darwin && !linux && !windows

package main

import "meron/internal/appicon"

func appIconStyleSupported() bool { return false }

func platformSetAppIcon(appicon.Variant, []byte) {}
