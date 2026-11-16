//go:build !windows

package main

// Only Windows keeps an icon per window; elsewhere the app or its desktop
// entry carries it.
func refreshWindowIcons() {}
