package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"time"

	"github.com/google/uuid"
	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

type mcpPermission string

const (
	mcpRead     mcpPermission = "read"
	mcpDraft    mcpPermission = "draft"
	mcpOrganize mcpPermission = "organize"
	mcpSend     mcpPermission = "send"
	mcpDelete   mcpPermission = "delete"
)

func mcpPermitted(c mcpClient, p mcpPermission) bool {
	switch p {
	case mcpRead:
		return true
	case mcpDraft:
		return c.Drafts
	case mcpOrganize:
		return c.Organize
	case mcpSend:
		return c.Send
	case mcpDelete:
		return c.Delete
	}
	return false
}

type mcpOperation struct {
	ID          string    `json:"id"`
	ClientName  string    `json:"client"`
	Account     string    `json:"account"`
	Tool        string    `json:"tool"`
	Status      string    `json:"status"`
	ExpiresAt   time.Time `json:"expires_at"`
	Preview     any       `json:"preview,omitempty"`
	client      mcpClient
	permission  mcpPermission
	requestID   string
	fingerprint string
	created     time.Time
	execute     func() (any, error)
	result      any
	failure     string
}

func (op *mcpOperation) response() map[string]any {
	return map[string]any{"operation_id": op.ID, "status": op.Status, "expires_at": op.ExpiresAt, "result": op.result, "error": op.failure}
}

// Caller holds s.mu. Results remain available for one hour in this process;
// request IDs prevent network retries from repeating sends or deletions.
func (s *mcpService) previousOperation(c mcpClient, tool, requestID string, args any) (*mcpOperation, string, error) {
	if len(requestID) < 1 || len(requestID) > 128 {
		return nil, "", errors.New("request_id must contain 1–128 characters")
	}
	encoded, err := json.Marshal(args)
	if err != nil {
		return nil, "", err
	}
	hash := sha256.Sum256(encoded)
	fingerprint := hex.EncodeToString(hash[:])
	s.expireOperations()
	for _, op := range s.operations {
		if op.client.ID == c.ID && op.requestID == requestID {
			if op.Tool != tool || op.fingerprint != fingerprint {
				return nil, "", errors.New("request_id was already used with different arguments")
			}
			return op, fingerprint, nil
		}
	}
	pending := 0
	for _, op := range s.operations {
		if op.client.ID == c.ID && op.Status == "pending_approval" {
			pending++
		}
	}
	if pending >= 8 {
		return nil, "", errors.New("Resolve pending approvals before requesting more")
	}
	if len(s.operations) >= 64 {
		return nil, "", errors.New("Too many recent operations; try again later")
	}
	return nil, fingerprint, nil
}
func (s *mcpService) newOperation(c mcpClient, tool, account, requestID, fingerprint string, p mcpPermission, ask bool, preview any, execute func() (any, error)) any {
	if s.operations == nil {
		s.operations = map[string]*mcpOperation{}
	}
	op := &mcpOperation{ID: uuid.NewString(), ClientName: c.Name, Account: account, Tool: tool, Status: "pending_approval", ExpiresAt: time.Now().Add(5 * time.Minute), Preview: preview, client: c, permission: p, requestID: requestID, fingerprint: fingerprint, created: time.Now(), execute: execute}
	s.operations[op.ID] = op
	if ask {
		s.notifyApprovals()
	} else {
		s.executeOperation(op)
	}
	return op.response()
}
func (s *mcpService) expireOperations() {
	now := time.Now()
	expired := false
	for id, op := range s.operations {
		if op.Status == "pending_approval" && !now.Before(op.ExpiresAt) {
			op.Status = "expired"
			op.execute = nil
			op.Preview = nil
			expired = true
		}
		if now.Sub(op.created) > time.Hour {
			delete(s.operations, id)
		}
	}
	if expired {
		s.notifyApprovals()
	}
}
func (s *mcpService) cancelPending() {
	cancelled := false
	for _, op := range s.operations {
		if op.Status == "pending_approval" {
			op.Status = "cancelled"
			op.execute = nil
			op.Preview = nil
			cancelled = true
		}
	}
	if cancelled {
		s.notifyApprovals()
	}
}
func (s *mcpService) pendingApprovals() any {
	s.expireOperations()
	pending := []*mcpOperation{}
	for _, op := range s.operations {
		if op.Status == "pending_approval" {
			pending = append(pending, op)
		}
	}
	// Return values detached from mutable backend state before releasing the lock.
	data, _ := json.Marshal(pending)
	var out any
	_ = json.Unmarshal(data, &out)
	return out
}
func (s *mcpService) resolveOperation(id string, approve bool) (any, error) {
	s.expireOperations()
	op := s.operations[id]
	if op == nil {
		return nil, errors.New("Request no longer exists")
	}
	if op.Status != "pending_approval" {
		return op.response(), nil
	}
	if !approve {
		op.Status = "denied"
		op.execute = nil
		op.Preview = nil
		s.notifyApprovals()
		return op.response(), nil
	}
	// No MCP tool exposes this method. The trusted Wails UI alone can approve.
	allowed := false
	for _, c := range s.config.Clients {
		if c.ID == op.client.ID && c.TokenHash == op.client.TokenHash && mcpAllows(c, op.Account) && mcpPermitted(c, op.permission) {
			allowed = true
			break
		}
	}
	if !s.config.Enabled || !allowed {
		op.Status = "cancelled"
		op.execute = nil
		op.Preview = nil
		s.notifyApprovals()
		return op.response(), nil
	}
	s.executeOperation(op)
	return op.response(), nil
}
func (s *mcpService) executeOperation(op *mcpOperation) {
	op.Status = "running"
	result, err := op.execute()
	op.result = result
	op.execute = nil
	op.Preview = nil
	if err != nil {
		op.Status = "failed"
		op.failure = err.Error()
	} else {
		op.Status = "completed"
	}
	s.record(op.client, op.Tool, op.Account, err)
	s.notifyApprovals()
	// A failed transport can still mean the mutation landed; refresh either way.
	s.notifyMailChanged(op.Account)
}

// The approval prompt and the settings panel listen for these instead of
// polling: an idle Meron should not be asking the backend anything at all.
func (s *mcpService) notifyApprovals() {
	if s.app != nil && s.app.ctx != nil {
		wailsRuntime.EventsEmit(s.app.ctx, "mcp.approvals")
	}
}
func (s *mcpService) notifyActivity() {
	if s.app != nil && s.app.ctx != nil {
		wailsRuntime.EventsEmit(s.app.ctx, "mcp.activity")
	}
}
func (s *mcpService) notifyMailChanged(account string) {
	if s.app.ctx != nil {
		wailsRuntime.EventsEmit(s.app.ctx, "mail.synced", map[string]any{"account": account, "folders": true})
	}
}
