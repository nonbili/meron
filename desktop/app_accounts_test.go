package main

import "testing"

// The UI reads each account's signature override back from account.list, so a
// field missing from the Account struct reads as "setting lost on restart".
func TestAccountListKeepsSignatureOverride(t *testing.T) {
	app, _ := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{
		"accounts": []any{map[string]any{
			"id":        "acc",
			"email":     "user@example.com",
			"signature": map[string]any{"mode": "custom", "html": "<p>Ping</p>"},
		}},
	}})

	res, err := app.accountList()
	if err != nil {
		t.Fatal(err)
	}
	accounts := res.(map[string]any)["accounts"].([]Account)
	if len(accounts) != 1 {
		t.Fatalf("accounts = %#v, want one", accounts)
	}
	signature, _ := accounts[0].Signature.(map[string]any)
	if signature["mode"] != "custom" || signature["html"] != "<p>Ping</p>" {
		t.Fatalf("signature = %#v, want the stored override", accounts[0].Signature)
	}
}

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

// An account whose server certificate the user accepted only stays reachable if
// the pin reaches the core with the connection settings.
func TestAccountAddPasswordForwardsAcceptedCertificate(t *testing.T) {
	app, writer := newMailHandlerTestApp(t,
		sidecarResponsePlan{Result: map[string]any{"ok": true}}, // account.connect
		sidecarResponsePlan{Result: map[string]any{"ok": true}}, // watch.start
	)

	if _, err := app.accountAddPassword(map[string]any{
		"email":     "user@example.com",
		"imap_host": "127.0.0.1",
		"imap_port": 1143,
		"smtp_host": "127.0.0.1",
		"smtp_port": 1025,
		"username":  "user@example.com",
		"password":  "bridge-password",
		"cert_pin":  "  AB12  ",
	}); err != nil {
		t.Fatal(err)
	}

	if writer.calls[0].Method != "account.connect" {
		t.Fatalf("first call = %q, want account.connect", writer.calls[0].Method)
	}
	if got := writer.calls[0].Params["cert_pin"]; got != "AB12" {
		t.Fatalf("cert_pin = %#v, want the trimmed pin", got)
	}
}

// Accounts on servers with a normal certificate must not carry a pin at all —
// an empty one would be persisted and read back as "pinned to nothing".
func TestAccountAddPasswordOmitsEmptyCertificatePin(t *testing.T) {
	app, writer := newMailHandlerTestApp(t,
		sidecarResponsePlan{Result: map[string]any{"ok": true}},
		sidecarResponsePlan{Result: map[string]any{"ok": true}},
	)

	if _, err := app.accountAddPassword(map[string]any{
		"email":     "user@example.com",
		"imap_host": "imap.example.com",
		"smtp_host": "smtp.example.com",
		"username":  "user@example.com",
		"password":  "secret",
		"cert_pin":  "",
	}); err != nil {
		t.Fatal(err)
	}

	if _, ok := writer.calls[0].Params["cert_pin"]; ok {
		t.Fatalf("params = %#v, want no cert_pin key", writer.calls[0].Params)
	}
}

// Accepting a certificate for an existing account must pin only the server the
// user was shown; the other server keeps whatever it already trusted.
func TestAccountSetCertPinForwardsOnlyTheAcceptedServer(t *testing.T) {
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{"ok": true}})

	if _, err := app.accountSetCertPin(map[string]any{
		"id":            "acc",
		"smtp_cert_pin": " 6f69d6a7 ",
	}); err != nil {
		t.Fatal(err)
	}

	if got := writer.calls[0].Params["smtp_cert_pin"]; got != "6f69d6a7" {
		t.Fatalf("smtp_cert_pin = %#v, want the trimmed pin", got)
	}
	if _, ok := writer.calls[0].Params["cert_pin"]; ok {
		t.Fatalf("params = %#v, want no cert_pin key", writer.calls[0].Params)
	}
}

func TestAccountSetCertPinRequiresAnAccount(t *testing.T) {
	app, _ := newMailHandlerTestApp(t)

	if _, err := app.accountSetCertPin(map[string]any{"cert_pin": "abc"}); err == nil {
		t.Fatal("want an error for a pin with no account")
	}
}

// The probe must take the same route as the connection that failed: an account
// on its own proxy is otherwise reached over a different path, or not at all.
func TestAccountProbeCertForwardsTheAccountProxy(t *testing.T) {
	app, writer := newMailHandlerTestApp(t, sidecarResponsePlan{Result: map[string]any{"certificate": map[string]any{}}})

	if _, err := app.accountProbeCert(map[string]any{
		"host":     "127.0.0.1",
		"port":     1025,
		"protocol": "smtp",
		"starttls": true,
		"proxy":    map[string]any{"mode": "socks5", "host": "127.0.0.1", "port": 9050},
	}); err != nil {
		t.Fatal(err)
	}

	params := writer.calls[0].Params
	proxy, _ := params["proxy"].(map[string]any)
	if proxy["mode"] != "socks5" || proxy["port"] != float64(9050) {
		t.Fatalf("proxy = %#v, want the account's own proxy", params["proxy"])
	}
	if params["protocol"] != "smtp" || params["starttls"] != true {
		t.Fatalf("params = %#v, want the submission server with STARTTLS", params)
	}
}
