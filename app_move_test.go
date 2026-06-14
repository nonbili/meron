package main

import (
	"reflect"
	"testing"
)

func TestUIDsByFolderForSubjectThreadUsesFallbackFolder(t *testing.T) {
	res := map[string]any{
		"messages": []any{
			map[string]any{
				"uid": float64(11),
				"message": map[string]any{
					"subject": "Re: Todo",
				},
			},
			map[string]any{
				"uid":    float64(12),
				"folder": "Sent",
				"message": map[string]any{
					"subject": "Todo",
				},
			},
			map[string]any{
				"uid": float64(13),
				"message": map[string]any{
					"subject": "Other",
				},
			},
		},
	}

	got := uidsByFolderForSubjectThread(res, "INBOX", threadGroupingSubject("Todo"))
	want := map[string][]uint32{
		"INBOX": {11},
		"Sent":  {12},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("uidsByFolderForSubjectThread() = %#v, want %#v", got, want)
	}
}

func TestUIDsByFolderForSubjectThreadUsesHeaders(t *testing.T) {
	res := map[string]any{
		"headers": []any{
			map[string]any{
				"uid":     float64(21),
				"folder":  "Trash",
				"subject": "Re: Todo",
			},
			map[string]any{
				"uid":     float64(22),
				"folder":  "Sent",
				"subject": "Other",
			},
			map[string]any{
				"uid":     float64(23),
				"subject": "Todo",
			},
		},
	}

	got := uidsByFolderForSubjectThread(res, "INBOX", threadGroupingSubject("Todo"))
	want := map[string][]uint32{
		"Trash": {21},
		"INBOX": {23},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("uidsByFolderForSubjectThread() = %#v, want %#v", got, want)
	}
}

func TestThreadGroupingSubjectStripsGatewayTags(t *testing.T) {
	base := threadGroupingSubject("Regarding NouTube certification report")
	cases := []string{
		"[EXTERNAL] Regarding NouTube certification report",
		"Re: [EXTERNAL] Regarding NouTube certification report",
		"[EXTERNAL] Re: Regarding NouTube certification report",
		"[EXTERNAL][CAUTION] Regarding NouTube certification report",
	}
	for _, c := range cases {
		if got := threadGroupingSubject(c); got != base {
			t.Errorf("threadGroupingSubject(%q) = %q, want %q", c, got, base)
		}
	}
}

func TestThreadGroupingVsDisplayPreservesRealTags(t *testing.T) {
	// The display variant keeps a legitimate tag in the title; the grouping
	// variant drops any leading bracket tag for matching purposes.
	if got := normalizeThreadSubject("[github] build failed"); got != "[github] build failed" {
		t.Errorf("normalizeThreadSubject kept tag: got %q", got)
	}
	if got := threadGroupingSubject("[github] build failed"); got != "build failed" {
		t.Errorf("threadGroupingSubject(%q) = %q, want %q", "[github] build failed", got, "build failed")
	}
}
