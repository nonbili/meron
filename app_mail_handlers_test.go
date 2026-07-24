package main

import (
	"encoding/json"
	"io"
	"reflect"
	"testing"
)

type sidecarCallRecord struct {
	Method string
	Params map[string]any
}

type sidecarResponsePlan struct {
	Result any
	Error  any
}

type recordingSidecarWriter struct {
	t         *testing.T
	sidecar   *Sidecar
	calls     []sidecarCallRecord
	responses []sidecarResponsePlan
}

func newMailHandlerTestApp(t *testing.T, responses ...sidecarResponsePlan) (*App, *recordingSidecarWriter) {
	t.Helper()
	sidecar := &Sidecar{
		started: true,
		pending: map[uint64]chan sidecarResponse{},
	}
	writer := &recordingSidecarWriter{t: t, sidecar: sidecar, responses: responses}
	sidecar.stdin = writer
	return &App{sidecar: sidecar}, writer
}

func (w *recordingSidecarWriter) Write(data []byte) (int, error) {
	var req struct {
		ID     uint64         `json:"id"`
		Method string         `json:"method"`
		Params map[string]any `json:"params"`
	}
	if err := json.Unmarshal(data, &req); err != nil {
		w.t.Fatalf("sidecar request JSON: %v", err)
	}
	w.calls = append(w.calls, sidecarCallRecord{Method: req.Method, Params: req.Params})
	if len(w.responses) == 0 {
		w.t.Fatalf("unexpected sidecar call %s with params %#v", req.Method, req.Params)
	}
	response := w.responses[0]
	w.responses = w.responses[1:]
	ch := w.sidecar.pending[req.ID]
	if ch == nil {
		w.t.Fatalf("missing pending response channel for request %d", req.ID)
	}
	ch <- sidecarResponse{Result: response.Result, Error: response.Error}
	return len(data), nil
}

func (w *recordingSidecarWriter) Close() error {
	return nil
}

var _ io.WriteCloser = (*recordingSidecarWriter)(nil)

func assertCall(t *testing.T, got sidecarCallRecord, method string, params map[string]any) {
	t.Helper()
	if got.Method != method {
		t.Fatalf("method = %q, want %q", got.Method, method)
	}
	if !reflect.DeepEqual(got.Params, params) {
		t.Fatalf("%s params = %#v, want %#v", method, got.Params, params)
	}
}

func uint32sFromRPCParam(t *testing.T, value any) []uint32 {
	t.Helper()
	switch v := value.(type) {
	case []uint32:
		return v
	case []any:
		out := make([]uint32, 0, len(v))
		for _, item := range v {
			n, ok := item.(float64)
			if !ok {
				t.Fatalf("uid item = %#v (%T), want JSON number", item, item)
			}
			out = append(out, uint32(n))
		}
		return out
	default:
		t.Fatalf("uids = %#v (%T), want []uint32 or JSON array", value, value)
		return nil
	}
}

// The sidecar splits branch-compound keys, filters the thread to the subject
// branch itself, and shapes final bridge-ready message JSON; the bridge passes
// the key (and the exact thread id, for the messages' id fields) through and
// returns the response untouched.
func TestThreadReadPassesBranchKeyThrough(t *testing.T) {
	threadID := formatImapThreadID("acc", "INBOX", "k1#Todo")
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{
		"messages": []any{
			map[string]any{
				"id":        threadID + "#11",
				"thread_id": threadID,
				"subject":   "Re: Todo",
				"from_name": "Ann",
			},
		},
	}})

	out, err := app.threadRead(map[string]any{"thread_id": threadID, "limit": float64(20), "before_cursor": "c1"})
	if err != nil {
		t.Fatal(err)
	}
	if len(writer.calls) != 1 {
		t.Fatalf("sidecar calls = %d, want 1", len(writer.calls))
	}
	assertCall(t, writer.calls[0], "messages.thread", map[string]any{
		"account":       "acc",
		"folder":        "INBOX",
		"thread_key":    "k1#Todo",
		"thread_id":     threadID,
		"limit":         float64(20),
		"before_cursor": "c1",
	})

	messages, _ := out.(map[string]any)["messages"].([]any)
	if len(messages) != 1 {
		t.Fatalf("messages = %#v, want the sidecar's rows unmodified", messages)
	}
	first, _ := messages[0].(map[string]any)
	if first["id"] != threadID+"#11" || first["from_name"] != "Ann" {
		t.Fatalf("passed-through message = %#v", first)
	}
}

func TestMailMovePassesBranchKeyThrough(t *testing.T) {
	threadID := formatImapThreadID("acc", "INBOX", "k1#Todo")
	app, writer := newMailHandlerTestApp(t,
		sidecarResponsePlan{Result: map[string]any{"ok": true, "moved": float64(2)}},
	)

	out, err := app.mailMove(map[string]any{"thread_id": threadID, "target_folder_id": "Archive"})
	if err != nil {
		t.Fatal(err)
	}
	if got := out.(map[string]any)["moved"]; got != float64(2) {
		t.Fatalf("moved = %v, want 2", got)
	}
	if len(writer.calls) != 1 {
		t.Fatalf("sidecar calls = %#v, want one move", writer.calls)
	}
	assertCall(t, writer.calls[0], "messages.move", map[string]any{
		"account":       "acc",
		"folder":        "INBOX",
		"target_folder": "Archive",
		"thread_key":    "k1#Todo",
	})
}

func TestMailDeleteWithExplicitMessageIDsUsesUIDsAndMovedLocation(t *testing.T) {
	threadID := formatImapThreadID("acc", "INBOX", "k1#Todo")
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{
		"ok":      true,
		"deleted": float64(2),
		"trash":   "Trash",
	}})

	out, err := app.mailDelete(map[string]any{
		"thread_id":   threadID,
		"message_ids": []any{threadID + "#11", "12"},
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(writer.calls) != 1 {
		t.Fatalf("sidecar calls = %#v, want one delete", writer.calls)
	}
	if writer.calls[0].Method != "messages.delete" {
		t.Fatalf("method = %q, want messages.delete", writer.calls[0].Method)
	}
	if writer.calls[0].Params["account"] != "acc" || writer.calls[0].Params["folder"] != "INBOX" {
		t.Fatalf("delete params = %#v", writer.calls[0].Params)
	}
	if got, want := uint32sFromRPCParam(t, writer.calls[0].Params["uids"]), []uint32{11, 12}; !reflect.DeepEqual(got, want) {
		t.Fatalf("delete uids = %#v, want %#v", got, want)
	}

	obj := out.(map[string]any)
	if obj["thread_id"] != formatImapThreadID("acc", "Trash", "k1#Todo") {
		t.Fatalf("thread_id = %v, want trash thread location", obj["thread_id"])
	}
}

func TestMarkReadPassesBranchKeyThrough(t *testing.T) {
	threadID := formatImapThreadID("acc", "INBOX", "k1#Todo")
	app, writer := newMailHandlerTestApp(t,
		sidecarResponsePlan{Result: map[string]any{"ok": true}},
	)

	if _, err := app.markRead(map[string]any{"thread_id": threadID}); err != nil {
		t.Fatal(err)
	}
	if len(writer.calls) != 1 {
		t.Fatalf("sidecar calls = %#v, want one markRead", writer.calls)
	}
	assertCall(t, writer.calls[0], "messages.markRead", map[string]any{
		"account":    "acc",
		"folder":     "INBOX",
		"thread_key": "k1#Todo",
		"seen":       true,
	})
}

func TestMarkStarredPassesBranchKeyThrough(t *testing.T) {
	threadID := formatImapThreadID("acc", "INBOX", "k1#Todo")
	app, writer := newMailHandlerTestApp(t,
		sidecarResponsePlan{Result: map[string]any{"ok": true}},
	)

	if _, err := app.markStarred(map[string]any{"thread_id": threadID}); err != nil {
		t.Fatal(err)
	}
	if len(writer.calls) != 1 {
		t.Fatalf("sidecar calls = %#v, want one markStarred", writer.calls)
	}
	assertCall(t, writer.calls[0], "messages.markStarred", map[string]any{
		"account":    "acc",
		"folder":     "INBOX",
		"thread_key": "k1#Todo",
		"starred":    true,
	})
}

func TestMailSendMapsPayloadToSidecarSend(t *testing.T) {
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{"ok": true}})

	out, err := app.mailSend(map[string]any{
		"account_id":  "acc",
		"to":          "bob@example.com",
		"cc":          "copy@example.com",
		"bcc":         "blind@example.com",
		"subject":     "Hello",
		"body":        "plain",
		"html":        "<p>plain</p>",
		"in_reply_to": "<parent@example.com>",
		"references":  "<root@example.com>",
		"reply_to":    "reply@example.com",
		"from":        "Alice <alice@example.com>",
		"message_id":  "<message@example.com>",
		"attachments": []any{
			map[string]any{"filename": "note.txt", "mime": "text/plain", "data": "bm90ZQ=="},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !out.(map[string]any)["ok"].(bool) {
		t.Fatalf("send result = %#v", out)
	}
	if len(writer.calls) != 1 {
		t.Fatalf("sidecar calls = %#v, want send", writer.calls)
	}
	params := writer.calls[0].Params
	if writer.calls[0].Method != "send" {
		t.Fatalf("method = %q, want send", writer.calls[0].Method)
	}
	if params["account"] != "acc" || params["to"] != "bob@example.com" || params["message_id"] != "<message@example.com>" {
		t.Fatalf("send params = %#v", params)
	}
	attachments, ok := params["attachments"].([]any)
	if !ok || len(attachments) != 1 {
		t.Fatalf("attachments = %#v, want one attachment", params["attachments"])
	}
}
