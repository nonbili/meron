//go:build !darwin

package main

// Only macOS has a Dock tile badge; Linux and Windows show unread mail
// through the tray icon.
func platformSetDockBadgeLabel(string) {}
