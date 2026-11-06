package main

import "net/http"

// runtimeGuard prevents email images, CSS URLs, and forms from invoking Wails
// commands. The desktop transport accepts query-string calls, but the runtime's
// fetch requests always use POST and a custom header that those elements cannot
// set. This header is a browser request guard, not an authentication secret.
func runtimeGuard(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/wails/runtime" && (r.Method != http.MethodPost || r.Header.Get("x-wails-client-id") == "") {
			http.Error(w, http.StatusText(http.StatusForbidden), http.StatusForbidden)
			return
		}
		next.ServeHTTP(w, r)
	})
}
