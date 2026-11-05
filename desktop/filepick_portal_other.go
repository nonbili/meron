//go:build !linux

package main

import (
	"context"

	wailsRuntime "meron/internal/desktopruntime"
)

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

func saveFileDialog(ctx context.Context, options wailsRuntime.SaveDialogOptions) (string, error) {
	return wailsRuntime.SaveFileDialog(ctx, options)
}

func openFileDialog(ctx context.Context, options wailsRuntime.OpenDialogOptions) (string, error) {
	return wailsRuntime.OpenFileDialog(ctx, options)
}
