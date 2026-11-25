//go:build integration

package main

import (
	"bufio"
	"fmt"
	"io"
	"net"
	"strconv"
	"strings"
	"testing"
	"time"
)

// (messages.recent triggers a background IMAP sync and serves the store cache).
func pollFolder(t *testing.T, sidecar *Sidecar, account, folder string, predicate func(map[string]any) bool) map[string]any {
	t.Helper()
	deadline := time.Now().Add(60 * time.Second)
	for time.Now().Before(deadline) {
		result := callMap(t, sidecar, "messages.recent", map[string]any{
			"account": account,
			"folder":  folder,
			"refresh": true,
			"limit":   50,
		})
		rows, _ := result["messages"].([]any)
		for _, row := range rows {
			message, ok := row.(map[string]any)
			if ok && predicate(message) {
				return message
			}
		}
		time.Sleep(500 * time.Millisecond)
	}
	t.Fatalf("message did not arrive in %s %s within deadline", account, folder)
	return nil
}

func pollInbox(t *testing.T, sidecar *Sidecar, account string, predicate func(map[string]any) bool) map[string]any {
	t.Helper()
	return pollFolder(t, sidecar, account, "INBOX", predicate)
}

func assertNoMessageInFolder(t *testing.T, sidecar *Sidecar, account, folder string, predicate func(map[string]any) bool) {
	t.Helper()
	deadline := time.Now().Add(10 * time.Second)
	for time.Now().Before(deadline) {
		result := callMap(t, sidecar, "messages.recent", map[string]any{
			"account": account,
			"folder":  folder,
			"refresh": true,
			"limit":   50,
		})
		rows, _ := result["messages"].([]any)
		found := false
		for _, row := range rows {
			message, ok := row.(map[string]any)
			if ok && predicate(message) {
				found = true
				break
			}
		}
		if !found {
			return
		}
		time.Sleep(500 * time.Millisecond)
	}
	t.Fatalf("message still present in %s %s after deadline", account, folder)
}

func str(message map[string]any, key string) string {
	value, _ := message[key].(string)
	return value
}

func num(message map[string]any, key string) uint32 {
	switch value := message[key].(type) {
	case float64:
		return uint32(value)
	case int:
		return uint32(value)
	case uint32:
		return value
	default:
		return 0
	}
}

func boolValue(message map[string]any, key string) bool {
	value, _ := message[key].(bool)
	return value
}

// imapClient is a bare-bones IMAP client standing in for another mail client
// (webmail, a phone) touching the mailbox behind meron's back — and for reading
// server truth directly. Assertions that only consult the sidecar cannot tell a
// write that reached the server from one that stopped at the local store.
type imapClient struct {
	t      *testing.T
	conn   net.Conn
	reader *bufio.Reader
	tag    int
}

func dialIMAP(t *testing.T, port int, user, password string) *imapClient {
	t.Helper()
	conn, err := net.DialTimeout("tcp", fmt.Sprintf("127.0.0.1:%d", port), 5*time.Second)
	if err != nil {
		t.Fatalf("dial imap: %v", err)
	}
	conn.SetDeadline(time.Now().Add(30 * time.Second))
	client := &imapClient{t: t, conn: conn, reader: bufio.NewReader(conn)}
	client.readUntil("* OK")
	client.do("LOGIN %q %q", user, password)
	return client
}

func (c *imapClient) close() {
	_, _ = io.WriteString(c.conn, "zz LOGOUT\r\n")
	_ = c.conn.Close()
}

func (c *imapClient) write(data []byte) {
	c.t.Helper()
	for len(data) > 0 {
		n, err := c.conn.Write(data)
		if err != nil {
			c.t.Fatalf("imap write: %v", err)
		}
		if n == 0 {
			c.t.Fatal("imap write made no progress")
		}
		data = data[n:]
	}
}

// readUntil reads past untagged/unsolicited lines until one starts with prefix.
func (c *imapClient) readUntil(prefix string) string {
	c.t.Helper()
	for {
		line, err := c.reader.ReadString('\n')
		if err != nil {
			c.t.Fatalf("imap read waiting for %q: %v", prefix, err)
		}
		if strings.HasPrefix(line, prefix) {
			return line
		}
	}
}

// do runs one command, fails the test unless the server answers OK, and returns
// the untagged lines the command produced.
func (c *imapClient) do(format string, args ...any) []string {
	c.t.Helper()
	c.tag++
	tag := fmt.Sprintf("t%d", c.tag)
	command := fmt.Sprintf(format, args...)
	c.write([]byte(fmt.Sprintf("%s %s\r\n", tag, command)))
	var untagged []string
	for {
		line, err := c.reader.ReadString('\n')
		if err != nil {
			c.t.Fatalf("imap %s: %v", command, err)
		}
		line = strings.TrimRight(line, "\r\n")
		if strings.HasPrefix(line, tag+" ") {
			if !imapTaggedOK(line, tag) {
				c.t.Fatalf("imap %s: %s", command, line)
			}
			return untagged
		}
		untagged = append(untagged, line)
	}
}

// selectFolder selects a mailbox and returns its UIDVALIDITY.
func (c *imapClient) selectFolder(folder string) uint32 {
	c.t.Helper()
	lines := c.do("SELECT %q", folder)
	validity, ok := parseIMAPUIDValidity(lines)
	if !ok {
		c.t.Fatalf("SELECT %q returned no valid UIDVALIDITY: %v", folder, lines)
	}
	return validity
}

// imapAppend adds a message to a folder over raw IMAP, standing in for another
// mail client (e.g. webmail) writing to the mailbox behind meron's back.
func imapAppend(t *testing.T, port int, user, password, folder string, message []byte) {
	t.Helper()
	client := dialIMAP(t, port, user, password)
	defer client.close()
	client.tag++
	tag := fmt.Sprintf("t%d", client.tag)
	client.write([]byte(fmt.Sprintf("%s APPEND %q {%d}\r\n", tag, folder, len(message))))
	client.readUntil("+")
	client.write(message)
	client.write([]byte("\r\n"))
	if line := client.readUntil(tag + " "); !imapTaggedOK(strings.TrimRight(line, "\r\n"), tag) {
		t.Fatalf("imap append failed: %s", line)
	}
}

// imapFlags reads a message's flags straight from the server, e.g.
// `\Seen \Flagged`. Returns "" when the UID is gone.
func imapFlags(t *testing.T, port int, user, password, folder string, uid uint32) string {
	t.Helper()
	client := dialIMAP(t, port, user, password)
	defer client.close()
	client.selectFolder(folder)
	return parseIMAPFlags(client.do("UID FETCH %d (FLAGS)", uid), uid)
}

func imapTaggedOK(line, tag string) bool {
	fields := strings.Fields(line)
	return len(fields) >= 2 && fields[0] == tag && strings.EqualFold(fields[1], "OK")
}

func parseIMAPUIDValidity(lines []string) (uint32, bool) {
	for _, line := range lines {
		upper := strings.ToUpper(line)
		marker := strings.Index(upper, "[UIDVALIDITY ")
		if marker < 0 {
			continue
		}
		value := line[marker+len("[UIDVALIDITY "):]
		if end := strings.IndexByte(value, ']'); end >= 0 {
			value = value[:end]
		}
		parsed, err := strconv.ParseUint(strings.TrimSpace(value), 10, 32)
		if err == nil && parsed > 0 {
			return uint32(parsed), true
		}
	}
	return 0, false
}

func parseIMAPFlags(lines []string, uid uint32) string {
	for _, line := range lines {
		upper := strings.ToUpper(line)
		marker := strings.Index(upper, "UID ")
		if marker < 0 {
			continue
		}
		var got uint32
		if _, err := fmt.Sscanf(upper[marker:], "UID %d", &got); err != nil || got != uid {
			continue
		}
		start := strings.Index(upper, "FLAGS (")
		if start < 0 {
			continue
		}
		rest := line[start+len("FLAGS ("):]
		if end := strings.Index(rest, ")"); end >= 0 {
			return rest[:end]
		}
	}
	return ""
}

// imapSetFlags changes the two user-visible flags as another mail client would.
func imapSetFlags(t *testing.T, port int, user, password, folder string, uid uint32, seen, starred bool) {
	t.Helper()
	client := dialIMAP(t, port, user, password)
	defer client.close()
	client.selectFolder(folder)
	for _, change := range []struct {
		flag    string
		enabled bool
	}{
		{`\Seen`, seen},
		{`\Flagged`, starred},
	} {
		operation := "-FLAGS"
		if change.enabled {
			operation = "+FLAGS"
		}
		client.do("UID STORE %d %s (%s)", uid, operation, change.flag)
	}
}

// imapExpunge deletes a message the way another client would: flag it \Deleted
// and expunge it out of the mailbox.
func imapExpunge(t *testing.T, port int, user, password, folder string, uid uint32) {
	t.Helper()
	client := dialIMAP(t, port, user, password)
	defer client.close()
	client.selectFolder(folder)
	client.do(`UID STORE %d +FLAGS (\Deleted)`, uid)
	client.do("EXPUNGE")
}

// imapUIDValidity reports a mailbox's current UIDVALIDITY.
func imapUIDValidity(t *testing.T, port int, user, password, folder string) uint32 {
	t.Helper()
	client := dialIMAP(t, port, user, password)
	defer client.close()
	return client.selectFolder(folder)
}

// imapRecreateFolder deletes and recreates a mailbox, which is what makes a
// server hand out a fresh UIDVALIDITY and start UIDs over from 1.
func imapRecreateFolder(t *testing.T, port int, user, password, folder string) {
	t.Helper()
	client := dialIMAP(t, port, user, password)
	defer client.close()
	client.do("DELETE %q", folder)
	client.do("CREATE %q", folder)
}

// imapSearchSubject returns the UIDs in a folder whose Subject header matches,
// server side — how many copies of a sent message actually exist, regardless of
// what the local store cached.
func imapSearchSubject(t *testing.T, port int, user, password, folder, subject string) []uint32 {
	t.Helper()
	client := dialIMAP(t, port, user, password)
	defer client.close()
	client.selectFolder(folder)
	return parseIMAPSearch(client.do("UID SEARCH HEADER SUBJECT %q", subject))
}

func parseIMAPSearch(lines []string) []uint32 {
	var uids []uint32
	for _, line := range lines {
		fields := strings.Fields(line)
		if len(fields) < 2 || fields[0] != "*" || !strings.EqualFold(fields[1], "SEARCH") {
			continue
		}
		for _, field := range fields[2:] {
			uid, err := strconv.ParseUint(field, 10, 32)
			if err == nil && uid > 0 {
				uids = append(uids, uint32(uid))
			}
		}
	}
	return uids
}

// rawMessage builds an RFC 5322 message for imapAppend. Headers are passed
// verbatim so a test can deliver encodings and header shapes meron's own send
// path never emits.
func rawMessage(headers []string, body string) []byte {
	joined := strings.Join(headers, "\r\n")
	body = strings.ReplaceAll(body, "\n", "\r\n")
	return []byte(joined + "\r\n\r\n" + body + "\r\n")
}

func foldersContain(result map[string]any, name string) bool {
	folders, _ := result["folders"].([]any)
	for _, item := range folders {
		folder, ok := item.(map[string]any)
		if !ok {
			continue
		}
		if value, _ := folder["name"].(string); strings.EqualFold(value, name) {
			return true
		}
	}
	return false
}

func folderDelimiter(result map[string]any, name string) string {
	folders, _ := result["folders"].([]any)
	for _, item := range folders {
		folder, ok := item.(map[string]any)
		if !ok {
			continue
		}
		if value, _ := folder["name"].(string); strings.EqualFold(value, name) {
			delimiter, _ := folder["delimiter"].(string)
			return delimiter
		}
	}
	return ""
}

func threadLength(result map[string]any) int {
	messages, _ := result["messages"].([]any)
	return len(messages)
}

func threadContainsSubject(result map[string]any, subject string) bool {
	return messagesContainSubject(result, subject)
}

func messagesContainSubject(result map[string]any, subject string) bool {
	rows, _ := result["messages"].([]any)
	for _, row := range rows {
		message, ok := row.(map[string]any)
		if ok && str(message, "subject") == subject {
			return true
		}
	}
	return false
}

func starredMailContainsSubject(result map[string]any, subject string) bool {
	rows, _ := result["items"].([]any)
	for _, row := range rows {
		message, ok := row.(map[string]any)
		if ok && str(message, "subject") == subject {
			return true
		}
	}
	return false
}

func firstThreadMessage(t *testing.T, result map[string]any) map[string]any {
	t.Helper()
	rows, _ := result["messages"].([]any)
	if len(rows) == 0 {
		t.Fatalf("thread has no messages: %v", result)
	}
	message, ok := rows[0].(map[string]any)
	if !ok {
		t.Fatalf("thread row has type %T", rows[0])
	}
	return message
}

func attachmentRows(t *testing.T, message map[string]any) []map[string]any {
	t.Helper()
	raw, _ := message["attachments"].([]any)
	out := make([]map[string]any, 0, len(raw))
	for _, item := range raw {
		attachment, ok := item.(map[string]any)
		if !ok {
			t.Fatalf("attachment has type %T", item)
		}
		out = append(out, attachment)
	}
	return out
}
