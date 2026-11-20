//go:build integration

package main

import (
	"fmt"
	"net/http"
	"net/url"
	"os"
	"strings"
	"testing"
	"time"
)

// CARD_DAV_TEST_URL points at a Stalwart fixture such as ~/repo/mservers.
// Without it the test provisions an isolated Stalwart container.
func TestCardDAVSync(t *testing.T) {
	base := os.Getenv("CARD_DAV_TEST_URL")
	user, password := os.Getenv("CARD_DAV_TEST_USER"), os.Getenv("CARD_DAV_TEST_PASSWORD")
	if base == "" {
		base, user, password = startStalwart(t).url, calendarUser, calendarPassword
	}
	if user == "" || password == "" {
		t.Fatal("set CARD_DAV_TEST_USER and CARD_DAV_TEST_PASSWORD for the external fixture")
	}
	sidecar, _ := startSidecar(t)
	app := &App{sidecar: sidecar}
	connect := map[string]any{"server_url": base, "username": user, "password": "wrong-password"}
	if _, err := app.Invoke("carddav.add", connect); err == nil {
		t.Fatal("bad password was accepted")
	}
	if list := callMap(t, sidecar, "carddav.accounts", nil)["accounts"].([]any); len(list) != 0 {
		t.Fatal("bad password left an account behind")
	}
	connect["password"] = password
	added, err := app.Invoke("carddav.add", connect)
	if err != nil {
		t.Fatal(err)
	}
	id := added.(map[string]any)["account_id"].(string)
	if again := callMap(t, sidecar, "carddav.add", connect)["account_id"]; again != id {
		t.Fatal("reconnecting duplicated the account")
	}
	account := callMap(t, sidecar, "carddav.accounts", nil)["accounts"].([]any)[0].(map[string]any)
	if _, leaked := account["password"]; leaked {
		t.Fatal("password returned to the webview")
	}
	books := account["books"].([]any)
	if len(books) != 1 {
		t.Fatalf("expected default book, got %v", books)
	}
	book := books[0].(map[string]any)["id"].(string)
	nonce := fmt.Sprintf("meron-test-%d", time.Now().UnixNano())
	address := nonce + "@example.test"
	path := strings.TrimRight(base, "/") + "/dav/card/" + url.PathEscape(user) + "/default/" + nonce + ".vcf"
	request := func(method, body string) (int, error) {
		req, err := http.NewRequest(method, path, strings.NewReader(body))
		if err != nil {
			return 0, err
		}
		req.SetBasicAuth(user, password)
		req.Header.Set("Content-Type", "text/vcard; charset=utf-8")
		res, err := (&http.Client{Timeout: 10 * time.Second}).Do(req)
		if err != nil {
			return 0, err
		}
		res.Body.Close()
		return res.StatusCode, nil
	}
	t.Cleanup(func() { _, _ = request("DELETE", "") })
	put := func(name string) {
		vcard := "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:" + nonce + "\r\nFN:" + name + "\r\nN:Fixture;Cora;;;\r\nEMAIL;TYPE=INTERNET:" + address + "\r\nEMAIL;TYPE=WORK:work-" + address + "\r\nEND:VCARD\r\n"
		if status, err := request("PUT", vcard); err != nil || status < 200 || status >= 300 {
			t.Fatalf("seed CardDAV fixture: HTTP %d, %v", status, err)
		}
	}
	sync := func() float64 {
		return callMap(t, sidecar, "carddav.sync", map[string]any{"account_id": id})["changed"].(float64)
	}
	assertContacts := func(want string) {
		t.Helper()
		for _, directory := range []bool{false, true} {
			res, err := app.Invoke("mail.suggestContacts", map[string]any{"account": "", "query": nonce, "directory": directory})
			if err != nil {
				t.Fatal(err)
			}
			contacts := res.(map[string]any)["contacts"].([]any)
			if want == "" && len(contacts) == 0 {
				continue
			}
			if want == "" || len(contacts) != 2 {
				t.Fatalf("directory=%t: expected %q, got %v", directory, want, contacts)
			}
			for _, contact := range contacts {
				if contact.(map[string]any)["name"] != want {
					t.Fatalf("contact did not sync: %v", contact)
				}
			}
		}
	}
	put("Cora Fixture")
	sync()
	assertContacts("Cora Fixture")
	if sync() != 0 {
		t.Fatal("unchanged vCard was fetched again")
	}
	put("Cora Updated")
	sync()
	assertContacts("Cora Updated")
	setBook := func(enabled bool) {
		callMap(t, sidecar, "carddav.setBookEnabled", map[string]any{"book_id": book, "enabled": enabled})
	}
	setBook(false)
	assertContacts("")
	setBook(true)
	assertContacts("Cora Updated")
	if status, err := request("DELETE", ""); err != nil || status < 200 || status >= 300 {
		t.Fatalf("remove fixture: HTTP %d, %v", status, err)
	}
	sync()
	assertContacts("")
	put("Cora Again")
	sync()
	assertContacts("Cora Again")
	find := func(query string) map[string]any {
		t.Helper()
		matches := callMap(t, sidecar, "carddav.contacts", map[string]any{"book_id": book, "query": query})["contacts"].([]any)
		if len(matches) != 1 {
			t.Fatalf("expected one matching vCard, got %d", len(matches))
		}
		return matches[0].(map[string]any)
	}
	change := func(card map[string]any) map[string]any {
		return map[string]any{"book_id": book, "contact_id": card["id"], "version": card["version"]}
	}
	write := func(method string, params map[string]any) {
		t.Helper()
		if result := callMap(t, sidecar, method, params); result["sync_error"] != nil {
			t.Fatalf("%s saved but did not sync: %v", method, result)
		}
	}
	stale := change(find(nonce))
	stale["name"], stale["emails"] = "Cora Stale", []string{address}
	put("Cora Server Update")
	if _, err := sidecar.Call("carddav.updateContact", stale); err == nil {
		t.Fatal("stale contact update was accepted")
	}
	sync()
	assertContacts("Cora Server Update")
	edit := change(find(nonce))
	edit["name"], edit["emails"] = "Cora Edited in Meron", []string{address, "work-" + address}
	write("carddav.updateContact", edit)
	assertContacts("Cora Edited in Meron")
	write("carddav.deleteContact", change(find(nonce)))
	assertContacts("")
	if status, err := request("GET", ""); err != nil || status != 404 {
		t.Fatalf("contact was not deleted on the server: HTTP %d, %v", status, err)
	}
	write("carddav.createContact", map[string]any{"book_id": book, "name": "Cora Created in Meron", "emails": []string{address, "work-" + address}})
	assertContacts("Cora Created in Meron")
	created := find(nonce)
	t.Cleanup(func() {
		remote := created["id"].(string)
		req, err := http.NewRequest("DELETE", remote, nil)
		if err == nil && strings.HasPrefix(remote, strings.TrimRight(base, "/")+"/") {
			req.SetBasicAuth(user, password)
			if res, err := (&http.Client{Timeout: 10 * time.Second}).Do(req); err == nil {
				res.Body.Close()
			}
		}
	})
	write("carddav.deleteContact", change(created))
	assertContacts("")
	put("Cora Again")
	sync()
	callMap(t, sidecar, "carddav.remove", map[string]any{"account_id": id})
	assertContacts("")
	if status, err := request("GET", ""); err != nil || status != 200 {
		t.Fatalf("disconnect changed server data: HTTP %d, %v", status, err)
	}
}
