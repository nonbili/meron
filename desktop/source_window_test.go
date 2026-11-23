package main

import (
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"strings"
	"testing"
)

const sourceSample = "Authentication-Results: mx.example.org;\r\n" +
	"\tspf=pass (sender allowed) smtp.mailfrom=alice@example.com;\r\n" +
	"\tdkim=fail header.d=example.com;\r\n" +
	"\tdmarc=PASS header.from=example.com\r\n" +
	"Authentication-Results: forged.example.net; spf=neutral; dkim=pass\r\n" +
	"From: =?UTF-8?Q?Alice_M=C3=BCller?= <alice@example.com>\r\n" +
	"To: Bob <bob@example.net>\r\n" +
	"Date: Mon, 23 Nov 2026 09:30:00 +0000\r\n" +
	"Subject: =?UTF-8?B?SGVsbG8g8J+Riw==?=\r\n" +
	"Message-ID: <abc@example.com>\r\n" +
	"\r\n" +
	"Body line one\r\n"

func TestSourceHeaderSummaryDecodesTheUsualHeaders(t *testing.T) {
	summary := sourceHeaderSummary([]byte(sourceSample))
	got := map[string]string{}
	var order []string
	for _, row := range summary {
		got[row["name"]] = row["value"]
		order = append(order, row["name"])
	}
	if strings.Join(order, ",") != "From,To,Date,Subject,Message-ID" {
		t.Fatalf("unexpected headers %v", order)
	}
	if got["From"] != "Alice Müller <alice@example.com>" {
		t.Fatalf("From not decoded: %q", got["From"])
	}
	if got["Subject"] != "Hello 👋" {
		t.Fatalf("Subject not decoded: %q", got["Subject"])
	}
}

func TestSourceAuthResultsReadsOnlyTheTopmostHeader(t *testing.T) {
	server, results := sourceAuthResults([]byte(sourceSample))
	if server != "mx.example.org" {
		t.Fatalf("server = %q", server)
	}
	want := []authResult{
		{Method: "spf", Result: "pass", Domain: "example.com"},
		{Method: "dkim", Result: "fail", Domain: "example.com"},
		{Method: "dmarc", Result: "pass", Domain: "example.com"},
	}
	if !reflect.DeepEqual(results, want) {
		t.Fatalf("got %v, want %v", results, want)
	}
}

func TestSourceAuthResultsKeepsEachDKIMSignature(t *testing.T) {
	raw := "Authentication-Results: mx.example.org;\r\n" +
		"\tdkim=pass header.d=example.com;\r\n" +
		"\tdkim=fail header.d=mailer.example.net;\r\n" +
		"\tdkim=pass header.d=example.com\r\n" +
		"From: a@example.com\r\n\r\nBody\r\n"
	_, results := sourceAuthResults([]byte(raw))
	want := []authResult{
		{Method: "dkim", Result: "pass", Domain: "example.com"},
		{Method: "dkim", Result: "fail", Domain: "mailer.example.net"},
		{Method: "dkim", Result: "pass", Domain: "example.com"},
	}
	if !reflect.DeepEqual(results, want) {
		t.Fatalf("got %v, want %v", results, want)
	}
}

func TestSourceAuthResultsWithoutTheHeader(t *testing.T) {
	if server, results := sourceAuthResults([]byte("From: a@example.com\n\nBody\n")); server != "" || len(results) != 0 {
		t.Fatalf("expected no results, got %q %v", server, results)
	}
}

func TestSourceAuthResultsIgnoresCommentsAndAllowsSpaceAroundEquals(t *testing.T) {
	raw := "Authentication-Results: mx.example.org 1;\r\n" +
		"\tspf=pass (looks fine; dkim=pass (nested) too) smtp.mailfrom=a@example.com;\r\n" +
		"\tdkim = fail reason=\"bad; signature\";\r\n" +
		"\tdmarc=none\r\n" +
		"From: a@example.com\r\n\r\nBody\r\n"
	server, results := sourceAuthResults([]byte(raw))
	if server != "mx.example.org" {
		t.Fatalf("server = %q", server)
	}
	want := []authResult{
		{Method: "spf", Result: "pass", Domain: "example.com"},
		{Method: "dkim", Result: "fail"},
		{Method: "dmarc", Result: "none"},
	}
	if !reflect.DeepEqual(results, want) {
		t.Fatalf("got %v, want %v", results, want)
	}
}

func TestReadMessageSourceFlagsBytesThatAreNotUTF8(t *testing.T) {
	dir := t.TempDir()
	latin1 := filepath.Join(dir, "latin1.eml")
	if err := os.WriteFile(latin1, []byte("Subject: caf\xe9\r\n\r\nBody\r\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	data, err := readMessageSource(latin1, sourceViewLimit)
	if err != nil {
		t.Fatal(err)
	}
	if data["lossy"] != true {
		t.Fatal("expected a Latin-1 byte to be flagged")
	}
	// Cutting through a multi-byte character at the limit is not a loss.
	cut := filepath.Join(dir, "cut.eml")
	raw := "Subject: Hello\r\n\r\nCaf\u00e9 \U0001F44B"
	if err := os.WriteFile(cut, []byte(raw), 0o600); err != nil {
		t.Fatal(err)
	}
	data, err = readMessageSource(cut, int64(len(raw)-2))
	if err != nil {
		t.Fatal(err)
	}
	if data["truncated"] != true || data["lossy"] != false {
		t.Fatalf("truncated=%v lossy=%v", data["truncated"], data["lossy"])
	}
	// The cut character is left out of the text, not shown as U+FFFD.
	if source := data["source"].(string); strings.ContainsRune(source, '\uFFFD') || !strings.HasSuffix(source, "Caf\u00e9 ") {
		t.Fatalf("source ends %q", source[len(source)-8:])
	}
	if data["shown"] != len(raw)-4 {
		t.Fatalf("shown = %v, want %d", data["shown"], len(raw)-4)
	}
}

func TestSourceAuthResultsReadsQuotedValuesAndVersionedMethods(t *testing.T) {
	raw := "Authentication-Results: mx.example.org;\r\n" +
		"\tdkim/1=pass header.d=\"example.org\" header.s=one;\r\n" +
		"\tdkim=pass header.d=example.org header.s=two\r\n" +
		"From: a@example.org\r\n\r\nBody\r\n"
	_, results := sourceAuthResults([]byte(raw))
	want := []authResult{
		{Method: "dkim", Result: "pass", Domain: "example.org"},
		{Method: "dkim", Result: "pass", Domain: "example.org"},
	}
	if !reflect.DeepEqual(results, want) {
		t.Fatalf("got %v, want %v", results, want)
	}
}

func TestSourceAuthResultsNeedsAReportingServer(t *testing.T) {
	raw := "Authentication-Results: ; spf=pass smtp.mailfrom=a@example.com\r\nFrom: a@example.com\r\n\r\nBody\r\n"
	if server, results := sourceAuthResults([]byte(raw)); server != "" || len(results) != 0 {
		t.Fatalf("expected nothing from a header without its server, got %q %v", server, results)
	}
}

func TestSourceAuthResultsTakesEachMethodsOwnDomainOutsideQuotes(t *testing.T) {
	raw := "Authentication-Results: mx.example.org;\r\n" +
		"\tdkim=pass reason=\"header.d=forged.example\" header.i=@news.example.com header.d=example.com;\r\n" +
		"\tspf=pass smtp.helo=relay.example.net\r\n" +
		"From: a@example.com\r\n\r\nBody\r\n"
	_, results := sourceAuthResults([]byte(raw))
	want := []authResult{
		{Method: "dkim", Result: "pass", Domain: "example.com"},
		{Method: "spf", Result: "pass", Domain: "relay.example.net"},
	}
	if !reflect.DeepEqual(results, want) {
		t.Fatalf("got %v, want %v", results, want)
	}
}

func TestWriteMessageSourceLeavesTheDestinationAloneWhenTheCopyIsGone(t *testing.T) {
	dir := t.TempDir()
	dest := filepath.Join(dir, "kept.eml")
	if err := os.WriteFile(dest, []byte("earlier export"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := writeMessageSource(filepath.Join(dir, "closed-window", "message.eml"), dest); err == nil {
		t.Fatal("expected an error once the fetched copy is gone")
	}
	if got, _ := os.ReadFile(dest); string(got) != "earlier export" {
		t.Fatalf("destination changed to %q", got)
	}
}

func TestWriteMessageSourceWritesAPrivateFile(t *testing.T) {
	dir := t.TempDir()
	src := filepath.Join(dir, "message.eml")
	if err := os.WriteFile(src, []byte(sourceSample), 0o600); err != nil {
		t.Fatal(err)
	}
	dest := filepath.Join(dir, "saved.eml")
	if err := writeMessageSource(src, dest); err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(dest)
	if err != nil {
		t.Fatal(err)
	}
	if runtime.GOOS != "windows" && info.Mode().Perm() != 0o600 {
		t.Fatalf("mode = %v", info.Mode().Perm())
	}
	if got, _ := os.ReadFile(dest); string(got) != sourceSample {
		t.Fatal("saved bytes differ from the message")
	}
}

func TestReadMessageSourceCutsALargeMessageButKeepsItsHeaders(t *testing.T) {
	path := filepath.Join(t.TempDir(), "message.eml")
	raw := sourceSample + strings.Repeat("A", 4096)
	if err := os.WriteFile(path, []byte(raw), 0o600); err != nil {
		t.Fatal(err)
	}
	data, err := readMessageSource(path, int64(len(sourceSample)+10))
	if err != nil {
		t.Fatal(err)
	}
	if data["shown"] != len(sourceSample)+10 {
		t.Fatalf("shown = %v", data["shown"])
	}
	if data["truncated"] != true || data["size"] != int64(len(raw)) {
		t.Fatalf("expected a truncated %d-byte message, got %v / %v", len(raw), data["truncated"], data["size"])
	}
	if source := data["source"].(string); len(source) != len(sourceSample)+10 {
		t.Fatalf("shown %d bytes", len(source))
	}
	if headers := data["headers"].([]map[string]string); len(headers) != 5 {
		t.Fatalf("headers lost when the body was cut: %v", headers)
	}
}

func TestReadMessageSourceShowsASmallMessageWhole(t *testing.T) {
	path := filepath.Join(t.TempDir(), "message.eml")
	if err := os.WriteFile(path, []byte(sourceSample), 0o600); err != nil {
		t.Fatal(err)
	}
	data, err := readMessageSource(path, sourceViewLimit)
	if err != nil {
		t.Fatal(err)
	}
	if data["truncated"] != false || data["source"] != sourceSample {
		t.Fatalf("expected the whole message, got truncated=%v", data["truncated"])
	}
}

func TestSourceCommandRefusesAClosedWindow(t *testing.T) {
	app := &App{}
	if _, err := app.sourceCommand("source.get", map[string]any{"session": "source-gone"}); err == nil {
		t.Fatal("expected an error for an unknown session")
	}
}

func TestWriteMessageSourceReplacesAnExistingFileOnlyWhenComplete(t *testing.T) {
	dir := t.TempDir()
	src := filepath.Join(dir, "attempt-1.eml")
	if err := os.WriteFile(src, []byte(sourceSample), 0o600); err != nil {
		t.Fatal(err)
	}
	dest := filepath.Join(dir, "existing.eml")
	if err := os.WriteFile(dest, []byte("earlier export"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := writeMessageSource(src, dest); err != nil {
		t.Fatal(err)
	}
	if got, _ := os.ReadFile(dest); string(got) != sourceSample {
		t.Fatal("destination was not replaced")
	}
	info, _ := os.Stat(dest)
	if runtime.GOOS != "windows" && info.Mode().Perm() != 0o600 {
		t.Fatalf("a replaced file should be private too, got %v", info.Mode().Perm())
	}
	entries, _ := os.ReadDir(dir)
	for _, entry := range entries {
		if strings.HasPrefix(entry.Name(), ".meron-save-") {
			t.Fatalf("staging file left behind: %s", entry.Name())
		}
	}
}

func TestSourceViewerSavesOnlyTheAttemptThatLoaded(t *testing.T) {
	viewer := &sourceViewer{dir: t.TempDir()}
	viewer.attempts = 1
	viewer.raw = filepath.Join(viewer.dir, "attempt-1.eml")
	if got := viewer.rawPath(); got != viewer.raw {
		t.Fatalf("rawPath = %q", got)
	}
	if (&sourceViewer{}).rawPath() != "" {
		t.Fatal("an unloaded viewer has no file to save")
	}
}

func TestReadMessageSourceShowsNulAsAVisibleSymbol(t *testing.T) {
	path := filepath.Join(t.TempDir(), "message.eml")
	if err := os.WriteFile(path, []byte("Subject: x\r\n\r\nbefore\x00after\r\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	data, err := readMessageSource(path, sourceViewLimit)
	if err != nil {
		t.Fatal(err)
	}
	if source := data["source"].(string); !strings.Contains(source, "before\u2400after") {
		t.Fatalf("source = %q", source)
	}
	if data["lossy"] != true {
		t.Fatal("a NUL should mark the text as inexact")
	}
}
