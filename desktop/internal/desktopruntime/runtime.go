// Package desktopruntime adapts Meron's single-window operations to Wails v3.
// Context parameters are retained for the application lifecycle and test hooks;
// Wails itself no longer uses them to select a window.
package desktopruntime

import (
	"context"

	"github.com/wailsapp/wails/v3/pkg/application"
	"github.com/wailsapp/wails/v3/pkg/events"
)

var App *application.App
var Window *application.WebviewWindow

type FileFilter = application.FileFilter
type WindowStartState = application.WindowState

const Normal = application.WindowStateNormal
const Maximised = application.WindowStateMaximised

type RGBA struct{ R, G, B, A uint8 }

type OpenDialogOptions struct {
	Window           *application.WebviewWindow
	Title            string
	DefaultDirectory string
	Filters          []FileFilter
}

type SaveDialogOptions struct {
	Window               *application.WebviewWindow
	CanCreateDirectories bool
	Title                string
	DefaultDirectory     string
	DefaultFilename      string
	Filters              []FileFilter
}

func EventsEmit(_ context.Context, name string, data ...any) {
	if App == nil {
		return
	}
	var detail any
	if len(data) == 1 {
		detail = data[0]
	} else if len(data) > 1 {
		detail = data
	}
	App.Event.Emit(name, detail)
}

func WindowShow(context.Context)             { Window.Show() }
func WindowHide(context.Context)             { Window.Hide() }
func WindowMinimise(context.Context)         { Window.Minimise() }
func WindowUnminimise(context.Context)       { Window.UnMinimise() }
func WindowMaximise(context.Context)         { Window.Maximise() }
func WindowToggleMaximise(context.Context)   { Window.ToggleMaximise() }
func WindowIsMinimised(context.Context) bool { return Window.IsMinimised() }
func WindowIsMaximised(context.Context) bool { return Window.IsMaximised() }
func WindowIsNormal(context.Context) bool {
	return !Window.IsMinimised() && !Window.IsMaximised() && !Window.IsFullscreen()
}
func WindowGetSize(context.Context) (int, int) { return Window.Size() }

func Quit(context.Context)                         { App.Quit() }
func BrowserOpenURL(_ context.Context, url string) { _ = App.Browser.OpenURL(url) }

func OnFileDrop(_ context.Context, callback func(int, int, []string)) {
	Window.OnWindowEvent(events.Common.WindowFilesDropped, func(event *application.WindowEvent) {
		callback(0, 0, event.Context().DroppedFiles())
		if target := event.Context().DropTargetDetails(); target != nil {
			App.Event.Emit("window.filesDropped", target)
		}
	})
}

func OpenFileDialog(_ context.Context, options OpenDialogOptions) (string, error) {
	window := options.Window
	if window == nil {
		window = Window
	}
	return App.Dialog.OpenFileWithOptions(&application.OpenFileDialogOptions{
		Title: options.Title, Directory: options.DefaultDirectory, Filters: options.Filters, Window: window,
	}).PromptForSingleSelection()
}

func OpenMultipleFilesDialog(_ context.Context, options OpenDialogOptions) ([]string, error) {
	window := options.Window
	if window == nil {
		window = Window
	}
	return App.Dialog.OpenFileWithOptions(&application.OpenFileDialogOptions{
		Title: options.Title, Directory: options.DefaultDirectory, Filters: options.Filters, Window: window,
		AllowsMultipleSelection: true,
	}).PromptForMultipleSelection()
}

func SaveFileDialog(_ context.Context, options SaveDialogOptions) (string, error) {
	window := options.Window
	if window == nil {
		window = Window
	}
	return App.Dialog.SaveFileWithOptions(&application.SaveFileDialogOptions{
		Title: options.Title, Directory: options.DefaultDirectory, Filename: options.DefaultFilename,
		Filters: options.Filters, Window: window, CanCreateDirectories: options.CanCreateDirectories,
	}).PromptForSingleSelection()
}
