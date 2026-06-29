//go:build !linux || (linux && (!webkit2_41 || bindings))

package main

func setupNativeSpellChecking() {}
