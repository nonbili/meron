package main

type Account struct {
	ID                string  `json:"id"`
	Email             string  `json:"email"`
	DisplayName       string  `json:"display_name"`
	SenderName        string  `json:"sender_name"`
	AvatarURL         string  `json:"avatar_url,omitempty"`
	Provider          string  `json:"provider"`
	AuthType          string  `json:"auth_type"`
	IMAPHost          string  `json:"imap_host"`
	IMAPPort          uint16  `json:"imap_port"`
	SMTPHost          string  `json:"smtp_host"`
	SMTPPort          uint16  `json:"smtp_port"`
	TLS               bool    `json:"tls"`
	LoadRemoteImages  bool    `json:"load_remote_images"`
	ConversationHTML  bool    `json:"conversation_html"`
	ChatWallpaper     any     `json:"chat_wallpaper,omitempty"`
	IncludedInUnified bool    `json:"included_in_unified"`
	Muted             bool    `json:"muted"`
	Paused            bool    `json:"paused"`
	NeedsReconnect    bool    `json:"needs_reconnect,omitempty"`
	FeedURL           string  `json:"feed_url,omitempty"`
	AccessToken       string  `json:"access_token,omitempty"`
	RefreshToken      string  `json:"refresh_token,omitempty"`
	TokenExpiresAt    int64   `json:"token_expires_at,omitempty"`
	SortOrder         int     `json:"sort_order"`
	Aliases           []Alias `json:"aliases,omitempty"`
}

// Alias is a send-as identity for an account: an address the user owns plus an
// optional From display name (blank falls back to the account's sender name).
type Alias struct {
	Email string `json:"email"`
	Name  string `json:"name,omitempty"`
}

type Folder struct {
	ID        string `json:"id"`
	AccountID string `json:"account_id"`
	Name      string `json:"name"`
	Role      string `json:"role"`
	Delimiter string `json:"delimiter"`
	Unread    uint32 `json:"unread"`
}

type Message struct {
	ID               string `json:"id"`
	AccountID        string `json:"account_id"`
	FolderID         string `json:"folder_id"`
	ThreadID         string `json:"thread_id"`
	FromName         string `json:"from_name"`
	FromAddr         string `json:"from_addr"`
	To               string `json:"to"`
	ReplyTo          string `json:"reply_to,omitempty"`
	Cc               string `json:"cc,omitempty"`
	Bcc              string `json:"bcc,omitempty"`
	MessageID        string `json:"message_id,omitempty"`
	References       string `json:"references,omitempty"`
	Subject          string `json:"subject"`
	Preview          string `json:"preview"`
	Body             string `json:"body"`
	BodyHTML         string `json:"body_html,omitempty"`
	Date             int64  `json:"date"`
	Unread           bool   `json:"unread"`
	UnreadCount      uint32 `json:"unread_count,omitempty"`
	Starred          bool   `json:"starred"`
	HasAttachments   bool   `json:"has_attachments"`
	Attachments      any    `json:"attachments,omitempty"`
	OriginalThreadID string `json:"original_thread_id,omitempty"`
	// RecipientOverflow is the count of additional recipients beyond the one shown
	// on an outbound thread card (for a "+N" hint); 0 for inbound/single-recipient.
	RecipientOverflow uint32 `json:"recipient_overflow,omitempty"`
}

type AddPasswordAccountRequest struct {
	Email       string `json:"email"`
	DisplayName string `json:"display_name"`
	SenderName  string `json:"sender_name"`
	IMAPHost    string `json:"imap_host"`
	IMAPPort    uint16 `json:"imap_port"`
	SMTPHost    string `json:"smtp_host"`
	SMTPPort    uint16 `json:"smtp_port"`
	Username    string `json:"username"`
	Password    string `json:"password"`
	TLS         bool   `json:"tls"`
}

type AddGmailOAuthRequest struct {
	Email        string `json:"email"`
	DisplayName  string `json:"display_name"`
	SenderName   string `json:"sender_name"`
	AvatarURL    string `json:"avatar_url"`
	AuthCode     string `json:"auth_code"`
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	ExpiresIn    int64  `json:"expires_in"`
}

// AddOutlookOAuthRequest mirrors AddGmailOAuthRequest. AvatarURL is unused
// (Microsoft's id_token carries no picture) but kept for shape parity.
type AddOutlookOAuthRequest struct {
	Email        string `json:"email"`
	DisplayName  string `json:"display_name"`
	SenderName   string `json:"sender_name"`
	AvatarURL    string `json:"avatar_url"`
	AuthCode     string `json:"auth_code"`
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	ExpiresIn    int64  `json:"expires_in"`
}

type FolderListRequest struct {
	AccountID string `json:"account_id"`
	Refresh   bool   `json:"refresh"`
}

type FolderCreateRequest struct {
	AccountID string `json:"account_id"`
	Name      string `json:"name"`
}

type ThreadListRequest struct {
	AccountID    string `json:"account_id"`
	FolderID     string `json:"folder_id"`
	Query        string `json:"query"`
	Filter       string `json:"filter"`
	BeforeCursor string `json:"before_cursor"`
	Refresh      bool   `json:"refresh"`
}

type AttachmentInput struct {
	Filename string `json:"filename"`
	Mime     string `json:"mime"`
	Data     string `json:"data"` // base64 encoded
	InlineID string `json:"inline_id"`
}

type SendMailRequest struct {
	AccountID   string            `json:"account_id"`
	To          string            `json:"to"`
	Cc          string            `json:"cc"`
	Bcc         string            `json:"bcc"`
	Subject     string            `json:"subject"`
	Body        string            `json:"body"`
	Html        string            `json:"html"`
	InReplyTo   string            `json:"in_reply_to"`
	References  string            `json:"references"`
	ReplyTo     string            `json:"reply_to"`
	From        string            `json:"from"`
	DraftID     string            `json:"draft_id"`
	MessageID   string            `json:"message_id"`
	Attachments []AttachmentInput `json:"attachments"`
}

type ExchangedProfile struct {
	Email        string `json:"email"`
	DisplayName  string `json:"display_name"`
	AvatarURL    string `json:"avatar_url"`
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	ExpiresIn    int64  `json:"expires_in"`
	AuthCode     string `json:"auth_code"`
}

type TokenExchangeResult struct {
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	ExpiresIn    int64  `json:"expires_in"`
	// IDToken is the OIDC id_token Microsoft returns; Google omits it. Used to
	// read the account email/name without an extra userinfo call.
	IDToken string `json:"id_token"`
}

type GoogleUserInfo struct {
	Email   string `json:"email"`
	Name    string `json:"name"`
	Picture string `json:"picture"`
}

type ImapThreadIDs struct {
	Account   string
	Folder    string
	ThreadKey string
	UID       uint32
}
