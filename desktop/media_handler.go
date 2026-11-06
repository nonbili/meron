package main

import (
	"mime"
	"net/http"
	"path"
	"strings"
)

// mediaContentSecurityPolicy goes on every `/media/` response. The files are
// written from mail a stranger sent (attachments keep the sender's extension),
// and the app origin trusts its own scripts, so a cached `.html` loaded as a
// page could pull in a cached `.js` and drive the app. `sandbox` gives such a
// page an opaque origin with no scripts; the rest only lets an image or video
// opened on its own still render. Ignored when the file is an `<img>`/`<video>`
// source, which is how the app uses it.
const mediaContentSecurityPolicy = "sandbox; default-src 'none'; img-src 'self' data:; " +
	"media-src 'self'; style-src 'unsafe-inline'"

// mediaHandler serves attachment files written by the sidecar at `/media/<key>`,
// so image bytes load straight into the webview instead of round-tripping as
// base64 through the JSON bridge. The AssetServer tries embedded frontend assets
// first and only falls through here for unmatched paths.
func mediaHandler() http.Handler {
	return mediaHandlerAt(mediaDir())
}

func mediaHandlerAt(root string) http.Handler {
	// http.Dir cleans the path and blocks `..` traversal out of the media root.
	dir := http.Dir(root)
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		key, ok := strings.CutPrefix(r.URL.Path, "/media/")
		if !ok || key == "" || strings.HasSuffix(key, "/") {
			http.NotFound(w, r)
			return
		}
		if r.Method != http.MethodGet && r.Method != http.MethodHead {
			w.Header().Set("Allow", "GET, HEAD")
			http.Error(w, http.StatusText(http.StatusMethodNotAllowed), http.StatusMethodNotAllowed)
			return
		}
		file, err := dir.Open("/" + key)
		if err != nil {
			http.NotFound(w, r)
			return
		}
		defer file.Close()
		// Files only: no directory listings of the cache.
		info, err := file.Stat()
		if err != nil || info.IsDir() {
			http.NotFound(w, r)
			return
		}
		header := w.Header()
		header.Set("Content-Security-Policy", mediaContentSecurityPolicy)
		header.Set("X-Content-Type-Options", "nosniff")
		header.Set("Content-Type", mediaContentType(key))
		http.ServeContent(w, r, info.Name(), info.ModTime(), file)
	})
}

// mediaContentType keeps the real type only for what the app renders from the
// cache (images, audio, video). Anything else, `.html`, `.js`, `.xml` included,
// goes out as opaque bytes, so it can never act as a page or a script.
func mediaContentType(key string) string {
	contentType := mime.TypeByExtension(strings.ToLower(path.Ext(key)))
	mediaType, _, err := mime.ParseMediaType(contentType)
	if err == nil {
		for _, prefix := range []string{"image/", "audio/", "video/"} {
			if strings.HasPrefix(mediaType, prefix) {
				return contentType
			}
		}
	}
	return "application/octet-stream"
}
