package main

import (
	"errors"
	"fmt"
	"net/mail"
	"strings"

	"github.com/google/uuid"
	"github.com/modelcontextprotocol/go-sdk/mcp"
)

type mcpOrganizeArgs struct {
	AccountID      string   `json:"account_id"`
	FolderID       string   `json:"folder_id" jsonschema:"Exact folder_id of the selected messages"`
	UIDs           []uint32 `json:"uids" jsonschema:"1–1000 message UIDs in this account and folder. The UID is the final numeric component of each message id from read_thread."`
	Action         string   `json:"action" jsonschema:"One of mark_read, mark_unread, star, unstar, archive, trash, move"`
	TargetFolderID string   `json:"target_folder_id,omitempty" jsonschema:"Required for move; within the same account"`
}
type mcpSendArgs struct {
	mcpDraftArgs
	RequestID string `json:"request_id" jsonschema:"Unique ID for this operation. Reuse it unchanged for retries; retained for one hour while Meron stays running."`
}
type mcpDeleteArgs struct {
	AccountID string   `json:"account_id"`
	FolderID  string   `json:"folder_id"`
	UIDs      []uint32 `json:"uids" jsonschema:"Exact message UIDs to delete, 1–1000. The UID is the final numeric component of each message id from read_thread."`
	RequestID string   `json:"request_id"`
}
type mcpEmptyTrashArgs struct {
	AccountID string `json:"account_id"`
	FolderID  string `json:"folder_id" jsonschema:"The actual Trash folder ID returned by list_folders"`
	RequestID string `json:"request_id"`
}
type mcpOperationArgs struct {
	OperationID string `json:"operation_id"`
}

func validateMCPSelection(account, folder string, uids []uint32, empty bool) error {
	if account == "" || account == "unified" || isRSSAccountID(account) {
		return errors.New("A mail account is required")
	}
	if strings.TrimSpace(folder) == "" || len(folder) > 1024 || strings.ContainsAny(folder, "\r\n\x00") {
		return errors.New("A valid folder_id is required")
	}
	if empty {
		return nil
	}
	if len(uids) == 0 || len(uids) > 1000 {
		return errors.New("Select 1–1000 message UIDs")
	}
	seen := map[uint32]bool{}
	for _, uid := range uids {
		if uid == 0 || seen[uid] {
			return errors.New("Invalid or duplicate UID")
		}
		seen[uid] = true
	}
	return nil
}
func (s *mcpService) coreCall(method string, args any) (any, error) {
	if s.app.sidecar == nil || !s.app.sidecar.Started() {
		return nil, s.app.engineUnavailable()
	}
	return s.app.sidecar.Call(method, args)
}
func (s *mcpService) registerMutationTools(server *mcp.Server, c mcpClient) {
	// Kept available after a permission is removed so the client can learn the
	// outcome of its own earlier request without reissuing the mutation.
	mcpRegister(s, server, c, "operation_status", "Check an operation returned by send_message, delete_permanently or empty_trash. pending_approval is not success. Approval can only happen inside Meron. Do not retry a failed send with a new request_id without checking Sent: a network failure can leave its outcome uncertain.", mcpRead, func(mcpOperationArgs) string { return "" }, func(a mcpOperationArgs, grant mcpClient) (any, error) {
		s.expireOperations()
		op := s.operations[a.OperationID]
		if op == nil || op.client.ID != grant.ID || op.client.TokenHash != grant.TokenHash || !mcpAllows(grant, op.Account) {
			return nil, errors.New("Operation not found")
		}
		return op.response(), nil
	})
	if c.Organize {
		mcpRegister(s, server, c, "organize_messages", "Mark selected mail messages read/unread, star/unstar, archive, move to trash or move to another folder in the same account. Trash always moves; it never permanently deletes, including from Drafts. Moving requires IMAP MOVE or UIDPLUS.", mcpOrganize, func(a mcpOrganizeArgs) string { return a.AccountID }, func(a mcpOrganizeArgs, _ mcpClient) (any, error) {
			if err := validateMCPSelection(a.AccountID, a.FolderID, a.UIDs, false); err != nil {
				return nil, err
			}
			switch a.Action {
			case "mark_read", "mark_unread", "star", "unstar", "archive", "trash", "move":
			default:
				return nil, errors.New("Unknown organization action")
			}
			if a.Action == "move" && (strings.TrimSpace(a.TargetFolderID) == "" || strings.ContainsAny(a.TargetFolderID, "\r\n\x00")) {
				return nil, errors.New("A target folder is required")
			}
			result, err := s.coreCall("mcp.organize", map[string]any{"account": a.AccountID, "folder": a.FolderID, "uids": a.UIDs, "action": a.Action, "target_folder": a.TargetFolderID})
			s.notifyMailChanged(a.AccountID)
			return result, err
		})
	}
	if c.Send {
		mcpRegister(s, server, c, "send_message", "Send a plain text email, reply or forward. Usually returns pending_approval for review inside Meron; poll operation_status until completed. Sending is separate from draft permission. Reuse request_id unchanged for retries. No attachments or sender override.", mcpSend, func(a mcpSendArgs) string { return a.AccountID }, func(a mcpSendArgs, grant mcpClient) (any, error) {
			if a.AccountID == "" || isRSSAccountID(a.AccountID) {
				return nil, errors.New("A mail account is required")
			}
			if len(a.Body) > 128*1024 {
				return nil, errors.New("Message body exceeds 128 KiB")
			}
			for _, header := range []string{a.To, a.Cc, a.Bcc, a.Subject, a.InReplyTo, a.References} {
				if len(header) > 8192 || strings.ContainsAny(header, "\r\n\x00") {
					return nil, errors.New("Invalid message header")
				}
			}
			if strings.TrimSpace(a.To) == "" {
				return nil, errors.New("At least one To recipient is required")
			}
			if err := mcpValidateThreading(a.InReplyTo, a.References); err != nil {
				return nil, err
			}
			for _, addresses := range []string{a.To, a.Cc, a.Bcc} {
				if addresses != "" {
					if _, err := mail.ParseAddressList(addresses); err != nil {
						return nil, errors.New("Invalid recipient address")
					}
				}
			}
			previous, fingerprint, err := s.previousOperation(grant, "send_message", a.RequestID, a)
			if err != nil {
				return nil, err
			}
			if previous != nil {
				return previous.response(), nil
			}
			messageID := fmt.Sprintf("<%s@meron.local>", uuid.NewString())
			preview := map[string]any{"kind": "send", "to": a.To, "cc": a.Cc, "bcc": a.Bcc, "subject": a.Subject, "body": a.Body, "in_reply_to": a.InReplyTo, "references": a.References}
			return s.newOperation(grant, "send_message", a.AccountID, a.RequestID, fingerprint, mcpSend, !grant.SendWithoutConfirmation, preview, func() (any, error) {
				return s.app.mailSend(map[string]any{"account_id": a.AccountID, "to": a.To, "cc": a.Cc, "bcc": a.Bcc, "subject": a.Subject, "body": a.Body, "in_reply_to": a.InReplyTo, "references": a.References, "message_id": messageID})
			}), nil
		})
	}
	if c.Delete {
		mcpRegister(s, server, c, "delete_permanently", "Permanently delete the exact selected messages, skipping Trash. Usually returns pending_approval; poll operation_status. Preview is fetched from the server. Requires IMAP UIDPLUS. Reuse request_id unchanged for retries.", mcpDelete, func(a mcpDeleteArgs) string { return a.AccountID }, func(a mcpDeleteArgs, grant mcpClient) (any, error) {
			return s.prepareDelete(grant, "delete_permanently", a.AccountID, a.FolderID, a.UIDs, a.RequestID, false, a)
		})
		mcpRegister(s, server, c, "empty_trash", "Snapshot and permanently delete messages currently in Trash (maximum 1000). New arrivals after the snapshot are excluded. Usually returns pending_approval; poll operation_status. Requires IMAP UIDPLUS. Reuse request_id unchanged for retries.", mcpDelete, func(a mcpEmptyTrashArgs) string { return a.AccountID }, func(a mcpEmptyTrashArgs, grant mcpClient) (any, error) {
			return s.prepareDelete(grant, "empty_trash", a.AccountID, a.FolderID, nil, a.RequestID, true, a)
		})
	}
}
func (s *mcpService) prepareDelete(c mcpClient, tool, account, folder string, uids []uint32, requestID string, empty bool, args any) (any, error) {
	if err := validateMCPSelection(account, folder, uids, empty); err != nil {
		return nil, err
	}
	previous, fingerprint, err := s.previousOperation(c, tool, requestID, args)
	if err != nil {
		return nil, err
	}
	if previous != nil {
		return previous.response(), nil
	}
	if uids == nil {
		uids = []uint32{}
	}
	snapshot, err := s.coreCall("mcp.prepareDelete", map[string]any{"account": account, "folder": folder, "uids": uids, "empty_trash": empty})
	if err != nil {
		return nil, err
	}
	preview := map[string]any{"kind": "delete", "selection": snapshot}
	return s.newOperation(c, tool, account, requestID, fingerprint, mcpDelete, !c.DeleteWithoutConfirmation, preview, func() (any, error) { return s.coreCall("mcp.delete", snapshot) }), nil
}
