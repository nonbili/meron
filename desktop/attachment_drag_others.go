//go:build (!linux && !darwin && !windows) || (linux && bindings)

package main

func startNativeAttachmentDrag(path string) (bool, error) {
	return false, nil
}
