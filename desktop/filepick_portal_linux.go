//go:build linux

package main

import (
	"context"
	"errors"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"strings"

	"github.com/godbus/dbus/v5"
	"github.com/google/uuid"
	wailsRuntime "meron/internal/desktopruntime"
)

const (
	portalDesktopName        = "org.freedesktop.portal.Desktop"
	portalDesktopPath        = dbus.ObjectPath("/org/freedesktop/portal/desktop")
	portalRequestNamespace   = dbus.ObjectPath("/org/freedesktop/portal/desktop/request")
	portalFileChooserOpen    = "org.freedesktop.portal.FileChooser.OpenFile"
	portalFileChooserSave    = "org.freedesktop.portal.FileChooser.SaveFile"
	portalRequestResponse    = "org.freedesktop.portal.Request.Response"
	portalRequestInterface   = "org.freedesktop.portal.Request"
	portalRequestCloseMethod = "org.freedesktop.portal.Request.Close"
)

type portalFilterRule struct {
	Kind    uint32
	Pattern string
}

type portalFilter struct {
	Name  string
	Rules []portalFilterRule
}

func openFilePicker(
	ctx context.Context,
	title string,
	defaultDir string,
	imagesOnly bool,
	multiple bool,
) ([]string, bool, error) {
	if !useFileChooserPortal() {
		paths, err := openWailsFilePicker(ctx, title, defaultDir, imagesOnly, multiple)
		return paths, false, err
	}

	paths, err := portalFileChooser(ctx, portalFileChooserOpen, title, portalOpenFileOptions(imagesOnly, multiple))
	return paths, true, err
}

// useFileChooserPortal reports whether file dialogs must go through the
// xdg-desktop-portal. Wails builds its GTK dialog in-process, which inside the
// Flatpak sandbox can only browse the sandbox itself, while the portal shows
// the host filesystem and grants access to just the chosen file.
func useFileChooserPortal() bool {
	return os.Getenv("FLATPAK_ID") != ""
}

// saveFileDialog is wailsRuntime.SaveFileDialog, routed through the portal
// when sandboxed. It returns "" when the user cancels.
func saveFileDialog(ctx context.Context, options wailsRuntime.SaveDialogOptions) (string, error) {
	if !useFileChooserPortal() {
		return wailsRuntime.SaveFileDialog(ctx, options)
	}
	portalOptions := portalBaseOptions()
	if options.DefaultFilename != "" {
		portalOptions["current_name"] = dbus.MakeVariant(options.DefaultFilename)
	}
	addPortalFilters(portalOptions, options.Filters)
	return firstPortalPath(portalFileChooser(ctx, portalFileChooserSave, options.Title, portalOptions))
}

// openFileDialog is wailsRuntime.OpenFileDialog, routed through the portal
// when sandboxed. It returns "" when the user cancels.
func openFileDialog(ctx context.Context, options wailsRuntime.OpenDialogOptions) (string, error) {
	if !useFileChooserPortal() {
		return wailsRuntime.OpenFileDialog(ctx, options)
	}
	portalOptions := portalBaseOptions()
	addPortalFilters(portalOptions, options.Filters)
	return firstPortalPath(portalFileChooser(ctx, portalFileChooserOpen, options.Title, portalOptions))
}

func firstPortalPath(paths []string, err error) (string, error) {
	if err != nil || len(paths) == 0 {
		return "", err
	}
	return paths[0], nil
}

// portalFileChooser runs one FileChooser portal request and returns the
// chosen local paths, or none when the user cancels.
func portalFileChooser(ctx context.Context, method, title string, options map[string]dbus.Variant) ([]string, error) {
	uris, err := portalFileChooserURIs(ctx, method, title, options)
	if err != nil {
		return nil, err
	}
	return portalFilePaths(uris)
}

func portalFileChooserURIs(ctx context.Context, method, title string, options map[string]dbus.Variant) ([]string, error) {
	conn, err := dbus.ConnectSessionBus()
	if err != nil {
		return nil, fmt.Errorf("connect to file chooser portal: %w", err)
	}
	defer conn.Close()

	signals := make(chan *dbus.Signal, 1)
	conn.Signal(signals)
	defer conn.RemoveSignal(signals)

	matchOptions := []dbus.MatchOption{
		dbus.WithMatchInterface(portalRequestInterface),
		dbus.WithMatchMember("Response"),
		dbus.WithMatchPathNamespace(portalRequestNamespace),
	}
	if err := conn.AddMatchSignal(matchOptions...); err != nil {
		return nil, fmt.Errorf("listen for file chooser response: %w", err)
	}
	defer conn.RemoveMatchSignal(matchOptions...)

	var handle dbus.ObjectPath
	call := conn.Object(portalDesktopName, portalDesktopPath).CallWithContext(
		ctx,
		method,
		0,
		"",
		title,
		options,
	)
	if call.Err != nil {
		return nil, fmt.Errorf("open file chooser portal: %w", call.Err)
	}
	if err := call.Store(&handle); err != nil {
		return nil, fmt.Errorf("read file chooser handle: %w", err)
	}

	for {
		select {
		case <-ctx.Done():
			conn.Object(portalDesktopName, handle).Go(portalRequestCloseMethod, 0, nil)
			return nil, ctx.Err()
		case signal, ok := <-signals:
			if !ok {
				return nil, errors.New("file chooser portal closed unexpectedly")
			}
			if signal.Path != handle || signal.Name != portalRequestResponse {
				continue
			}
			return portalResponseURIs(signal.Body)
		}
	}
}

func portalBaseOptions() map[string]dbus.Variant {
	return map[string]dbus.Variant{
		"handle_token": dbus.MakeVariant("meron_" + strings.ReplaceAll(uuid.NewString(), "-", "_")),
		"modal":        dbus.MakeVariant(true),
	}
}

// addPortalFilters converts Wails filters ("*.opml;*.xml") into portal glob
// filters, preselecting the first one as Wails does.
func addPortalFilters(options map[string]dbus.Variant, filters []wailsRuntime.FileFilter) {
	converted := portalFilters(filters)
	if len(converted) == 0 {
		return
	}
	options["filters"] = dbus.MakeVariant(converted)
	options["current_filter"] = dbus.MakeVariant(converted[0])
}

func portalFilters(filters []wailsRuntime.FileFilter) []portalFilter {
	var converted []portalFilter
	for _, filter := range filters {
		var rules []portalFilterRule
		for _, pattern := range strings.Split(filter.Pattern, ";") {
			if pattern = strings.TrimSpace(pattern); pattern != "" {
				rules = append(rules, portalFilterRule{Kind: 0, Pattern: pattern})
			}
		}
		if len(rules) > 0 {
			converted = append(converted, portalFilter{Name: filter.DisplayName, Rules: rules})
		}
	}
	return converted
}

func portalOpenFileOptions(imagesOnly, multiple bool) map[string]dbus.Variant {
	options := portalBaseOptions()
	options["multiple"] = dbus.MakeVariant(multiple)
	if imagesOnly {
		options["filters"] = dbus.MakeVariant([]portalFilter{
			{
				Name: "Image files",
				Rules: []portalFilterRule{
					{Kind: 1, Pattern: "image/png"},
					{Kind: 1, Pattern: "image/jpeg"},
					{Kind: 1, Pattern: "image/gif"},
					{Kind: 1, Pattern: "image/webp"},
				},
			},
		})
	}
	return options
}

func portalResponseURIs(body []any) ([]string, error) {
	if len(body) != 2 {
		return nil, errors.New("invalid file chooser response")
	}
	response, ok := body[0].(uint32)
	if !ok {
		return nil, errors.New("invalid file chooser response code")
	}
	switch response {
	case 0:
		results, ok := body[1].(map[string]dbus.Variant)
		if !ok {
			return nil, errors.New("invalid file chooser results")
		}
		uris, ok := results["uris"]
		if !ok {
			return nil, errors.New("file chooser returned no files")
		}
		values, ok := uris.Value().([]string)
		if !ok {
			return nil, errors.New("invalid file chooser file list")
		}
		return values, nil
	case 1:
		return nil, nil
	default:
		return nil, errors.New("file chooser request failed")
	}
}

func portalFilePaths(uris []string) ([]string, error) {
	paths := make([]string, 0, len(uris))
	for _, raw := range uris {
		uri, err := url.Parse(raw)
		if err != nil {
			return nil, fmt.Errorf("parse selected file URI: %w", err)
		}
		if uri.Scheme != "file" || (uri.Host != "" && uri.Host != "localhost") {
			return nil, fmt.Errorf("unsupported selected file URI: %q", raw)
		}
		if uri.Path == "" {
			return nil, fmt.Errorf("selected file URI has no path: %q", raw)
		}
		paths = append(paths, filepath.FromSlash(uri.Path))
	}
	return paths, nil
}
