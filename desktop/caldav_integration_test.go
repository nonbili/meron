//go:build integration

package main

// CalDAV calendars end to end: sidecar → caldav.* → Stalwart, both ways.

import (
	"fmt"
	"os"
	"strings"
	"testing"
	"time"
)

func TestCalDAVSync(t *testing.T) {
	server := startStalwart(t)
	sidecar, events := startSidecar(t)
	nonce := fmt.Sprint(time.Now().UnixNano())

	// A wrong password is refused up front and nothing is kept.
	if _, err := sidecar.Call("caldav.add", map[string]any{
		"server_url": server.url,
		"username":   calendarUser,
		"password":   "wrong-password",
	}); err == nil {
		t.Fatal("caldav.add accepted a wrong password")
	}
	if accounts := callMap(t, sidecar, "caldav.accounts", nil)["accounts"].([]any); len(accounts) != 0 {
		t.Fatalf("a refused account was kept: %v", accounts)
	}

	// An event already on the server.
	serverTitle := "Server event " + nonce
	serverPath := calendarPath() + "server-" + nonce + ".ics"
	status, body := server.dav(t, "PUT", serverPath, "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Test//EN\r\nBEGIN:VEVENT\r\n"+
		"UID:server-"+nonce+"\r\nDTSTAMP:20261119T080000Z\r\nDTSTART:20261121T090000Z\r\nDTEND:20261121T100000Z\r\n"+
		"SUMMARY:"+serverTitle+"\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
		map[string]string{"Content-Type": "text/calendar"})
	if status != 201 {
		t.Fatalf("seed event: %d %s", status, body)
	}

	// Discovery from the bare server URL finds the default calendar.
	added := callMap(t, sidecar, "caldav.add", map[string]any{
		"server_url": server.url,
		"username":   calendarUser,
		"password":   calendarPassword,
	})
	accountID, _ := added["account_id"].(string)
	if accountID == "" {
		t.Fatalf("caldav.add: %v", added)
	}
	calendars := callMap(t, sidecar, "caldav.calendars", nil)["calendars"].([]any)
	if len(calendars) == 0 {
		t.Fatal("no calendars after caldav.add")
	}
	calendarID := calendars[0].(map[string]any)["id"].(string)

	// Adding starts a sync of its own; this one waits for it and runs again.
	callMap(t, sidecar, "caldav.sync", map[string]any{"account_id": accountID})
	// 2026-11-21, the seeded event's day.
	dayEvents := func() []any {
		return callMap(t, sidecar, "calendar.events", map[string]any{
			"from": 1795219200, "to": 1795305600,
		})["events"].([]any)
	}
	var serverEvent map[string]any
	for _, item := range dayEvents() {
		if event := item.(map[string]any); event["title"] == serverTitle {
			serverEvent = event
		}
	}
	if serverEvent == nil || serverEvent["calendar"] != calendarID {
		t.Fatalf("server event not in the calendar view: %v", dayEvents())
	}

	// An event made in Meron reaches the server.
	localTitle := "Meron event " + nonce
	callMap(t, sidecar, "calendar.create", map[string]any{
		"title": localTitle, "start": 1795251600 + 7200, "end": 1795251600 + 10800,
		"calendar": calendarID,
	})
	// The edit's own background sync may send it first; either way it is
	// there once this returns.
	callMap(t, sidecar, "caldav.sync", nil)
	if !strings.Contains(server.serverCalendarData(t), "SUMMARY:"+localTitle) {
		t.Fatal("created event not on the server")
	}

	// An edit in Meron reaches the server; one on the server comes back.
	callMap(t, sidecar, "calendar.update", map[string]any{
		"event_id": serverEvent["id"], "title": serverTitle + " edited",
		"start": serverEvent["start"], "end": serverEvent["end"],
	})
	callMap(t, sidecar, "caldav.sync", nil)
	if !strings.Contains(server.serverCalendarData(t), "SUMMARY:"+serverTitle+" edited") {
		t.Fatal("edit did not reach the server")
	}
	before := events.count(func(e sidecarEvent) bool { return e.name == "calendar.changed" })
	if status, _ := server.dav(t, "DELETE", serverPath, "", nil); status != 204 {
		t.Fatalf("delete on server: %d", status)
	}
	callMap(t, sidecar, "caldav.sync", nil)
	for _, item := range dayEvents() {
		if strings.HasPrefix(item.(map[string]any)["title"].(string), serverTitle) {
			t.Fatal("event deleted on the server is still shown")
		}
	}
	if events.count(func(e sidecarEvent) bool { return e.name == "calendar.changed" }) == before {
		t.Fatal("no calendar.changed after a sync that removed an event")
	}

	// A second calendar on the server shows up, and an event moves into it:
	// gone from the first, in the second, its UID unchanged.
	homePath := strings.Replace(calendarPath(), "/default/", "/home-"+nonce+"/", 1)
	if status, body := server.dav(t, "MKCALENDAR", homePath, "", nil); status != 201 {
		t.Fatalf("MKCALENDAR: %d %s", status, body)
	}
	callMap(t, sidecar, "caldav.sync", nil)
	var homeID string
	for _, c := range callMap(t, sidecar, "caldav.calendars", nil)["calendars"].([]any) {
		if id := c.(map[string]any)["id"].(string); id != calendarID {
			homeID = id
		}
	}
	if homeID == "" {
		t.Fatal("second calendar not listed after sync")
	}
	var localEvent map[string]any
	for _, item := range dayEvents() {
		if event := item.(map[string]any); event["title"] == localTitle {
			localEvent = event
		}
	}
	callMap(t, sidecar, "calendar.update", map[string]any{
		"event_id": localEvent["id"], "title": localTitle,
		"start": localEvent["start"], "end": localEvent["end"], "calendar": homeID,
	})
	callMap(t, sidecar, "caldav.sync", nil)
	if strings.Contains(server.serverCalendarData(t), localTitle) {
		t.Fatal("moved event is still in the calendar it left")
	}
	if !strings.Contains(server.calendarDataAt(t, homePath), "SUMMARY:"+localTitle) {
		t.Fatal("moved event not in the second calendar")
	}

	// Removing the account forgets its events here and leaves the server alone.
	callMap(t, sidecar, "caldav.remove", map[string]any{"account_id": accountID})
	for _, item := range dayEvents() {
		if item.(map[string]any)["calendar"] != "" {
			t.Fatalf("synced event left after removing the account: %v", item)
		}
	}
	if !strings.Contains(server.calendarDataAt(t, homePath), localTitle) {
		t.Fatal("removing the account deleted server data")
	}
}

// MERON_TEST_MSERVERS=1 uses the fictional account on the local mservers
// Stalwart instance. The test owns a unique collection on either server.
func TestCalDAVTasksSync(t *testing.T) {
	var server *calendarServer
	if os.Getenv("MERON_TEST_MSERVERS") == "1" {
		server = &calendarServer{url: "http://127.0.0.1:4080", username: "user1@mail.localhost", password: "user1password"}
	} else {
		server = startStalwart(t)
	}
	sidecar, events := startSidecar(t)
	nonce := fmt.Sprint(time.Now().UnixNano())
	name := "Meron task integration " + nonce
	path := "/dav/cal/" + strings.ReplaceAll(server.username, "@", "%40") + "/tasks-" + nonce + "/"
	mkcalendar := `<c:mkcalendar xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:set><d:prop><d:displayname>` + name + `</d:displayname><c:supported-calendar-component-set><c:comp name="VTODO"/></c:supported-calendar-component-set></d:prop></d:set></c:mkcalendar>`
	if status, body := server.dav(t, "MKCALENDAR", path, mkcalendar, map[string]string{"Content-Type": "application/xml"}); status != 201 {
		t.Fatalf("make task collection: %d %s", status, body)
	}
	t.Cleanup(func() {
		if status, body := server.dav(t, "DELETE", path, "", nil); status != 204 && status != 404 {
			t.Errorf("clean up task collection: %d %s", status, body)
		}
	})
	icsHeaders := map[string]string{"Content-Type": "text/calendar"}
	remoteBody := "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Test//EN\r\nBEGIN:VTODO\r\n" +
		"UID:remote-" + nonce + "\r\nDTSTAMP:20261119T080000Z\r\nSUMMARY:From server\r\n" +
		"DUE;VALUE=DATE:20261121\r\nDESCRIPTION:First line\\nSecond line\r\nPRIORITY:5\r\nEND:VTODO\r\nEND:VCALENDAR\r\n"
	if status, body := server.dav(t, "PUT", path+"remote.ics", remoteBody, icsHeaders); status != 201 {
		t.Fatalf("seed task: %d %s", status, body)
	}
	added := callMap(t, sidecar, "caldav.add", map[string]any{
		"server_url": server.url, "username": server.username, "password": server.password,
	})
	account := added["account_id"].(string)
	var calendar string
	for _, raw := range callMap(t, sidecar, "caldav.accounts", nil)["accounts"].([]any)[0].(map[string]any)["calendars"].([]any) {
		collection := raw.(map[string]any)
		if collection["name"] == name {
			if collection["tasks"] != true || collection["events"] != false {
				t.Fatalf("wrong capabilities: %v", collection)
			}
			calendar = collection["id"].(string)
		}
	}
	if calendar == "" {
		t.Fatal("task collection not discovered")
	}
	list := func() string {
		for _, raw := range callMap(t, sidecar, "tasks.lists", nil)["lists"].([]any) {
			item := raw.(map[string]any)
			if item["title"] == name {
				return item["id"].(string)
			}
		}
		return ""
	}
	if list() != "" {
		t.Fatal("task collection subscribed without opting in")
	}
	params := map[string]any{"calendar_id": calendar, "enabled": true}
	callMap(t, sidecar, "caldav.setTasksEnabled", params)
	sync := func() {
		t.Helper()
		result := callMap(t, sidecar, "caldav.sync", map[string]any{"account_id": account})
		if result["error"] != "" {
			t.Fatalf("sync: %v", result)
		}
	}
	sync()
	listID := list()
	if listID == "" {
		t.Fatal("subscribed task list is missing")
	}
	find := func(title string) map[string]any {
		for _, raw := range callMap(t, sidecar, "tasks.items", map[string]any{
			"list_id": listID, "include_completed": true,
		})["tasks"].([]any) {
			if item := raw.(map[string]any); item["title"] == title {
				return item
			}
		}
		return nil
	}
	remote := find("From server")
	day := float64(time.Date(2026, 11, 21, 0, 0, 0, 0, time.Local).Unix())
	if remote["due_at"] != day || remote["due_has_time"] != false || remote["notes"] != "First line\nSecond line" {
		t.Fatalf("server task did not round trip: %v, want date %v", remote, day)
	}
	callMap(t, sidecar, "tasks.update", map[string]any{
		"task_id": remote["id"], "title": "Edited in Meron",
		"due_at": time.Date(2026, 11, 22, 0, 0, 0, 0, time.Local).Unix(), "due_has_time": false,
	})
	sync()
	status, contents := server.dav(t, "GET", path+"remote.ics", "", nil)
	if status != 200 || !strings.Contains(contents, "SUMMARY:Edited in Meron") || !strings.Contains(contents, "PRIORITY:5") || !strings.Contains(contents, "DUE;VALUE=DATE:20261122") {
		t.Fatalf("server edit lost content: %d %s", status, contents)
	}
	made := callMap(t, sidecar, "tasks.create", map[string]any{
		"list_id": listID, "title": "From Meron", "notes": "Two way", "due_at": 1795251600,
		"due_has_time": true, "thread_id": "private-local-thread",
	})["task"].(map[string]any)
	sync()
	href := path + made["id"].(string) + ".ics"
	status, contents = server.dav(t, "GET", href, "", nil)
	if status != 200 || !strings.Contains(contents, "DUE:20261121T090000Z") || !strings.Contains(contents, "SUMMARY:From Meron") || strings.Contains(contents, "private-local-thread") {
		t.Fatalf("local task was not uploaded correctly: %d %s", status, contents)
	}
	// An external client completes the task. Sync updates the same local row.
	done := strings.Replace(contents, "END:VTODO", "STATUS:COMPLETED\r\nCOMPLETED:20261121T100000Z\r\nEND:VTODO", 1)
	before := events.count(func(event sidecarEvent) bool { return event.name == "tasks.changed" })
	if status, body := server.dav(t, "PUT", href, done, icsHeaders); status != 204 {
		t.Fatalf("complete on server: %d %s", status, body)
	}
	sync()
	completed := find("From Meron")
	if completed["id"] != made["id"] || completed["done"] != true || completed["thread_id"] != "private-local-thread" || completed["due_has_time"] != true {
		t.Fatalf("external completion did not update the same task: %v", completed)
	}
	if events.count(func(event sidecarEvent) bool { return event.name == "tasks.changed" }) <= before {
		t.Fatal("no tasks.changed after external completion")
	}
	callMap(t, sidecar, "tasks.setDone", map[string]any{"task_id": made["id"], "done": false})
	sync()
	status, contents = server.dav(t, "GET", href, "", nil)
	if status != 200 || strings.Contains(contents, "COMPLETED:") || !strings.Contains(contents, "STATUS:NEEDS-ACTION") {
		t.Fatalf("reopening did not reach the server: %d %s", status, contents)
	}
	callMap(t, sidecar, "tasks.setDone", map[string]any{"task_id": made["id"], "done": true})
	sync()
	status, contents = server.dav(t, "GET", href, "", nil)
	if status != 200 || !strings.Contains(contents, "STATUS:COMPLETED") {
		t.Fatalf("completion did not reach the server: %d %s", status, contents)
	}
	removed := callMap(t, sidecar, "tasks.clearCompleted", map[string]any{"list_id": listID})
	sync()
	if status, _ := server.dav(t, "GET", href, "", nil); status != 404 {
		t.Fatalf("task still on server: %d", status)
	}
	callMap(t, sidecar, "tasks.restore", map[string]any{"restore": removed["restore"]})
	sync()
	if status, _ := server.dav(t, "GET", href, "", nil); status != 200 {
		t.Fatalf("restored task missing from server: %d", status)
	}
	if status, _ := server.dav(t, "DELETE", href, "", nil); status != 204 {
		t.Fatalf("delete from server: %d", status)
	}
	sync()
	if find("From Meron") != nil {
		t.Fatal("task deleted on server still visible")
	}
	linked := callMap(t, sidecar, "tasks.create", map[string]any{
		"list_id": listID, "title": "Linked across settings", "account": "mail-fixture",
		"thread_id": "thread-fixture", "message_id": "message-fixture",
	})["task"].(map[string]any)
	sync()
	params["enabled"] = false
	callMap(t, sidecar, "caldav.setTasksEnabled", params)
	if list() != "" {
		t.Fatal("unsubscribed list still visible")
	}
	if status, _ := server.dav(t, "GET", path+"remote.ics", "", nil); status != 200 {
		t.Fatalf("unsubscribing affected the server task: %d", status)
	}
	params["enabled"] = true
	callMap(t, sidecar, "caldav.setTasksEnabled", params)
	sync()
	if list() != listID || find("Edited in Meron") == nil {
		t.Fatal("subscribing again did not restore the list")
	}
	restored := find("Linked across settings")
	if restored["id"] != linked["id"] || restored["account"] != "mail-fixture" ||
		restored["thread_id"] != "thread-fixture" || restored["message_id"] != "message-fixture" {
		t.Fatalf("re-subscribing lost the local mail link: %v", restored)
	}

}
