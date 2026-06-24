// Shared data-model types, mirroring the sidecar's bridge shapes. Imported by
// both the state modules and the UI; no runtime code lives here.

export type AuthType = 'password' | 'gmail_oauth' | 'outlook_oauth' | 'rss'

/** A send-as identity for an account: an owned address and an optional From
 * display name (blank falls back to the account's `sender_name`). */
export type Alias = {
  email: string
  name?: string
}

export type Account = {
  id: string
  email: string
  display_name: string
  avatar_url?: string
  provider: string
  auth_type: AuthType
  imap_host: string
  imap_port: number
  smtp_host: string
  smtp_port: number
  tls: boolean
  /** Whether remote (URL-based) inline images render for this account. */
  load_remote_images?: boolean
  /** Whether message views prefer original HTML when available (default true). */
  conversation_html?: boolean
  /** Per-account conversation background; absent uses Meron's default pattern. */
  chat_wallpaper?: ChatWallpaper | null
  /** Whether this account's inbox folds into the unified inbox (default true). */
  included_in_unified?: boolean
  /** Whether new-mail desktop notifications are suppressed (default false). */
  muted?: boolean
  /** Whether automatic checking for new messages is paused (default false). */
  paused?: boolean
  /** True when account metadata was restored but the OS keychain secret is missing. */
  needs_reconnect?: boolean
  /** RSS automatic sync interval in minutes (default 60). */
  rss_sync_interval_minutes?: number
  feed_url?: string
  sender_name?: string
  /** Additional send-as addresses (besides the primary `email`). */
  aliases?: Alias[]
}

export type ChatWallpaper = { kind: 'preset'; presetId: string } | { kind: 'custom'; url: string }

export type Folder = {
  id: string
  account_id: string
  name: string
  role: string
  delimiter?: string
  unread: number
}

/** A correspondent surfaced for recipient autocomplete. */
export type Contact = {
  name: string
  addr: string
}

export type Attachment = {
  filename: string
  mime: string
  size: number
  /** Relative media key served at `/media/<key>`; null for non-image files. */
  key: string | null
  /** Remote image URL (RSS inline images); null/absent for local attachments. */
  url?: string | null
}

export type ComposerAttachment = {
  id: string
  filename: string
  mime: string
  size: number
  data: string // base64 encoded
  /** Content-ID for images embedded in the rich-text body. */
  inlineId?: string
}

export type Message = {
  id: string
  account_id: string
  folder_id: string
  thread_id: string
  from_name: string
  from_addr: string
  to: string
  /** Comma-separated Reply-To addresses from the original message ("Name <addr>" or "addr"). */
  reply_to?: string
  /** Comma-separated Cc addresses from the original message. */
  cc?: string
  /** Comma-separated Bcc addresses. Present only on outgoing copies (Sent/Drafts);
   * received messages never carry Bcc (the sending server strips it). */
  bcc?: string
  /** Normalized Message-ID ("id@host", no angle brackets). Replies use this for In-Reply-To. */
  message_id?: string
  /** Normalized References chain (space-separated bare ids). */
  references?: string
  subject: string
  preview: string
  body: string
  /** Iframe-ready original email HTML for "HTML mode"; absent for plain-text messages. */
  body_html?: string
  /** Send time as Unix epoch seconds (0 when unknown). Format via lib/date helpers. */
  date: number
  unread: boolean
  unread_count?: number
  starred: boolean
  has_attachments: boolean
  attachments?: Attachment[]
  /** Source feed URL; present on RSS feed threads only. */
  feed_url?: string
  /** Cached feed-icon media key (served at `/media/<key>`); present on RSS feed
   * threads only, empty when the feed declared no icon or it isn't cached yet. */
  feed_icon?: string
  original_thread_id?: string
  /** On an outbound thread card, the count of recipients beyond the one shown,
   * rendered as a "+N" hint. Absent/0 for inbound or single-recipient threads. */
  recipient_overflow?: number
  /** Local send lifecycle for an optimistically-rendered outgoing message.
   * Absent on messages loaded from the engine (treated as already sent). */
  send_status?: 'sending' | 'sent' | 'failed'
}

// Editable state for a compose/reply draft living inside a compose tab.
export type ComposeDraft = {
  accountId: string
  /** Chosen send-as address (the account's primary or one of its aliases).
   * Empty means the account's primary address. */
  fromEmail: string
  to: string
  cc: string
  bcc: string
  /** Outgoing `Reply-To` header. Optional — most messages don't set one. */
  replyTo: string
  subject: string
  rich: boolean // true = rich-text (HTML) editor, false = plaintext
  html: string // body when rich
  text: string // body when plaintext
  showCcBcc: boolean
  /** Parent message's bare Message-ID, used to set In-Reply-To/References on send. */
  inReplyTo: string
  /** Parent's References chain (space-separated bare ids) plus parent's Message-ID. */
  references: string
  /** Stable Message-ID reused across draft autosaves so the server-side Drafts
   * copy is replaced in place instead of duplicated. Generated on tab creation. */
  draftMessageId: string
  /** Server-side draft row this compose tab was restored from, if any. */
  sourceDraft?: {
    threadId: string
    messageId: string
    folderId: string
  }
  attachments: ComposerAttachment[]
}

/** An open reader tab for a single message (alongside the default conversation view).
 * The body is snapshotted at open time so the tab survives switching threads (which
 * reloads `mail$.messages`). */
export type MessageTab = {
  id: string
  kind: 'reader' | 'compose' | 'thread'
  messageId: string
  threadId: string
  /** Present on thread tabs so they can render outside the currently selected mailbox. */
  accountId?: string
  /** Present on thread tabs so message loading does not depend on side navigation. */
  folderId?: string
  subject: string
  from: string
  /** Raw correspondent header strings ("Name <addr>", comma-separated), snapshotted
   * at open time so the reader tab can show the full From/To/Cc/Reply-To list.
   * Present only on reader tabs. */
  fromRaw?: string
  to?: string
  cc?: string
  bcc?: string
  replyTo?: string
  /** Original message date as Unix epoch seconds, shown in the reader tab header. */
  date?: number
  body: string
  bodyHtml?: string
  viewMode: 'html' | 'plain'
  /** Present only when kind === "compose". */
  compose?: ComposeDraft
}

export type SystemCheck = {
  platform: string
  mail_engine: 'meron_mail'
  meron_mail: {
    configured: boolean
    available: boolean
    server_path: string
  }
  gmail_oauth_configured: boolean
  outlook_oauth_configured: boolean
  database_path: string
  log_path?: string
}
