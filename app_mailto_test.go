package main

import (
	"reflect"
	"testing"
)

func TestIsMailtoURL(t *testing.T) {
	cases := []struct {
		raw  string
		want bool
	}{
		{"mailto:a@x.com", true},
		{"MAILTO:a@x.com?subject=Hi", true},
		{"  mailto:a@x.com  ", true},
		{"https://example.com", false},
		{"a@x.com", false},
		{"", false},
	}
	for _, c := range cases {
		if got := isMailtoURL(c.raw); got != c.want {
			t.Errorf("isMailtoURL(%q) = %v, want %v", c.raw, got, c.want)
		}
	}
}

func TestMailtoURLsFiltersArgs(t *testing.T) {
	args := []string{"--flag", "mailto:a@x.com", "/some/path", "MAILTO:b@y.com"}
	want := []string{"mailto:a@x.com", "MAILTO:b@y.com"}
	if got := mailtoURLs(args); !reflect.DeepEqual(got, want) {
		t.Errorf("mailtoURLs(%v) = %v, want %v", args, got, want)
	}
	if got := mailtoURLs(nil); len(got) != 0 {
		t.Errorf("mailtoURLs(nil) = %v, want empty", got)
	}
}

func TestConsumePendingMailtoReturnsEmptySlice(t *testing.T) {
	app := &App{}
	if got := app.consumePendingMailto(); got == nil || len(got) != 0 {
		t.Fatalf("consumePendingMailto() = %#v, want empty non-nil slice", got)
	}
}
