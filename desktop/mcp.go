package main

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"crypto/subtle"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/google/uuid"
	"github.com/modelcontextprotocol/go-sdk/mcp"
)

// The default port. Client configurations embed the URL, so the listener keeps
// a fixed port rather than picking a free one: a port that moved on its own
// would silently break every client already configured. It is settable for the
// one case that cannot be fixed otherwise — another program already holding it.
const mcpDefaultPort = 43827
const mcpAddress = "127.0.0.1:43827"

func mcpAddressFor(port int) string {
	return "127.0.0.1:" + strconv.Itoa(port)
}
func (c mcpConfig) port() int {
	if c.Port >= 1024 && c.Port <= 65535 {
		return c.Port
	}
	return mcpDefaultPort
}

// Caller holds s.mu (or s.mu.RLock).
func (s *mcpService) address() string {
	return mcpAddressFor(s.config.port())
}

type mcpClient struct {
	ID                        string   `json:"id"`
	Name                      string   `json:"name"`
	Accounts                  []string `json:"accounts"`
	Drafts                    bool     `json:"drafts"`
	Organize                  bool     `json:"organize"`
	Send                      bool     `json:"send"`
	Delete                    bool     `json:"delete"`
	SendWithoutConfirmation   bool     `json:"send_without_confirmation"`
	DeleteWithoutConfirmation bool     `json:"delete_without_confirmation"`
	TokenHash                 string   `json:"token_hash,omitempty"`
}
type mcpConfig struct {
	Enabled bool        `json:"enabled"`
	Port    int         `json:"port,omitempty"`
	Clients []mcpClient `json:"clients"`
}
type mcpActivity struct {
	Time    string `json:"time"`
	Client  string `json:"client"`
	Tool    string `json:"tool"`
	Account string `json:"account"`
	OK      bool   `json:"ok"`
	Status  string `json:"status,omitempty"`
}
type mcpService struct {
	mu         sync.RWMutex
	config     mcpConfig
	path       string
	app        *App
	server     *http.Server
	listener   net.Listener
	startError string
	activityMu sync.Mutex
	activity   []mcpActivity
	operations map[string]*mcpOperation
}
type mcpClientContextKey struct{}

func (a *App) startMCP() {
	s := &mcpService{app: a, path: filepath.Join(appConfigDir(), "mcp.json")}
	a.mcp = s
	data, err := os.ReadFile(s.path)
	if err == nil {
		if err = json.Unmarshal(data, &s.config); err != nil {
			s.config = mcpConfig{}
			s.startError = "Cannot read MCP settings; access is disabled."
			return
		}
	} else if !os.IsNotExist(err) {
		s.startError = "Cannot read MCP settings; access is disabled."
		return
	}
	if s.config.Enabled {
		s.startError = errorText(s.listen())
	}
}
func errorText(err error) string {
	if err != nil {
		return err.Error()
	}
	return ""
}

// Only hashes are persisted. The credential is returned once to the settings UI.
func (s *mcpService) save(config mcpConfig) error {
	data, err := json.MarshalIndent(config, "", "  ")
	if err != nil {
		return err
	}
	if err = os.MkdirAll(filepath.Dir(s.path), 0700); err != nil {
		return err
	}
	f, err := os.CreateTemp(filepath.Dir(s.path), ".mcp-*")
	if err != nil {
		return err
	}
	defer os.Remove(f.Name())
	if _, err = f.Write(data); err != nil {
		f.Close()
		return err
	}
	if err = f.Close(); err != nil {
		return err
	}
	if err = os.Rename(f.Name(), s.path); err != nil {
		return err
	}
	s.config = config
	s.cancelPending()
	return nil
}
func (s *mcpService) listen() error {
	if s.server != nil {
		return nil
	}
	listener, err := net.Listen("tcp", s.address())
	if err != nil {
		return fmt.Errorf("Cannot start local MCP server: %w", err)
	}
	server := &http.Server{Handler: s.handler(), ReadHeaderTimeout: 5 * time.Second, ReadTimeout: 15 * time.Second, WriteTimeout: 2 * time.Minute, IdleTimeout: 30 * time.Second, MaxHeaderBytes: 8192}
	s.server = server
	s.listener = listener
	go func() {
		if err := server.Serve(listener); err != nil && !errors.Is(err, http.ErrServerClosed) {
			s.mu.Lock()
			if s.server == server {
				s.server = nil
				s.listener = nil
				s.startError = "Local MCP server stopped unexpectedly."
			}
			s.mu.Unlock()
		}
	}()
	return nil
}
func (s *mcpService) close() {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.cancelPending()
	s.stop()
}

// stop shuts the listener down. Caller holds s.mu. http.Server.Close only
// closes listeners its Serve goroutine has already registered, so the socket is
// closed here too: otherwise a stop immediately followed by a start can fail to
// rebind the port it just gave up.
func (s *mcpService) stop() {
	if s.server != nil {
		_ = s.server.Close()
		s.server = nil
	}
	if s.listener != nil {
		_ = s.listener.Close()
		s.listener = nil
	}
}
func (s *mcpService) status() any {
	clients := make([]mcpClient, 0, len(s.config.Clients))
	for _, c := range s.config.Clients {
		c.TokenHash = ""
		clients = append(clients, c)
	}
	s.activityMu.Lock()
	activity := append([]mcpActivity{}, s.activity...)
	s.activityMu.Unlock()
	return map[string]any{"enabled": s.config.Enabled, "running": s.server != nil, "port": s.config.port(), "url": "http://" + s.address() + "/mcp", "clients": clients, "error": s.startError, "activity": activity}
}
func (a *App) mcpSettings(command string, payload map[string]any) (any, error) {
	s := a.mcp
	if s == nil {
		return nil, errors.New("MCP is unavailable")
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	cfg := s.config
	cfg.Clients = append([]mcpClient{}, cfg.Clients...)
	switch command {
	case "mcp.pending":
		return s.pendingApprovals(), nil
	case "mcp.resolve":
		id, _ := payload["id"].(string)
		approve, ok := payload["approve"].(bool)
		if !ok {
			return nil, errors.New("approve is required")
		}
		return s.resolveOperation(id, approve)
	case "mcp.status":
	case "mcp.enable":
		enabled, ok := payload["enabled"].(bool)
		if !ok {
			return nil, errors.New("enabled is required")
		}
		if enabled {
			if err := s.listen(); err != nil {
				s.startError = err.Error()
				return nil, err
			}
		}
		cfg.Enabled = enabled
		if err := s.save(cfg); err != nil {
			if !s.config.Enabled {
				s.stop()
			}
			return nil, err
		}
		if !enabled {
			s.stop()
		}
		s.startError = ""
	case "mcp.setPort":
		port, ok := payload["port"].(float64)
		if !ok || port != float64(int(port)) || int(port) < 1024 || int(port) > 65535 {
			return nil, errors.New("Choose a port between 1024 and 65535")
		}
		if int(port) == cfg.port() {
			break
		}
		cfg.Port = int(port)
		// A port that cannot be taken, or settings that cannot be written, must
		// leave the running server exactly as it was: a typo should not cost the
		// listener that already works.
		previous := s.config
		restore := func() {
			s.config = previous
			if s.server == nil && previous.Enabled {
				s.startError = errorText(s.listen())
			}
		}
		if s.server != nil {
			s.stop()
			s.config = cfg
			if err := s.listen(); err != nil {
				restore()
				return nil, err
			}
		}
		if err := s.save(cfg); err != nil {
			s.stop()
			restore()
			return nil, err
		}
		s.startError = ""
	case "mcp.clientSave":
		var c mcpClient
		if err := decode(payload, &c); err != nil {
			return nil, err
		}
		c.SendWithoutConfirmation = c.Send && c.SendWithoutConfirmation
		c.DeleteWithoutConfirmation = c.Delete && c.DeleteWithoutConfirmation
		c.Name = strings.TrimSpace(c.Name)
		if c.Name == "" || len(c.Name) > 80 {
			return nil, errors.New("Client name must contain 1–80 characters")
		}
		result, err := a.accountList()
		if err != nil {
			return nil, err
		}
		accounts := result.(map[string]any)["accounts"].([]Account)
		allowed := map[string]bool{}
		for _, account := range accounts {
			allowed[account.ID] = true
		}
		selected := map[string]bool{}
		for _, id := range c.Accounts {
			if !allowed[id] || id == "unified" {
				return nil, errors.New("Select existing accounts")
			}
			if selected[id] {
				return nil, errors.New("Duplicate account")
			}
			selected[id] = true
		}
		if len(selected) == 0 {
			return nil, errors.New("Select at least one account")
		}
		token := ""
		if c.ID == "" {
			if len(cfg.Clients) >= 32 {
				return nil, errors.New("Revoke an unused client before adding another")
			}
			var hash string
			var err error
			if token, hash, err = mcpNewCredential(); err != nil {
				return nil, err
			}
			c.ID = uuid.NewString()
			c.TokenHash = hash
			cfg.Clients = append(cfg.Clients, c)
		} else {
			found := false
			for i, old := range cfg.Clients {
				if old.ID == c.ID {
					c.TokenHash = old.TokenHash
					cfg.Clients[i] = c
					found = true
					break
				}
			}
			if !found {
				return nil, errors.New("Client no longer exists")
			}
		}
		if err := s.save(cfg); err != nil {
			return nil, err
		}
		return map[string]any{"status": s.status(), "token": token}, nil
	// The plaintext credential exists only in the one response that issues it, so
	// a lost or exposed secret is replaced here rather than costing the grant.
	case "mcp.clientRegenerate":
		id, _ := payload["id"].(string)
		found := -1
		for i, c := range cfg.Clients {
			if c.ID == id && id != "" {
				found = i
				break
			}
		}
		if found < 0 {
			return nil, errors.New("Client no longer exists")
		}
		token, hash, err := mcpNewCredential()
		if err != nil {
			return nil, err
		}
		cfg.Clients[found].TokenHash = hash
		if err := s.save(cfg); err != nil {
			return nil, err
		}
		return map[string]any{"status": s.status(), "token": token}, nil
	case "mcp.clientRevoke":
		id, _ := payload["id"].(string)
		clients := make([]mcpClient, 0, len(cfg.Clients))
		for _, c := range cfg.Clients {
			if c.ID != id {
				clients = append(clients, c)
			}
		}
		cfg.Clients = clients
		if err := s.save(cfg); err != nil {
			return nil, err
		}
	default:
		return nil, errors.New("Unknown MCP setting")
	}
	return s.status(), nil
}

// mcpNewCredential returns a fresh bearer token and the hash stored for it. The
// token is returned to the settings UI once and never persisted.
func mcpNewCredential() (string, string, error) {
	secret := make([]byte, 32)
	if _, err := rand.Read(secret); err != nil {
		return "", "", err
	}
	token := hex.EncodeToString(secret)
	hash := sha256.Sum256([]byte(token))
	return token, hex.EncodeToString(hash[:]), nil
}

func (s *mcpService) handler() http.Handler {
	protocol := mcp.NewStreamableHTTPHandler(func(r *http.Request) *mcp.Server {
		c := r.Context().Value(mcpClientContextKey{}).(mcpClient)
		return s.tools(c)
	}, &mcp.StreamableHTTPOptions{Stateless: true, JSONResponse: true})
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Cache-Control", "no-store")
		s.mu.RLock()
		address := s.address()
		s.mu.RUnlock()
		// Native clients do not need browser origins. Reject them rather than expose
		// mail to a website, even one served from another loopback port.
		if r.Host != address || len(r.Header.Values("Origin")) != 0 {
			http.Error(w, "Forbidden", http.StatusForbidden)
			return
		}
		if r.URL.Path != "/mcp" || r.URL.RawQuery != "" {
			http.NotFound(w, r)
			return
		}
		token := strings.TrimPrefix(r.Header.Get("Authorization"), "Bearer ")
		hash := sha256.Sum256([]byte(token))
		// Authenticate from current settings; tools recheck the grant at execution.
		s.mu.RLock()
		if !s.config.Enabled {
			s.mu.RUnlock()
			http.Error(w, "MCP disabled", http.StatusForbidden)
			return
		}
		for _, c := range s.config.Clients {
			expected, err := hex.DecodeString(c.TokenHash)
			if err == nil && strings.HasPrefix(r.Header.Get("Authorization"), "Bearer ") && subtle.ConstantTimeCompare(expected, hash[:]) == 1 {
				s.mu.RUnlock()
				r.Body = http.MaxBytesReader(w, r.Body, 1<<20)
				// Replies are always JSON here, so accept clients that omit the
				// event-stream type the streamable transport otherwise demands.
				r.Header.Set("Accept", "application/json, text/event-stream")
				protocol.ServeHTTP(w, r.WithContext(context.WithValue(r.Context(), mcpClientContextKey{}, c)))
				return
			}
		}
		s.mu.RUnlock()
		w.Header().Set("WWW-Authenticate", `Bearer realm="Meron"`)
		http.Error(w, "Unauthorized", http.StatusUnauthorized)
	})
}
func (s *mcpService) record(c mcpClient, tool, account string, err error) {
	s.activityMu.Lock()
	defer s.activityMu.Unlock()
	s.activity = append(s.activity, mcpActivity{Time: time.Now().UTC().Format(time.RFC3339), Client: c.Name, Tool: tool, Account: account, OK: err == nil})
	if len(s.activity) > 100 {
		s.activity = s.activity[len(s.activity)-100:]
	}
	s.notifyActivity()
}
func mcpAllows(c mcpClient, account string) bool {
	if account == "" || account == "unified" {
		return false
	}
	for _, id := range c.Accounts {
		if id == account {
			return true
		}
	}
	return false
}

// Account IDs can be reused when an address is added again. Remove grants
// before deleting the account so a later setup requires fresh approval.
func (s *mcpService) removeAccount(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	cfg := s.config
	cfg.Clients = append([]mcpClient{}, cfg.Clients...)
	changed := false
	for i, c := range cfg.Clients {
		selected := make([]string, 0, len(c.Accounts))
		for _, account := range c.Accounts {
			if account != id {
				selected = append(selected, account)
			} else {
				changed = true
			}
		}
		cfg.Clients[i].Accounts = selected
	}
	if !changed {
		return nil
	}
	return s.save(cfg)
}
