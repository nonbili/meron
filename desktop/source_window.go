package main

import (
	"bytes"
	"crypto/rand"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"mime"
	"net/mail"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"sync"
	"unicode/utf8"

	"github.com/wailsapp/wails/v3/pkg/application"
	wailsRuntime "meron/internal/desktopruntime"
)

// The most of a message's source the window shows. The rest, attachments
// mostly, is only in the saved .eml: megabytes of base64 help no one reading.
const sourceViewLimit = 2 << 20

// sourceViewer is one "Show original" window: the message it shows, and once
// fetched, the raw bytes in its own directory under the app's _eml cache,
// which startup and quit sweep, so a crash leaves no message behind. The
// window opens at once and asks for the source itself, so a slow server never
// holds up the conversation the request came from.
type sourceViewer struct {
	key     string
	params  map[string]any
	subject string
	dir     string
	window  emlReaderWindow

	loadMu   sync.Mutex
	loaded   bool
	data     map[string]any
	attempts int
	// The file of the fetch that loaded. Each attempt writes its own: one that
	// timed out here may still finish in the core, and must not rewrite a file
	// being shown or saved.
	raw string
}

func (v *sourceViewer) rawPath() string {
	v.loadMu.Lock()
	defer v.loadMu.Unlock()
	return v.raw
}

// openMessageSource opens a window showing one message's original RFC 822
// source, or brings back the one already showing it.
func (a *App) openMessageSource(payload map[string]any) (any, error) {
	params, err := messageEmlSource(payload)
	if err != nil {
		return nil, err
	}
	key := fmt.Sprintf("%v\x00%v\x00%v", params["account"], params["folder"], params["uid"])
	// One open at a time, so two requests for one message can't both miss the
	// other's window; the registry lock itself is never held over native calls.
	a.sourceOpenMu.Lock()
	defer a.sourceOpenMu.Unlock()
	a.sourceMu.Lock()
	for _, viewer := range a.sourceViewers {
		if viewer.key == key && viewer.window != nil {
			window := viewer.window
			a.sourceMu.Unlock()
			window.Show()
			window.Focus()
			return map[string]any{"opened": true}, nil
		}
	}
	a.sourceMu.Unlock()
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	root := filepath.Join(mediaDir(), "_eml")
	if err := os.MkdirAll(root, 0o700); err != nil {
		return nil, err
	}
	dir, err := os.MkdirTemp(root, "source-")
	if err != nil {
		return nil, err
	}
	buf := make([]byte, 12)
	if _, err := rand.Read(buf); err != nil {
		_ = os.RemoveAll(dir)
		return nil, err
	}
	token := "source-" + hex.EncodeToString(buf)
	subject, _ := payload["subject"].(string)
	viewer := &sourceViewer{key: key, params: params, subject: subject, dir: dir}
	a.sourceMu.Lock()
	if a.sourceViewers == nil {
		a.sourceViewers = make(map[string]*sourceViewer)
	}
	a.sourceViewers[token] = viewer
	a.sourceMu.Unlock()
	title := strings.TrimSpace(subject)
	if title == "" {
		title = "(no subject)"
	}
	window := newEmlReaderWindow(application.WebviewWindowOptions{
		Name: token, Title: title + " — " + appTitle(),
		URL: "/?source=" + token, Width: 900, Height: 720,
		MinWidth: 480, MinHeight: 360, Hidden: true,
	}, func() {
		a.sourceMu.Lock()
		delete(a.sourceViewers, token)
		a.sourceMu.Unlock()
		_ = os.RemoveAll(dir)
	})
	a.sourceMu.Lock()
	viewer.window = window
	a.sourceMu.Unlock()
	window.Show()
	window.Focus()
	return map[string]any{"opened": true}, nil
}

func (a *App) sourceCommand(command string, payload map[string]any) (any, error) {
	if command == "source.open" {
		return a.openMessageSource(payload)
	}
	token, _ := payload["session"].(string)
	a.sourceMu.Lock()
	viewer := a.sourceViewers[token]
	a.sourceMu.Unlock()
	if viewer == nil {
		return nil, errors.New("this window is closed")
	}
	switch command {
	case "source.get":
		return a.loadMessageSource(viewer)
	case "source.save":
		return a.saveMessageSource(viewer)
	case "source.close":
		if viewer.window != nil {
			viewer.window.Close()
		}
		return nil, nil
	}
	return nil, fmt.Errorf("unknown command %s", command)
}

// loadMessageSource fetches the message once per window; a second request
// (a reload, or React's double effect) waits for the first and shares it.
func (a *App) loadMessageSource(viewer *sourceViewer) (any, error) {
	viewer.loadMu.Lock()
	defer viewer.loadMu.Unlock()
	if viewer.loaded {
		return viewer.data, nil
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	params := make(map[string]any, len(viewer.params)+1)
	for k, v := range viewer.params {
		params[k] = v
	}
	viewer.attempts++
	path := filepath.Join(viewer.dir, fmt.Sprintf("attempt-%d.eml", viewer.attempts))
	params["path"] = path
	if _, err := a.sidecar.Call("messages.saveRaw", params); err != nil {
		// Not remembered: the next request tries the server again.
		return nil, err
	}
	data, err := readMessageSource(path, sourceViewLimit)
	if err != nil {
		return nil, err
	}
	viewer.data, viewer.loaded, viewer.raw = data, true, path
	return data, nil
}

func (a *App) saveMessageSource(viewer *sourceViewer) (any, error) {
	raw := viewer.rawPath()
	if raw == "" {
		return nil, errors.New("the message has not loaded yet")
	}
	var parent *application.WebviewWindow
	if window, ok := viewer.window.(*application.WebviewWindow); ok {
		parent = window
	}
	dest, err := saveFileDialog(a.ctx, wailsRuntime.SaveDialogOptions{
		Window: parent, Title: "Save message", DefaultFilename: emlFilename(viewer.subject), CanCreateDirectories: true,
	})
	if err != nil {
		return nil, err
	}
	if dest == "" {
		return map[string]any{"saved": false}, nil
	}
	if err := writeMessageSource(raw, dest); err != nil {
		return nil, err
	}
	return map[string]any{"saved": true, "path": dest}, nil
}

// writeMessageSource copies the fetched message to dest. The copy is held open
// before dest is touched: closing the window meanwhile deletes it, and that
// must not leave dest emptied in its place. It is written to a private file
// beside dest that replaces it only once complete, so a failed write (a full
// disk) leaves a file being replaced as it was.
func writeMessageSource(src, dest string) error {
	in, err := os.Open(src)
	if err != nil {
		return fmt.Errorf("the message is no longer available: %w", err)
	}
	defer in.Close()
	staged, err := os.CreateTemp(filepath.Dir(dest), ".meron-save-*")
	if err != nil {
		// A sandboxed file chooser grants only the chosen path itself. Any
		// other failure (a full disk) leaves dest alone.
		if !errors.Is(err, fs.ErrPermission) {
			return err
		}
		return writeMessageSourceInPlace(in, dest)
	}
	stagedPath := staged.Name()
	defer os.Remove(stagedPath)
	// CreateTemp makes it private: a message is no one else's to read.
	if _, err := io.Copy(staged, in); err != nil {
		_ = staged.Close()
		return err
	}
	if err := staged.Close(); err != nil {
		return err
	}
	return os.Rename(stagedPath, dest)
}

func writeMessageSourceInPlace(in io.Reader, dest string) error {
	out, err := os.OpenFile(dest, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, 0o600)
	if err != nil {
		return err
	}
	if _, err := io.Copy(out, in); err != nil {
		_ = out.Close()
		return err
	}
	return out.Close()
}

// readMessageSource reads up to limit bytes of a raw message for display,
// with a summary of its headers: who and when, and how the receiving server
// judged where it came from (Authentication-Results).
func readMessageSource(path string, limit int64) (map[string]any, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer file.Close()
	info, err := file.Stat()
	if err != nil {
		return nil, err
	}
	shown, err := io.ReadAll(io.LimitReader(file, limit))
	if err != nil {
		return nil, err
	}
	truncated := info.Size() > int64(len(shown))
	if truncated {
		shown = withoutCutCharacter(shown)
	}
	server, results := sourceAuthResults(shown)
	// A NUL is valid UTF-8 but ends the text for the native clipboard on
	// Linux, so it is shown as the visible ␀ and the text counts as inexact.
	text := string(shown)
	hasNul := strings.ContainsRune(text, 0)
	if hasNul {
		text = strings.ReplaceAll(text, "\x00", "\u2400")
	}
	data := map[string]any{
		"size":      info.Size(),
		"source":    text,
		"truncated": truncated,
		"shown":     len(shown),
		// Bytes that aren't UTF-8 (8-bit legacy charsets) arrive as U+FFFD, so
		// the text shown and copied isn't the message byte for byte.
		"lossy":      hasNul || !utf8.Valid(shown),
		"headers":    sourceHeaderSummary(shown),
		"auth":       results,
		"authServer": server,
	}
	return data, nil
}

// sourceHeaderSummary pulls the headers people look a message up for, decoded
// for reading. Missing ones are left out.
func sourceHeaderSummary(raw []byte) []map[string]string {
	header := sourceHeader(raw)
	if header == nil {
		return []map[string]string{}
	}
	decoder := new(mime.WordDecoder)
	summary := []map[string]string{}
	for _, name := range []string{"From", "Reply-To", "To", "Cc", "Date", "Subject", "Message-ID"} {
		value := strings.TrimSpace(header.Get(name))
		if value == "" {
			continue
		}
		if decoded, err := decoder.DecodeHeader(value); err == nil {
			value = decoded
		}
		summary = append(summary, map[string]string{"name": name, "value": value})
	}
	return summary
}

// authResult is one verdict from an Authentication-Results header: the method
// (spf, dkim, dmarc), its result, and the domain it vouches for when given.
type authResult struct {
	Method string `json:"method"`
	Result string `json:"result"`
	Domain string `json:"domain,omitempty"`
}

// sourceAuthResults reads the SPF, DKIM and DMARC verdicts out of the topmost
// Authentication-Results header, and the server that wrote it (its
// authserv-id). That is normally the receiving server, but nothing proves it:
// a server that adds none passes on whatever the sender wrote, which is why
// the window names the server beside the verdicts. Every verdict is kept: a
// message signed twice has a DKIM result for each signature.
func sourceAuthResults(raw []byte) (string, []authResult) {
	results := []authResult{}
	header := sourceHeader(raw)
	if header == nil {
		return "", results
	}
	values := header["Authentication-Results"]
	if len(values) == 0 {
		return "", results
	}
	clauses := strings.Split(withoutHeaderComments(values[0]), ";")
	server := strings.Fields(clauses[0])
	// Without its server the header is malformed, and the window would have
	// no one to name beside the verdicts: they stay in the raw source only.
	if len(server) == 0 {
		return "", results
	}
	for _, clause := range clauses[1:] {
		match := authResultClause.FindStringSubmatch(clause)
		if match == nil {
			continue
		}
		result := authResult{Method: strings.ToLower(match[1]), Result: strings.ToLower(match[2])}
		if result.Method != "spf" && result.Method != "dkim" && result.Method != "dmarc" {
			continue
		}
		result.Domain = authResultDomain(result.Method, clause[len(match[0]):])
		// Each clause stays: two signatures may agree in everything shown.
		results = append(results, result)
	}
	return server[0], results
}

// A result clause starts "method = result", with optional space around "="
// and an optional method version, as in "dkim/1=pass" (RFC 8601).
var authResultClause = regexp.MustCompile(`^\s*([A-Za-z0-9-]+)(?:\s*/\s*[0-9]+)?\s*=\s*([A-Za-z0-9-]+)`)

// The properties naming the domain each verdict is about, best first: the
// DKIM signing domain (header.d, the identity header.i only without it), the
// SPF envelope sender or else the HELO name, the DMARC From domain.
var authResultDomainProperties = map[string][]string{
	"dkim":  {"header.d", "header.i"},
	"spf":   {"smtp.mailfrom", "smtp.helo"},
	"dmarc": {"header.from"},
}

// A property, "ptype.name = value", where the value may be quoted.
var authResultProperty = regexp.MustCompile(`(?i)(?:^|\s)([a-z0-9-]+\.[a-z0-9-]+)\s*=\s*("(?:[^"\\]|\\.)*"|[^\s;"]+)`)

// authResultDomain finds the domain a verdict is about among its properties.
// A property's value may be quoted; text inside another quoted value, such as
// a reason="...", is never taken for a property.
func authResultDomain(method, properties string) string {
	inQuotes := quotedSpans(properties)
	found := map[string]string{}
	for _, match := range authResultProperty.FindAllStringSubmatchIndex(properties, -1) {
		if inQuotes[match[2]] {
			continue
		}
		name := strings.ToLower(properties[match[2]:match[3]])
		if _, ok := found[name]; !ok {
			found[name] = strings.Trim(properties[match[4]:match[5]], `"`)
		}
	}
	for _, name := range authResultDomainProperties[method] {
		if value := found[name]; value != "" {
			if at := strings.LastIndex(value, "@"); at >= 0 {
				value = value[at+1:]
			}
			return value
		}
	}
	return ""
}

// quotedSpans marks each byte of value that lies inside a "quoted string".
func quotedSpans(value string) []bool {
	inside := make([]bool, len(value)+1)
	quoted, escaped := false, false
	for i := 0; i < len(value); i++ {
		c := value[i]
		switch {
		case escaped:
			escaped = false
		case quoted && c == '\\':
			escaped = true
		case c == '"':
			quoted = !quoted
			inside[i] = true
			continue
		}
		inside[i] = quoted
	}
	return inside
}

// withoutHeaderComments blanks out a header's (comments), which may nest and
// hold a ";", and the ";" inside "quoted strings", so splitting on ";" finds
// only the real clause boundaries.
func withoutHeaderComments(value string) string {
	var out strings.Builder
	depth, quoted, escaped := 0, false, false
	for _, r := range value {
		switch {
		case escaped:
			escaped = false
			if depth == 0 {
				out.WriteRune(' ')
			}
			continue
		case r == '\\' && (quoted || depth > 0):
			escaped = true
			continue
		case quoted:
			if r == '"' {
				quoted = false
			}
			if r == ';' {
				r = ' '
			}
		case r == '(':
			depth++
			continue
		case r == ')' && depth > 0:
			depth--
			if depth == 0 {
				out.WriteRune(' ')
			}
			continue
		case depth > 0:
			continue
		case r == '"':
			quoted = true
		}
		out.WriteRune(r)
	}
	return out.String()
}

// withoutCutCharacter drops the start of a character the display limit cut
// through, which would otherwise show as U+FFFD though nothing is wrong with
// the message. Bytes that are invalid for any other reason stay, and are
// flagged as lossy.
func withoutCutCharacter(shown []byte) []byte {
	for back := 1; back < utf8.UTFMax && back <= len(shown); back++ {
		start := shown[len(shown)-back]
		if utf8.RuneStart(start) {
			if !utf8.FullRune(shown[len(shown)-back:]) {
				return shown[:len(shown)-back]
			}
			return shown
		}
	}
	return shown
}

// sourceHeader parses the header block, which may end in the shown part
// only: a message's headers come before its body.
func sourceHeader(raw []byte) mail.Header {
	end := bytes.Index(raw, []byte("\r\n\r\n"))
	if alt := bytes.Index(raw, []byte("\n\n")); alt >= 0 && (end < 0 || alt < end) {
		end = alt
	}
	block := raw
	if end >= 0 {
		block = raw[:end]
	}
	message, err := mail.ReadMessage(bytes.NewReader(append(append([]byte{}, block...), "\r\n\r\n"...)))
	if err != nil {
		return nil
	}
	return message.Header
}
