package main

import (
	_ "embed"
	"encoding/json"
	"strconv"
	"strings"
	"sync"
)

// wails.json is the single source of truth for the desktop version: the bump
// ritual edits info.productVersion, CI checks the pushed tag against it, and
// the frontend imports the same file for the About dialog. Embedding it here
// gives Go the same number without a second place to forget to update.
//
//go:embed wails.json
var wailsConfigJSON []byte

var (
	appVersionOnce  sync.Once
	appVersionValue string
)

// appVersion returns info.productVersion from the embedded wails.json, e.g.
// "0.1.12". Empty only if the embedded file is somehow unparseable.
func appVersion() string {
	appVersionOnce.Do(func() {
		var config struct {
			Info struct {
				ProductVersion string `json:"productVersion"`
			} `json:"info"`
		}
		if err := json.Unmarshal(wailsConfigJSON, &config); err != nil {
			return
		}
		appVersionValue = strings.TrimSpace(config.Info.ProductVersion)
	})
	return appVersionValue
}

// compareVersions orders two dotted numeric versions ("0.1.12", "v0.2.0"),
// returning -1, 0 or 1. A leading "v" is ignored, missing components count as
// zero (so "0.2" == "0.2.0"), and any non-numeric component compares as zero —
// we only ever ship plain numeric versions, and a malformed remote version must
// not be able to masquerade as newer.
func compareVersions(a, b string) int {
	left := versionParts(a)
	right := versionParts(b)
	for i := 0; i < len(left) || i < len(right); i++ {
		var lv, rv int
		if i < len(left) {
			lv = left[i]
		}
		if i < len(right) {
			rv = right[i]
		}
		if lv != rv {
			if lv < rv {
				return -1
			}
			return 1
		}
	}
	return 0
}

func versionParts(version string) []int {
	trimmed := strings.TrimPrefix(strings.TrimSpace(version), "v")
	// Drop any pre-release/build suffix ("0.1.13-beta.1" -> "0.1.13"); we don't
	// publish them, and treating one as equal to its base release is safer than
	// letting Atoi noise decide.
	if index := strings.IndexAny(trimmed, "-+"); index >= 0 {
		trimmed = trimmed[:index]
	}
	if trimmed == "" {
		return nil
	}
	fields := strings.Split(trimmed, ".")
	parts := make([]int, 0, len(fields))
	for _, field := range fields {
		value, err := strconv.Atoi(field)
		if err != nil {
			value = 0
		}
		parts = append(parts, value)
	}
	return parts
}
