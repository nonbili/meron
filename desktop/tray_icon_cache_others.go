//go:build !linux

package main

func platformTrayIconNeedsRepair() bool { return false }
