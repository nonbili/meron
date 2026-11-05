//go:build !linux || (linux && (!gtk3 || bindings))

package main

func setupNativeSpellChecking() {}
