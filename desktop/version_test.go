package main

import (
	"encoding/json"
	"os"
	"testing"
)

func TestAppVersionMatchesWailsConfig(t *testing.T) {
	raw, err := os.ReadFile("wails.json")
	if err != nil {
		t.Fatalf("read wails.json: %v", err)
	}
	var config struct {
		Info struct {
			ProductVersion string `json:"productVersion"`
		} `json:"info"`
	}
	if err := json.Unmarshal(raw, &config); err != nil {
		t.Fatalf("parse wails.json: %v", err)
	}
	if config.Info.ProductVersion == "" {
		t.Fatal("wails.json has no info.productVersion")
	}
	if got := appVersion(); got != config.Info.ProductVersion {
		t.Fatalf("appVersion() = %q, want %q", got, config.Info.ProductVersion)
	}
}

func TestCompareVersions(t *testing.T) {
	cases := []struct {
		a, b string
		want int
	}{
		{"0.1.12", "0.1.12", 0},
		{"0.1.13", "0.1.12", 1},
		{"0.1.12", "0.1.13", -1},
		{"0.2.0", "0.1.99", 1},
		{"1.0.0", "0.9.9", 1},
		// A leading v and missing components are normalized away.
		{"v0.1.13", "0.1.13", 0},
		{"0.2", "0.2.0", 0},
		{"0.2.1", "0.2", 1},
		// Double-digit components must not compare lexically.
		{"0.1.9", "0.1.10", -1},
		// Pre-release suffixes collapse to their base version, so a malformed
		// remote version can never look newer than it is.
		{"0.1.13-beta.1", "0.1.13", 0},
		{"", "0.1.12", -1},
		{"garbage", "0.0.0", 0},
	}
	for _, tc := range cases {
		if got := compareVersions(tc.a, tc.b); got != tc.want {
			t.Errorf("compareVersions(%q, %q) = %d, want %d", tc.a, tc.b, got, tc.want)
		}
	}
}
