package main

import (
	"bytes"
	"log"
	"strings"
	"testing"
	"time"
)

func TestRecoverInvokeTurnsPanicIntoError(t *testing.T) {
	var buf bytes.Buffer
	app := &App{logger: log.New(&buf, "", 0)}

	call := func() (err error) {
		defer app.recoverInvoke("mail.send", &err)
		panic("nil map write")
	}

	err := call()
	if err == nil {
		t.Fatal("recoverInvoke swallowed the panic without returning an error")
	}
	if !strings.Contains(err.Error(), "mail.send") || !strings.Contains(err.Error(), "nil map write") {
		t.Fatalf("error = %q, want it to name the command and the panic", err)
	}
	logged := buf.String()
	if !strings.Contains(logged, "invoke mail.send panicked") {
		t.Fatalf("log = %q, want the panic recorded", logged)
	}
	if !strings.Contains(logged, "recoverInvoke") {
		t.Fatalf("log = %q, want a stack trace so the crash stays diagnosable", logged)
	}
}

func TestRecoverInvokeLeavesSuccessfulCallsAlone(t *testing.T) {
	var buf bytes.Buffer
	app := &App{logger: log.New(&buf, "", 0)}

	call := func() (err error) {
		defer app.recoverInvoke("mail.list", &err)
		return nil
	}

	if err := call(); err != nil {
		t.Fatalf("err = %v, want nil", err)
	}
	if buf.Len() != 0 {
		t.Fatalf("log = %q, want nothing logged for a clean call", buf.String())
	}
}

func TestCoreLogLinesAreTimestamped(t *testing.T) {
	var buf bytes.Buffer
	stamp := time.Date(2026, 11, 14, 21, 59, 44, 123456000, time.Local)
	w := newTimestampLines(&buf, func() time.Time { return stamp })

	// A line split across writes keeps one stamp; two lines in a write get one each.
	for _, chunk := range []string{"meron-core [WARN] net: po", "oled drop\nmeron-core: a\nmeron-core: b\n", "partial"} {
		if n, err := w.Write([]byte(chunk)); err != nil || n != len(chunk) {
			t.Fatalf("Write(%q) = %d, %v", chunk, n, err)
		}
	}
	want := "2026/11/14 21:59:44.123456 meron-core [WARN] net: pooled drop\n" +
		"2026/11/14 21:59:44.123456 meron-core: a\n" +
		"2026/11/14 21:59:44.123456 meron-core: b\n"
	if got := buf.String(); got != want {
		t.Fatalf("log =\n%s\nwant\n%s", got, want)
	}
}

func TestCoreLogFlushesAnOverlongLine(t *testing.T) {
	var buf bytes.Buffer
	w := newTimestampLines(&buf, time.Now)
	long := strings.Repeat("x", maxPendingLine)
	if _, err := w.Write([]byte(long)); err != nil {
		t.Fatal(err)
	}
	if got := strings.Count(buf.String(), "x"); got != maxPendingLine || !strings.HasSuffix(buf.String(), "\n") {
		t.Fatalf("flushed %d bytes (want %d) ending %q", got, maxPendingLine, buf.String()[buf.Len()-1:])
	}
}

func TestRedactLogMasksEmailsAndHome(t *testing.T) {
	text := "startup: core path=/home/alex/.cache/meron/bin/meron-core\n" +
		"error: idle alex.doe@example.com/INBOX: dns lookup failed\n" +
		"backup.import: restored 2 accounts from /home/alex/Documents/meron.json\n" +
		"windows: C:\\Users\\alex\\AppData\\Roaming\\meron\\meron.log"
	got := redactLogEmails(redactLogHome(text, "/home/alex/"))
	got = redactLogHome(got, `C:\Users\alex`)
	want := "startup: core path=~/.cache/meron/bin/meron-core\n" +
		"error: idle a***@example.com/INBOX: dns lookup failed\n" +
		"backup.import: restored 2 accounts from ~/Documents/meron.json\n" +
		"windows: ~\\AppData\\Roaming\\meron\\meron.log"
	if got != want {
		t.Fatalf("redacted =\n%s\nwant\n%s", got, want)
	}
	if redactLogHome("/a/b", "/") != "/a/b" {
		t.Fatal("a root home directory must not be replaced throughout the log")
	}
}
