//go:build !linux

package main

import "context"

func openFilePicker(
	ctx context.Context,
	title string,
	defaultDir string,
	imagesOnly bool,
	multiple bool,
) ([]string, bool, error) {
	paths, err := openWailsFilePicker(ctx, title, defaultDir, imagesOnly, multiple)
	return paths, false, err
}
