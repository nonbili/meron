//go:build !darwin

package main

func (a *App) startTray() {
	a.startTrayPhysical()
}
