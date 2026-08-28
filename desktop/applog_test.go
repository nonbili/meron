package main

import (
	"bytes"
	"log"
	"strings"
	"testing"
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
