package main

import (
	"encoding/json"
	"errors"
	"fmt"

	"github.com/microcosm-cc/bluemonday"
	"github.com/modelcontextprotocol/go-sdk/mcp"
	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

// Configuration grants are app-wide and deliberately independent of mail grants.
// Never forward arbitrary bridge commands or arbitrary preference keys from MCP.
type mcpSettingSpec struct {
	Type        string   `json:"type"`
	Description string   `json:"description"`
	Values      []string `json:"values,omitempty"`
}

var mcpAppSettings = map[string]mcpSettingSpec{
	"signature":                     {Type: "string", Description: "App-wide signature HTML; empty disables it"},
	"send_shortcut":                 {Type: "string", Description: "Quick reply send shortcut", Values: []string{"enter", "mod_enter"}},
	"conversation_layout":           {Type: "string", Description: "Message layout", Values: []string{"chat", "traditional"}},
	"spell_check":                   {Type: "boolean", Description: "Spell checking in composers"},
	"show_real_avatars":             {Type: "boolean", Description: "Show sender avatars"},
	"show_unread_account_badge":     {Type: "boolean", Description: "Show unread badges on accounts"},
	"show_unified_inbox_in_sidenav": {Type: "boolean", Description: "Show unified inbox in navigation"},
	"font_family":                   {Type: "string", Description: "Interface font family; empty uses default"},
	"message_font_family":           {Type: "string", Description: "Message font family; empty follows interface"},
}

// Unlike editor-produced signatures, MCP HTML is arbitrary input. Apply a
// parser-based allowlist before persistence. Reject transformations rather than
// silently losing migration data (such as inline CSS or embedded logos).
var mcpSignaturePolicy = func() *bluemonday.Policy {
	p := bluemonday.UGCPolicy()
	p.RequireNoFollowOnLinks(false)
	return p
}()

func mcpSanitizeSetting(key string, value any) any {
	if key != "signature" {
		return value
	}
	if html, ok := value.(string); ok {
		return mcpSignaturePolicy.Sanitize(html)
	}
	if signature, ok := value.(map[string]any); ok {
		out := map[string]any{"mode": signature["mode"]}
		if html, ok := signature["html"].(string); ok {
			out["html"] = mcpSignaturePolicy.Sanitize(html)
		}
		return out
	}
	return value
}

func mcpValidateSignatureHTML(key string, value any) error {
	if key != "signature" {
		return nil
	}
	html, ok := value.(string)
	if object, isObject := value.(map[string]any); isObject {
		html, ok = object["html"].(string)
	}
	if ok && mcpSignaturePolicy.Sanitize(html) != html {
		return errors.New("Signature HTML would be changed by Meron's MCP allowlist; nothing was saved. Inline styles, embedded data: images, active content, unsupported attributes and unsafe URLs are not supported. Submit simplified HTML with HTTPS image URLs, or recreate the signature in Meron's editor. HTML must also use normalized markup (double-quoted attributes and escaped entities)")
	}
	return nil
}

var mcpAccountSettings = map[string]mcpSettingSpec{
	"display_name":        {Type: "string", Description: "Account label"},
	"sender_name":         {Type: "string", Description: "Default From display name"},
	"aliases":             {Type: "aliases", Description: "Complete send-as identity list: [{email, name}]; empty clears"},
	"signature":           {Type: "signature", Description: "{mode: custom, html: ...} or {mode: none}; null follows app signature"},
	"load_remote_images":  {Type: "boolean", Description: "Automatically load remote images"},
	"conversation_html":   {Type: "boolean", Description: "Display HTML in conversations"},
	"included_in_unified": {Type: "boolean", Description: "Include in unified inbox"},
	"muted":               {Type: "boolean", Description: "Mute notifications"},
	"paused":              {Type: "boolean", Description: "Pause synchronization"},
	"save_sent_copy":      {Type: "nullable_boolean", Description: "Save sent copies; null uses provider default"},
}

type mcpSetSettingArgs struct {
	Key   string `json:"key" jsonschema:"Setting key returned by configuration_capabilities"`
	Value any    `json:"value" jsonschema:"Value matching the capability type; only this setting changes"`
}

// MCP setup accepts normal validated TLS only. Certificate exceptions remain
// in the interactive account dialog, where the user can inspect the certificate.
type mcpCreateAccountArgs struct {
	Email        string `json:"email"`
	DisplayName  string `json:"display_name,omitempty"`
	SenderName   string `json:"sender_name,omitempty"`
	IMAPHost     string `json:"imap_host"`
	IMAPPort     uint16 `json:"imap_port,omitempty"`
	SMTPHost     string `json:"smtp_host"`
	SMTPPort     uint16 `json:"smtp_port,omitempty"`
	Username     string `json:"username"`
	Password     string `json:"password"`
	TLS          *bool  `json:"tls,omitempty"`
	StartTLS     *bool  `json:"starttls,omitempty"`
	SMTPTLS      *bool  `json:"smtp_tls,omitempty"`
	SMTPStartTLS *bool  `json:"smtp_starttls,omitempty"`
}

func (args mcpCreateAccountArgs) validateTLS() error {
	imapPort, smtpPort := args.IMAPPort, args.SMTPPort
	if imapPort == 0 {
		imapPort = 993
	}
	if smtpPort == 0 {
		smtpPort = 465
	}
	legacy := true
	if args.TLS != nil {
		legacy = *args.TLS
	}
	var explicit *bool
	if args.StartTLS != nil {
		explicit = args.TLS
	}
	imapTLS, imapStartTLS := selectedTLSMode(legacy, explicit, args.StartTLS, imapPort)
	smtpTLS, smtpStartTLS := selectedTLSMode(legacy, args.SMTPTLS, args.SMTPStartTLS, smtpPort)
	if (!imapTLS && !imapStartTLS) || (!smtpTLS && !smtpStartTLS) {
		return errors.New("MCP account setup requires TLS or STARTTLS for both IMAP and SMTP; use Meron's account dialog for other connections")
	}
	return nil
}

type mcpSetAccountSettingArgs struct {
	mcpSetSettingArgs
	AccountID string `json:"account_id"`
}

func mcpValidateSetting(specs map[string]mcpSettingSpec, key string, value any) error {
	// Match the core's MAX_SIGNATURE_HTML byte limit for both signature paths.
	if key == "signature" {
		html, _ := value.(string)
		if object, ok := value.(map[string]any); ok {
			html, _ = object["html"].(string)
		}
		if len(html) > 256*1024 {
			return errors.New("Signature HTML must not exceed 256 KiB")
		}
	}
	if err := mcpValidateSignatureHTML(key, value); err != nil {
		return err
	}
	spec, ok := specs[key]
	if !ok {
		return fmt.Errorf("Unsupported setting %q; see configuration_capabilities", key)
	}
	valid := false
	switch spec.Type {
	case "string":
		v, ok := value.(string)
		valid = ok
		if ok && len(spec.Values) > 0 {
			valid = false
			for _, allowed := range spec.Values {
				if v == allowed {
					valid = true
				}
			}
		}
	case "boolean":
		_, valid = value.(bool)
	case "nullable_boolean":
		_, valid = value.(bool)
		valid = valid || value == nil
	case "signature":
		if value == nil {
			valid = true
		} else if v, ok := value.(map[string]any); ok {
			mode, _ := v["mode"].(string)
			_, html := v["html"].(string)
			valid = (mode == "custom" && html) || mode == "none"
		}
	case "aliases":
		if list, ok := value.([]any); ok {
			valid = true
			for _, item := range list {
				v, ok := item.(map[string]any)
				if !ok {
					valid = false
					break
				}
				email, ok := v["email"].(string)
				if !ok || email == "" {
					valid = false
					break
				}
				if name, exists := v["name"]; exists {
					if _, ok := name.(string); !ok {
						valid = false
					}
				}
			}
		}
	}
	if !valid {
		return fmt.Errorf("Invalid value for %s (%s)", key, spec.Type)
	}
	return nil
}

func (s *mcpService) configurationAccounts() ([]Account, error) {
	if s.app.sidecar == nil || !s.app.sidecar.Started() {
		return nil, s.app.engineUnavailable()
	}
	res, err := s.app.accountList()
	if err != nil {
		return nil, err
	}
	accounts := res.(map[string]any)["accounts"].([]Account)
	for i := range accounts {
		accounts[i].AccessToken = ""
		accounts[i].RefreshToken = ""
		accounts[i].TokenExpiresAt = 0
		// Proxy configuration may contain a password.
		accounts[i].Proxy = nil
	}
	return accounts, nil
}

func (s *mcpService) notifyConfigurationChanged(keys ...string) {
	if s.app.ctx != nil {
		wailsRuntime.EventsEmit(s.app.ctx, "configuration.changed", map[string]any{"keys": keys})
	}
}

func (s *mcpService) registerConfigurationTools(server *mcp.Server, c mcpClient) {
	none := func(struct{}) string { return "" }
	if c.ManageAccounts || c.ManageSettings {
		mcpRegister(s, server, c, "configuration_capabilities", "Discover Meron's receiving interfaces. Translate external configuration into these supported settings; no source-specific backup format is required. Configuration permissions are app-wide and do not grant mail access.", mcpReadConfiguration, none, func(_ struct{}, grant mcpClient) (any, error) {
			out := map[string]any{}
			if grant.ManageAccounts {
				out["account_settings"] = mcpAccountSettings
				out["account_creation"] = "create_password_account connects and saves an IMAP/SMTP account. Password or app password required. OAuth accounts must be authenticated in Meron. Existing connection settings cannot be replaced through this tool."
			}
			if grant.ManageSettings {
				out["app_settings"] = mcpAppSettings
			}
			return out, nil
		})
	}
	if c.ManageAccounts {
		mcpRegister(s, server, c, "list_account_configurations", "List configuration for every account, without passwords, OAuth tokens or proxy credentials. Requires app-wide Manage accounts; does not grant access to messages.", mcpReadAccounts, none, func(_ struct{}, _ mcpClient) (any, error) {
			accounts, err := s.configurationAccounts()
			return map[string]any{"accounts": accounts}, err
		})
		mcpRegister(s, server, c, "create_password_account", "Connect and save a new IMAP/SMTP account using a password or app password. Requires validated TLS or STARTTLS for both servers; plaintext and certificate pins are not supported over MCP. Ports default to 993 and 465. Set tls/starttls and smtp_tls/smtp_starttls explicitly for STARTTLS. Rejects existing account IDs rather than overwriting credentials. Does not add mail grants; an existing all-accounts grant covers the new account. Revoking access does not cancel an already dispatched setup. OAuth and certificate exceptions are available inside Meron.", mcpCreateAccount, func(a mcpCreateAccountArgs) string { return accountID(a.Email) }, func(args mcpCreateAccountArgs, _ mcpClient) (any, error) {
			if err := args.validateTLS(); err != nil {
				return nil, err
			}
			accounts, err := s.configurationAccounts()
			if err != nil {
				return nil, err
			}
			for _, account := range accounts {
				if account.ID == accountID(args.Email) {
					return nil, errors.New("Account already exists; use set_account_setting for preferences or reconnect inside Meron")
				}
			}
			data, err := json.Marshal(args)
			if err != nil {
				return nil, err
			}
			var payload map[string]any
			if err := json.Unmarshal(data, &payload); err != nil {
				return nil, err
			}
			result, err := s.app.accountAddPassword(payload)
			if err == nil {
				s.notifyConfigurationChanged()
			}
			return result, err
		})
		mcpRegister(s, server, c, "set_account_setting", "Set one preference on any existing account. Discover keys and value types with configuration_capabilities. Aliases replace the entire list. Disabling a signature preserves its saved HTML when html is omitted. Does not change server credentials or mail permissions.", mcpManageAccounts, func(a mcpSetAccountSettingArgs) string { return a.AccountID }, func(args mcpSetAccountSettingArgs, _ mcpClient) (any, error) {
			if err := mcpValidateSetting(mcpAccountSettings, args.Key, args.Value); err != nil {
				return nil, err
			}
			args.Value = mcpSanitizeSetting(args.Key, args.Value)
			accounts, err := s.configurationAccounts()
			if err != nil {
				return nil, err
			}
			found := false
			for _, account := range accounts {
				if account.ID == args.AccountID {
					found = true
					if args.Key == "signature" {
						if incoming, ok := args.Value.(map[string]any); ok && incoming["mode"] != "custom" {
							if _, supplied := incoming["html"]; !supplied {
								if previous, ok := account.Signature.(map[string]any); ok {
									incoming["html"] = previous["html"]
								}
							}
						}
					}
				}
			}
			if !found {
				return nil, errors.New("Account does not exist")
			}
			method, field := "", "enabled"
			switch args.Key {
			case "display_name":
				method, field = "account.setName", "name"
			case "sender_name":
				method, field = "account.setSenderName", "name"
			case "aliases":
				method, field = "account.setAliases", "aliases"
			case "signature":
				method, field = "account.setSignature", "signature"
			case "load_remote_images":
				method = "account.setImages"
			case "conversation_html":
				method = "account.setConversationHtml"
			case "included_in_unified":
				method = "account.setUnified"
			case "muted":
				method = "account.setMuted"
			case "paused":
				method = "account.setPaused"
			case "save_sent_copy":
				method, field = "account.setSaveSentCopy", "value"
			}
			result, err := s.app.sidecar.Call(method, map[string]any{"account": args.AccountID, field: args.Value})
			if err == nil {
				s.notifyConfigurationChanged()
			}
			return result, err
		})
	}
	if c.ManageSettings {
		mcpRegister(s, server, c, "get_app_settings", "Read supported app-wide preferences. Missing keys use Meron's defaults. Does not expose internal session state or credentials.", mcpReadSettings, none, func(_ struct{}, _ mcpClient) (any, error) {
			if s.app.sidecar == nil || !s.app.sidecar.Started() {
				return nil, s.app.engineUnavailable()
			}
			keys := []string{}
			for key := range mcpAppSettings {
				keys = append(keys, key)
			}
			return s.app.appPrefsGet(map[string]any{"keys": keys})
		})
		mcpRegister(s, server, c, "set_app_setting", "Set one supported app-wide preference. Discover keys and allowed values with configuration_capabilities. Applies immediately in Meron.", mcpManageSettings, func(mcpSetSettingArgs) string { return "" }, func(args mcpSetSettingArgs, _ mcpClient) (any, error) {
			if err := mcpValidateSetting(mcpAppSettings, args.Key, args.Value); err != nil {
				return nil, err
			}
			args.Value = mcpSanitizeSetting(args.Key, args.Value)
			if s.app.sidecar == nil || !s.app.sidecar.Started() {
				return nil, s.app.engineUnavailable()
			}
			result, err := s.app.appPrefsSet(map[string]any{"key": args.Key, "value": args.Value})
			if err == nil {
				s.notifyConfigurationChanged(args.Key)
			}
			return result, err
		})
	}
}
