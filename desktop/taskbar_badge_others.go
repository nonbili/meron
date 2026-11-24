//go:build !windows

package main

// Taskbar overlay badges are Windows-only; macOS uses the Dock badge and Linux
// shows unread mail through the tray icon.
func platformSetTaskbarBadgeLabel(string) {}
