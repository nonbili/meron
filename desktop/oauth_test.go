package main

import (
	"net"
	"net/http"
	"testing"
	"time"
)

func TestOAuthCallbackChecksStateBeforeFinishing(t *testing.T) {
	listener, err := net.Listen("tcp", oauthLoopbackHost+":0")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = listener.Close() })
	redirect := "http://" + listener.Addr().String()
	complete := make(chan error, 1)
	app := &App{}
	go app.serveOAuthRedirect(listener, "google_calendar", "expected", "verifier", redirect, func(_ *ExchangedProfile, err error) {
		complete <- err
	})

	client := &http.Client{Timeout: 2 * time.Second}
	for _, state := range []string{"wrong", "expected"} {
		res, err := client.Get(redirect + "/?error=access_denied&state=" + state)
		if err != nil {
			t.Fatal(err)
		}
		res.Body.Close()
		if res.StatusCode != http.StatusBadRequest {
			t.Fatalf("callback status = %d, want 400", res.StatusCode)
		}
	}

	select {
	case err := <-complete:
		if err == nil || err.Error() != "Sign-in was declined" {
			t.Fatalf("unexpected sign-in result: %v", err)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("valid callback did not finish sign-in")
	}
}
