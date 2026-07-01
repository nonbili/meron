package main

import "testing"

func TestAccountSetSaveSentCopyMapsNullableValue(t *testing.T) {
	app, writer := newMailHandlerTestApp(t,
		sidecarResponsePlan{Result: map[string]any{"ok": true}},
		sidecarResponsePlan{Result: map[string]any{"ok": true}},
	)

	if _, err := app.accountSetSaveSentCopy(map[string]any{"id": "acc", "value": true}); err != nil {
		t.Fatal(err)
	}
	if _, err := app.accountSetSaveSentCopy(map[string]any{"id": "acc", "value": nil}); err != nil {
		t.Fatal(err)
	}

	if len(writer.calls) != 2 {
		t.Fatalf("sidecar calls = %#v, want two calls", writer.calls)
	}
	assertCall(t, writer.calls[0], "account.setSaveSentCopy", map[string]any{"account": "acc", "value": true})
	assertCall(t, writer.calls[1], "account.setSaveSentCopy", map[string]any{"account": "acc", "value": nil})
}
