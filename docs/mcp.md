# MCP access on desktop

Meron can serve mail and feed data to local MCP clients while the desktop app
is running. Enable it in **Settings → MCP access**. It is disabled by default.

1. Enable the local server.
2. Enter a name for a client and select the accounts it may read. **Select all
   current accounts** selects a snapshot; accounts added later need approval.
3. Optionally grant write permissions: organizing messages, creating drafts,
   sending, and permanent deletion. Each is off by default. Sending and
   permanent deletion additionally choose whether Meron asks for confirmation
   in the app or performs the operation without asking.
4. Choose **Approve and generate credential**, then copy the configuration into
   your client's MCP settings. The credential is shown once. Each client should
   have its own credential.

The endpoint is `http://127.0.0.1:43827/mcp`, using Streamable HTTP with a bearer
credential. The port is settable in the same panel for the one case that cannot
be fixed otherwise — another program already holding the default. Clients then
need the new URL, so it stays fixed by default rather than moving on its own.
The client must support custom authorization headers. Configuration file shapes
vary by client; the generated `mcpServers` example contains the URL and header
to transfer to the client's equivalent settings. This release does
not provide OAuth discovery, remote access, browser connections, or stdio.
Requests are answered as JSON, so a client that asks only for `application/json`
is accepted; it need not also offer `text/event-stream`. Client notifications
Meron has no use for, such as `notifications/roots/list_changed`, are accepted
and ignored rather than failing the session.
If the port is occupied, enabling fails, settings shows the error, and the
listener is left exactly as it was. Close the conflicting program and retry, or
choose another port.

## Permissions

Every approved client can read only its selected accounts. Organizing, draft
creation, sending and permanent deletion are separate, optional permissions.
Edit access or revoke a client in the same panel. Disabling MCP stops the
listener and keeps grants for later use. Revoking deletes the grant and
invalidates its credential. Any change to MCP settings or grants also cancels
approval requests still waiting; the client sees them as `cancelled`. Changing
grants waits for any tool operation already in progress; it cannot undo a
completed read, draft, send, move or deletion. Removing an account also removes
it from all MCP grants, so adding that address again does not restore access
automatically.

Reading does not mark messages read. A client may send retrieved content to its
AI provider; local transport does not imply local AI processing. Email and feed
content is untrusted data, including instructions embedded in it. Nothing in a
message can approve an operation or widen a grant.

### Approving sends and deletions

Unless the grant allows them without confirmation, `send_message`,
`delete_permanently` and `empty_trash` do not act immediately. They return
`status: "pending_approval"` with an `operation_id`, and Meron shows the exact
recipients, subject and body, or the exact messages to be deleted, in a dialog.
Only that dialog can approve; no tool can. Requests expire after five minutes,
and a client may hold at most eight of them at once. The client polls
`operation_status` for `completed`, `failed`, `denied`, `expired` or
`cancelled`; `pending_approval` is not success.

Each of these tools takes a `request_id`. Reusing the same `request_id` returns
the original operation instead of sending or deleting a second time, so a client
that loses a response can retry safely; reusing one with different arguments is
rejected. A `request_id` whose operation was denied, cancelled or expired keeps
returning that outcome, so asking again needs a new one. Operations and their
results are kept for one hour while Meron runs.

## Tools

| Tool | Behavior |
| --- | --- |
| `list_accounts` | Approved account IDs, names, email addresses and provider only; no credentials or server settings. |
| `list_folders` | Folders for one approved account. |
| `search_messages` | Up to 50 conversations, from one account or from every approved account at once when `account_id` is omitted; defaults to INBOX and cached results. `filter: "unread"` or `"starred"` narrows the page before it is returned. Text search includes the selected folder and Sent. Use `server_search: true` with a query to search the mail server. RSS searches use the local cache. |
| `read_thread` | Up to 50 messages using an exact thread ID from search; accepts a pagination cursor. Missing bodies may be filled in the background, so a subsequent read may be needed. |
| `read_attachment` | Returns one cached attachment from an approved account as base64, up to 5 MiB. The key comes from `read_thread`; bytes exist only after that call cached the message. |
| `create_draft` | Optional: creates a new plain text draft with recipients, subject and body, plus `in_reply_to` and `references` to keep a reply in its thread. Review and send it in Meron. |
| `update_draft` | Optional: replaces an existing draft, addressed by its Message-ID — one `create_draft` returned, or one started in Meron and reported as `message_id` by `read_thread`. The whole draft is overwritten. |
| `organize_messages` | Optional: marks selected messages read/unread, stars, archives, trashes or moves them within one account. Trash always moves; it never deletes permanently, including from Drafts. Moving needs IMAP MOVE or UIDPLUS. |
| `send_message` | Optional: sends a plain text message, reply or forward. Usually returns `pending_approval`. |
| `delete_permanently` | Optional: permanently deletes the exact selected UIDs, skipping Trash. Requires IMAP UIDPLUS. Usually returns `pending_approval`. |
| `empty_trash` | Optional: snapshots the Trash folder (at most 1000 messages) and permanently deletes that snapshot; later arrivals are excluded. Usually returns `pending_approval`. |
| `operation_status` | Reports the outcome of an operation from the three tools above. Remains available after a permission is removed so a client can learn its own result. |

Omitting `account_id` searches the inboxes of every approved account and merges
them newest first. The fan-out happens in the MCP layer rather than through the
app's unified view, because that view spans every configured account, including
ones a client was never granted. A merged page has no cursor of its own: it
reports each account's own `next_cursor`, and paging deeper means calling that
account by ID. `folder_id` and `before_cursor` therefore require an
`account_id`, since both are per account. An account that cannot be reached is
reported with its error beside the accounts that answered, rather than failing
the whole search. Keep pagination cursors with their original account, folder
and query. Search responses identify their source; later search pages may use a
server search or its cached snapshot. Cached results are not a complete
inventory of a mailbox.

Mutating tools act on one account at a time and take 1–1000 exact UIDs; the UID
is the final numeric component of a message ID from `read_thread`. Draft
creation always generates a new message ID and never touches an existing draft.
`update_draft` replaces a draft wholesale, so every field must be sent again;
anything omitted is cleared. It writes only in the Drafts folder, so a message
ID from elsewhere in the mailbox adds a draft rather than replacing that
message. Neither tool can set an arbitrary sender or attach files. A reply draft
threads the same way a sent reply does: pass the `message_id` `read_thread`
reports as `in_reply_to`, and that message's references plus its own ID as
`references`. Choosing the recipients of a reply is the client's job; this
layer only carries the headers. Sending is
plain text only, without attachments or a sender override, and is separate from
the draft permission.
Attachments are read from Meron's own media cache, one file per call, and only
from within the approved account's subtree; a larger file stays available
through save and open in Meron. This release does not expose account settings or
filesystem operations.

## Local storage and activity

Client grants and SHA-256 credential hashes are stored in `mcp.json` in Meron's
configuration directory. Plaintext credentials are not persisted by Meron and
cannot be shown again. If one is lost or exposed, use **Replace credential** on
that client: it issues a new credential, shown once, and stops the old one
working immediately while the client keeps its accounts and permissions. Other
clients are unaffected. **Revoke** removes the grant itself.

Settings shows the most recent 100 tool calls since launch: client, tool,
account, timestamp, and outcome. It follows the backend's own change events, so
neither the panel nor the approval prompt polls while nothing is happening. Bodies, subjects, recipients, search queries and
credentials are excluded. Activity is cleared when Meron exits.

The listener binds only to IPv4 loopback, validates the Host header, rejects
browser Origin headers, limits request size, and checks current account grants
at tool execution. MCP tool annotations are descriptive; server-side checks
provide enforcement.

Developing this integration requires Go 1.25 or newer, as required by the
pinned official MCP Go SDK.
