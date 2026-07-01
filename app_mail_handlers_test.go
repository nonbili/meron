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

func TestThreadReadBranchedThreadRequestsRealThreadAndFiltersMessages(t *testing.T) {
	threadID := formatImapThreadID("acc", "INBOX", "k1#"+threadGroupingSubject("Todo"))
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{
		"messages": []any{
			map[string]any{
				"uid": float64(11),
				"message": map[string]any{
					"subject":   "Re: Todo",
					"from_name": "Ann",
				},
			},
			map[string]any{
				"uid": float64(12),
				"message": map[string]any{
					"subject": "Other",
				},
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
		"thread_key":    "k1",
		"limit":         float64(20),
		"before_cursor": "c1",
	})

	messages := out.(map[string]any)["messages"].([]Message)
	if len(messages) != 1 {
		t.Fatalf("messages = %#v, want only matching subject branch", messages)
	}
	if messages[0].ID != threadID+"#11" || messages[0].FromName != "Ann" {
		t.Fatalf("filtered message = %#v", messages[0])
	}
}

func TestMailMoveBranchedThreadMovesOnlyMatchingSubjectUIDsByFolder(t *testing.T) {
	threadID := formatImapThreadID("acc", "INBOX", "k1#"+threadGroupingSubject("Todo"))
	app, writer := newMailHandlerTestApp(t,
		sidecarResponsePlan{Result: map[string]any{
			"headers": []any{
				map[string]any{"uid": float64(11), "folder": "INBOX", "subject": "Todo"},
				map[string]any{"uid": float64(12), "folder": "Sent", "subject": "Re: Todo"},
				map[string]any{"uid": float64(13), "folder": "Archive", "subject": "Todo"},
				map[string]any{"uid": float64(14), "folder": "INBOX", "subject": "Other"},
			},
		}},
		sidecarResponsePlan{Result: map[string]any{"ok": true, "moved": float64(1)}},
		sidecarResponsePlan{Result: map[string]any{"ok": true, "moved": float64(1)}},
	)

	out, err := app.mailMove(map[string]any{"thread_id": threadID, "target_folder_id": "Archive"})
	if err != nil {
		t.Fatal(err)
	}
	if got := out.(map[string]any)["moved"]; got != 2 {
		t.Fatalf("moved = %d, want 2", got)
	}
	if len(writer.calls) != 3 {
		t.Fatalf("sidecar calls = %#v, want threadHeaders plus two moves", writer.calls)
	}
	assertCall(t, writer.calls[0], "messages.threadHeaders", map[string]any{
		"account":    "acc",
		"folder":     "INBOX",
		"thread_key": "k1",
	})

	moveCalls := map[string][]uint32{}
	for _, call := range writer.calls[1:] {
		if call.Method != "messages.move" {
			t.Fatalf("method = %q, want messages.move", call.Method)
		}
		if call.Params["account"] != "acc" || call.Params["target_folder"] != "Archive" {
			t.Fatalf("move params = %#v", call.Params)
		}
		moveCalls[call.Params["folder"].(string)] = uint32sFromRPCParam(t, call.Params["uids"])
	}
	want := map[string][]uint32{"INBOX": {11}, "Sent": {12}}
	if !reflect.DeepEqual(moveCalls, want) {
		t.Fatalf("move calls = %#v, want %#v", moveCalls, want)
	}
}

func TestMailDeleteWithExplicitMessageIDsUsesUIDsAndMovedLocation(t *testing.T) {
	threadID := formatImapThreadID("acc", "INBOX", "k1#"+threadGroupingSubject("Todo"))
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
	if obj["thread_id"] != formatImapThreadID("acc", "Trash", "k1#"+threadGroupingSubject("Todo")) {
		t.Fatalf("thread_id = %v, want trash thread location", obj["thread_id"])
	}
}

func TestMarkReadBranchedThreadOnlyMarksUnreadMatchingSubject(t *testing.T) {
	threadID := formatImapThreadID("acc", "INBOX", "k1#"+threadGroupingSubject("Todo"))
	app, writer := newMailHandlerTestApp(t,
		sidecarResponsePlan{Result: map[string]any{
			"messages": []any{
				map[string]any{
					"uid":  float64(11),
					"seen": false,
					"message": map[string]any{
						"subject": "Todo",
					},
				},
				map[string]any{
					"uid":  float64(12),
					"seen": true,
					"message": map[string]any{
						"subject": "Todo",
					},
				},
				map[string]any{
					"uid":  float64(13),
					"seen": false,
					"message": map[string]any{
						"subject": "Other",
					},
				},
			},
		}},
		sidecarResponsePlan{Result: map[string]any{"ok": true}},
	)

	if _, err := app.markRead(map[string]any{"thread_id": threadID}); err != nil {
		t.Fatal(err)
	}
	if len(writer.calls) != 2 {
		t.Fatalf("sidecar calls = %#v, want read then markRead", writer.calls)
	}
	assertCall(t, writer.calls[0], "messages.thread", map[string]any{
		"account":    "acc",
		"folder":     "INBOX",
		"thread_key": "k1",
	})
	if writer.calls[1].Method != "messages.markRead" {
		t.Fatalf("method = %q, want messages.markRead", writer.calls[1].Method)
	}
	if writer.calls[1].Params["account"] != "acc" || writer.calls[1].Params["folder"] != "INBOX" || writer.calls[1].Params["seen"] != true {
		t.Fatalf("markRead params = %#v", writer.calls[1].Params)
	}
	if got, want := uint32sFromRPCParam(t, writer.calls[1].Params["uids"]), []uint32{11}; !reflect.DeepEqual(got, want) {
		t.Fatalf("markRead uids = %#v, want %#v", got, want)
	}
}

func TestMarkStarredBranchedThreadGroupsUnstarredMatchingMessagesByFolder(t *testing.T) {
	threadID := formatImapThreadID("acc", "INBOX", "k1#"+threadGroupingSubject("Todo"))
	app, writer := newMailHandlerTestApp(t,
		sidecarResponsePlan{Result: map[string]any{
			"messages": []any{
				map[string]any{
					"uid":     float64(11),
					"folder":  "INBOX",
					"starred": false,
					"message": map[string]any{"subject": "Todo"},
				},
				map[string]any{
					"uid":     float64(12),
					"folder":  "Sent",
					"starred": false,
					"message": map[string]any{"subject": "Re: Todo"},
				},
				map[string]any{
					"uid":     float64(13),
					"folder":  "Sent",
					"starred": true,
					"message": map[string]any{"subject": "Todo"},
				},
				map[string]any{
					"uid":     float64(14),
					"folder":  "INBOX",
					"starred": false,
					"message": map[string]any{"subject": "Other"},
				},
			},
		}},
		sidecarResponsePlan{Result: map[string]any{"ok": true}},
		sidecarResponsePlan{Result: map[string]any{"ok": true}},
	)

	if _, err := app.markStarred(map[string]any{"thread_id": threadID}); err != nil {
		t.Fatal(err)
	}
	if len(writer.calls) != 3 {
		t.Fatalf("sidecar calls = %#v, want read then two markStarred calls", writer.calls)
	}
	assertCall(t, writer.calls[0], "messages.thread", map[string]any{
		"account":    "acc",
		"folder":     "INBOX",
		"thread_key": "k1",
	})
	markCalls := map[string][]uint32{}
	for _, call := range writer.calls[1:] {
		if call.Method != "messages.markStarred" {
			t.Fatalf("method = %q, want messages.markStarred", call.Method)
		}
		if call.Params["account"] != "acc" || call.Params["starred"] != true {
			t.Fatalf("markStarred params = %#v", call.Params)
		}
		markCalls[call.Params["folder"].(string)] = uint32sFromRPCParam(t, call.Params["uids"])
	}
	want := map[string][]uint32{"INBOX": {11}, "Sent": {12}}
	if !reflect.DeepEqual(markCalls, want) {
		t.Fatalf("markStarred calls = %#v, want %#v", markCalls, want)
	}
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
