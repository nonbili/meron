package main

import (
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"testing"
)

func TestRuntimeGuard(t *testing.T) {
	args := `{"call-id":"poc","methodName":"main.desktopService.Invoke","args":["storage.usage",{}]}`
	payloadURL := "/wails/runtime?" + url.Values{
		"object": {"0"}, "method": {"0"}, "args": {args},
	}.Encode()
	for _, test := range []struct {
		name, method, path, clientID, contentType, body string
		allowed                                         bool
	}{
		{name: "email image or CSS URL", method: "GET", path: payloadURL},
		{name: "HEAD", method: "HEAD", path: payloadURL},
		{name: "GET with client header", method: "GET", path: payloadURL, clientID: "client"},
		{name: "form POST", method: "POST", path: payloadURL, contentType: "application/x-www-form-urlencoded", body: "x-wails-client-id=client"},
		{name: "JSON without client header", method: "POST", path: payloadURL, contentType: "application/json", body: `{}`},
		{name: "OPTIONS", method: "OPTIONS", path: "/wails/runtime", clientID: "client"},
		{name: "runtime POST", method: "POST", path: "/wails/runtime", clientID: "client", contentType: "application/json", body: `{"object":0,"method":0,"args":` + args + `}`, allowed: true},
		{name: "runtime query fallback", method: "POST", path: payloadURL, clientID: "client", allowed: true},
		{name: "frontend", method: "GET", path: "/", allowed: true},
		{name: "runtime script", method: "GET", path: "/wails/runtime.js", allowed: true},
		{name: "attachment", method: "GET", path: "/media/attachment", allowed: true},
	} {
		t.Run(test.name, func(t *testing.T) {
			called := false
			handler := runtimeGuard(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				called = true
				w.WriteHeader(http.StatusNoContent)
			}))
			request := httptest.NewRequest(test.method, test.path, strings.NewReader(test.body))
			request.Header.Set("x-wails-client-id", test.clientID)
			request.Header.Set("Content-Type", test.contentType)
			response := httptest.NewRecorder()
			handler.ServeHTTP(response, request)
			wantStatus := http.StatusForbidden
			if test.allowed {
				wantStatus = http.StatusNoContent
			}
			if response.Code != wantStatus || called != test.allowed {
				t.Fatalf("status = %d, downstream called = %v; want %d, %v", response.Code, called, wantStatus, test.allowed)
			}
		})
	}
}
