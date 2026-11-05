//go:build linux

package main

import (
	"reflect"
	"testing"

	"github.com/godbus/dbus/v5"
	wailsRuntime "meron/internal/desktopruntime"
)

func TestPortalFilePaths(t *testing.T) {
	got, err := portalFilePaths([]string{
		"file:///run/user/1000/doc/abc/avatar%20photo.png",
		"file://localhost/tmp/other.png",
	})
	if err != nil {
		t.Fatalf("portalFilePaths: %v", err)
	}
	want := []string{
		"/run/user/1000/doc/abc/avatar photo.png",
		"/tmp/other.png",
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("portalFilePaths() = %#v, want %#v", got, want)
	}
}

func TestPortalFilePathsRejectsNonLocalURI(t *testing.T) {
	for _, uri := range []string{
		"https://example.com/avatar.png",
		"file://example.com/avatar.png",
		"file:",
	} {
		if _, err := portalFilePaths([]string{uri}); err == nil {
			t.Errorf("portalFilePaths(%q) returned no error", uri)
		}
	}
}

func TestPortalResponseURIs(t *testing.T) {
	want := []string{"file:///tmp/avatar.png"}
	got, err := portalResponseURIs([]any{
		uint32(0),
		map[string]dbus.Variant{"uris": dbus.MakeVariant(want)},
	})
	if err != nil {
		t.Fatalf("portalResponseURIs: %v", err)
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("portalResponseURIs() = %#v, want %#v", got, want)
	}
}

func TestPortalResponseURIsCancellation(t *testing.T) {
	got, err := portalResponseURIs([]any{uint32(1), map[string]dbus.Variant{}})
	if err != nil {
		t.Fatalf("portalResponseURIs: %v", err)
	}
	if got != nil {
		t.Fatalf("portalResponseURIs() = %#v, want nil", got)
	}
}

func TestPortalOpenFileOptions(t *testing.T) {
	options := portalOpenFileOptions(true, false)
	if multiple, ok := options["multiple"].Value().(bool); !ok || multiple {
		t.Fatalf("multiple = %#v, want false", options["multiple"].Value())
	}
	if signature := options["filters"].Signature().String(); signature != "a(sa(us))" {
		t.Fatalf("filters signature = %q, want %q", signature, "a(sa(us))")
	}
	filters, ok := options["filters"].Value().([]portalFilter)
	if !ok || len(filters) != 1 || len(filters[0].Rules) != 4 {
		t.Fatalf("filters = %#v", options["filters"].Value())
	}
	if token, ok := options["handle_token"].Value().(string); !ok || token == "" {
		t.Fatalf("handle_token = %#v", options["handle_token"].Value())
	}
}

func TestAddPortalFilters(t *testing.T) {
	options := map[string]dbus.Variant{}
	addPortalFilters(options, []wailsRuntime.FileFilter{
		{DisplayName: "OPML files", Pattern: "*.opml; *.xml"},
		{DisplayName: "Empty", Pattern: " ; "},
	})
	want := []portalFilter{{
		Name:  "OPML files",
		Rules: []portalFilterRule{{Kind: 0, Pattern: "*.opml"}, {Kind: 0, Pattern: "*.xml"}},
	}}
	if got := options["filters"].Value(); !reflect.DeepEqual(got, want) {
		t.Fatalf("filters = %#v, want %#v", got, want)
	}
	if sig := options["filters"].Signature().String(); sig != "a(sa(us))" {
		t.Fatalf("filters signature = %s, want a(sa(us))", sig)
	}
	if sig := options["current_filter"].Signature().String(); sig != "(sa(us))" {
		t.Fatalf("current_filter signature = %s, want (sa(us))", sig)
	}

	options = map[string]dbus.Variant{}
	addPortalFilters(options, nil)
	if len(options) != 0 {
		t.Fatalf("addPortalFilters(nil) set %v", options)
	}
}
