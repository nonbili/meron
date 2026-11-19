//go:build integration

package main

// CalDAV calendars end to end: sidecar → caldav.* → Stalwart, both ways.

import (
	"fmt"
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
