package jp.nonbili.meron.ui

import androidx.compose.ui.graphics.ImageBitmap
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.ThreadReadPage
import jp.nonbili.meron.shared.ThreadSummary

internal enum class Screen { Mail, Kanban, Tasks, Calendar, Thread, Compose, AddAccount, Settings }

internal object AppRoutes {
    const val Mail = "mail"
    const val Kanban = "kanban"
    const val Tasks = "tasks"
    const val Calendar = "calendar"
    const val Compose = "compose"
    const val AddAccount = "add-account"
    const val Settings = "settings"
}

// Thread has no route of its own: a conversation is drawn over the route it
// was opened from (see MeronMobileScreenContent).
internal fun Screen.route(): String =
    when (this) {
        Screen.Mail -> AppRoutes.Mail
        Screen.Kanban -> AppRoutes.Kanban
        Screen.Tasks -> AppRoutes.Tasks
        Screen.Calendar -> AppRoutes.Calendar
        Screen.Thread -> AppRoutes.Mail
        Screen.Compose -> AppRoutes.Compose
        Screen.AddAccount -> AppRoutes.AddAccount
        Screen.Settings -> AppRoutes.Settings
    }

internal fun appRouteToScreen(route: String?): Screen? =
    when (route?.substringBefore("?")) {
        AppRoutes.Mail -> Screen.Mail
        AppRoutes.Kanban -> Screen.Kanban
        AppRoutes.Tasks -> Screen.Tasks
        AppRoutes.Calendar -> Screen.Calendar
        AppRoutes.Compose -> Screen.Compose
        AppRoutes.AddAccount -> Screen.AddAccount
        AppRoutes.Settings -> Screen.Settings
        else -> null
    }

internal data class AccountMediaUploadTarget(
    val account: AccountSummary,
    val wallpaper: Boolean,
)

internal data class KanbanBoardMediaTarget(
    val board: KanbanBoardSpec,
    val wallpaper: Boolean,
)

/**
 * One message's remote-content state, as the conversation resolves it: the
 * account's "load remote images" toggle, the app-wide sender allowlist, and any
 * reveal the reader has made while the thread is open (see [RemoteContentPolicy]).
 */
internal data class MessageRemoteContent(
    /** Whether this message's remote content may load right now. */
    val allowRemote: Boolean,
    /** Bare sender address, or empty where trusting the sender is not offered:
     *  the user's own outgoing mail, or a message with no From address. */
    val senderAddress: String = "",
    /** Show this message's remote content, for as long as the thread is open. */
    val onReveal: () -> Unit = {},
    /** Add [senderAddress] to the app-wide allowlist. */
    val onAllowSender: () -> Unit = {},
)

/** A decoded image ready to preview, share, or save. Carries raw bytes so the
 *  share/save platform services can write the original file. */
internal data class ImagePreview(
    val title: String,
    val image: ImageBitmap,
    val bytes: ByteArray,
    val mimeType: String,
    val fileName: String,
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is ImagePreview) return false
        return title == other.title &&
            mimeType == other.mimeType &&
            fileName == other.fileName &&
            bytes.contentEquals(other.bytes)
    }

    override fun hashCode(): Int {
        var result = title.hashCode()
        result = 31 * result + mimeType.hashCode()
        result = 31 * result + fileName.hashCode()
        result = 31 * result + bytes.contentHashCode()
        return result
    }
}

/** Where a tapped notification lands. A blank [threadKey] — what a group
 *  summary carries — opens the folder without selecting a thread. */
data class NotificationThreadTarget(
    val accountId: String,
    val folder: String,
    val threadKey: String = "",
    val nonce: Long = currentTimeMillis(),
)

internal enum class FilterMode { All, Unread, Starred }

internal enum class SendShortcutMode { Enter, ModEnter }

/**
 * How a thread's messages are laid out.
 * [Chat]: left/right chat bubbles, every message expanded.
 * [Traditional]: full-width stacked messages, collapsed to a one-line summary
 * except the newest and the unread ones (the classic mail-client reading view).
 */
internal enum class ConversationLayout { Chat, Traditional }

internal const val UNIFIED_ACCOUNT_ID = "unified"
internal const val INBOX_FOLDER = "inbox"
internal const val STARRED_FOLDER = "starred"
internal const val KANBAN_PREFS = "kanban"
internal const val KANBAN_BOARDS_PREF = "kanban_boards_v1"
internal const val ACTIVE_KANBAN_BOARD_PREF = "active_kanban_board_id_v1"
internal const val KANBAN_FILTER_PREF = "kanban_filter_v1"
internal const val KANBAN_ATTACHMENTS_PREF = "kanban_attachments_only_v1"
internal const val KANBAN_SEARCH_PREF = "kanban_search_v1"
internal const val KANBAN_SEARCH_SCOPE_PREF = "kanban_search_scope_v1"
internal const val APP_PREFS = "meron_app_prefs"
internal const val APPEARANCE_MODE_PREF = "appearance_mode_v1"
internal const val THEME_FOLLOW_SYSTEM_PREF = "theme_follow_system_v1"
internal const val LIGHT_THEME_PREF = "light_theme_v1"
internal const val DARK_THEME_PREF = "dark_theme_v1"
internal const val CUSTOM_THEMES_PREF = "custom_themes_v1"
internal const val SHOW_UNREAD_BADGES_PREF = "show_unread_badges_v1"
internal const val SHOW_UNIFIED_INBOX_PREF = "show_unified_inbox_v1"

/** Whether the optional Tasks screen is offered. Off until the user asks for it. */
internal const val TASKS_ENABLED_PREF = "tasks_enabled_v1"

/** Whether the optional Calendar screen is offered, like Tasks. */
internal const val CALENDAR_ENABLED_PREF = "calendar_enabled_v1"
internal const val SHOW_SENDER_IMAGES_PREF = "show_sender_images_v1"

/** Whether HTML mail bodies are darkened under a dark theme. Off by default:
 *  the sender's own colors are what the mail was designed in. */
internal const val DARK_MAIL_BODIES_PREF = "dark_mail_bodies_v1"

/** Whether conversation bubbles shrink over-wide HTML mail to fit, the way the
 *  full-screen reader always does. Off by default. */
internal const val AUTO_FIT_MESSAGES_PREF = "auto_fit_messages_v1"

/** Whether chat bubbles grow to fit long bodies instead of scrolling inside a
 *  capped box, so only the conversation scrolls. Off by default. */
internal const val CHAT_FULL_MESSAGES_PREF = "chat_full_messages_v1"

/** Whether the message reader puts reply, forward and delete in a bottom bar
 *  where they are easier to reach, instead of only in the top overflow menu. */
internal const val READER_BOTTOM_ACTIONS_PREF = "reader_bottom_actions_v1"

/** Whether replies carry the message they answer, quoted under the reply text.
 *  Off by default: replies read as chat messages. */
internal const val QUOTE_REPLIES_PREF = "quote_replies_v1"
internal const val NOTIFICATION_BANNER_DISMISSED_PREF = "notification_banner_dismissed_v1"
internal const val LIVE_MAIL_PUSH_PREF = "live_mail_push_v1"
internal const val BACKGROUND_SYNC_ENABLED_PREF = "background_sync_enabled_v1"
internal const val POLL_INTERVAL_MINUTES_PREF = "poll_interval_minutes_v1"
internal const val SEND_SHORTCUT_PREF = "send_shortcut_v1"
internal const val CONVERSATION_LAYOUT_PREF = "conversation_layout_v1"
internal const val MESSAGE_FONT_SCALE_PREF = "message_font_scale_v1"
internal const val APP_LANGUAGE_PREF = "app_language_v1"
internal const val HIDDEN_NAV_ACCOUNTS_PREF = "hidden_navigation_accounts_v1"
internal const val KANBAN_COLUMN_WIDTH_PREF = "kanban_column_width_v1"
internal const val LAST_TOP_SCREEN_PREF = "last_top_screen_v1"
internal const val LAST_MAIL_ACCOUNT_PREF = "last_mail_account_v1"
internal const val LAST_MAIL_FOLDER_PREF = "last_mail_folder_v1"
internal const val OAUTH_PENDING_PROVIDER_PREF = "oauth_pending_provider_v1"
internal const val OAUTH_PENDING_STATE_PREF = "oauth_pending_state_v1"
internal const val OAUTH_PENDING_VERIFIER_PREF = "oauth_pending_verifier_v1"
internal const val OAUTH_PENDING_REDIRECT_URI_PREF = "oauth_pending_redirect_uri_v1"
internal const val OAUTH_PENDING_EMAIL_PREF = "oauth_pending_email_v1"

internal data class PendingOAuthFlow(
    val provider: String,
    val state: String,
    val verifier: String,
    val redirectUri: String,
    val email: String,
)

internal const val KANBAN_COLUMN_MIN_WIDTH = 240
internal const val KANBAN_COLUMN_DEFAULT_WIDTH = 320
internal const val KANBAN_COLUMN_MAX_WIDTH = 520
internal const val KANBAN_COLUMN_WIDTH_STEP = 20

internal data class MailboxLoadResult(
    val folders: List<FolderSummary>,
    val folder: String,
    val threads: List<ThreadSummary>,
    val unreadCount: Int? = null,
    val nextCursor: String = "",
    val accountCursors: Map<String, String> = emptyMap(),
    val folderSynced: Boolean? = null,
    val searchIncomplete: Boolean = false,
    // Per-account header depth these threads were read at. Cached with the
    // mailbox so switching away and back keeps the pages the user paged in.
    // Loaders leave it at the default; the mailbox sets it when caching.
    val pageDepth: Int = MAILBOX_PAGE_SIZE,
)

internal data class MailboxCacheKey(
    val accountId: String,
    val folderId: String,
    val query: String,
    val filter: FilterMode,
    val attachmentsOnly: Boolean = false,
)

/** See `MeronMobileState.deferredMailboxReload`. `refreshSearch` is kept so a
 *  live search that stepped aside is re-run as one, not as the cache read that
 *  would only schedule it again. */
internal data class DeferredMailboxReload(
    val key: MailboxCacheKey,
    val refreshSearch: Boolean,
)

internal data class KanbanColumnSpec(
    val accountId: String,
    val folderId: String,
    /** Set on a saved search column: it always lists this folder's matches. */
    val query: String = "",
)

/** A Trash or Junk folder awaiting "empty folder" confirmation. [column] is set
 *  when the action came from a Kanban column, so that column is refreshed after. */
internal data class EmptyFolderTarget(
    val accountId: String,
    val folderId: String,
    val folderName: String,
    val role: String,
    val column: KanbanColumnSpec? = null,
)

/** A folder awaiting "delete folder" confirmation. [nested] is how many
 *  subfolders would go with it, which the dialog spells out. [column] is set when
 *  the action came from a Kanban column, so that column is dropped from the
 *  board. */
internal data class DeleteFolderTarget(
    val accountId: String,
    val folderId: String,
    val folderName: String,
    val nested: Int = 0,
    val column: KanbanColumnSpec? = null,
)

/** Where a folder made by a "new folder" action shows up once it exists. The
 *  board placements apply only while [boardId] is still the active board. */
internal sealed interface NewFolderPlacement {
    /** As a new column, from the add-column dialog. */
    data class AddColumn(
        val boardId: String,
    ) : NewFolderPlacement

    /** In place of [column], as picking it in the column's folder switcher would. */
    data class SwitchColumn(
        val boardId: String,
        val column: KanbanColumnSpec,
    ) : NewFolderPlacement

    /** In the mail view, unless it moved on from [folderId] while the server
     *  made the folder. */
    data class OpenMailbox(
        val folderId: String,
    ) : NewFolderPlacement
}

/** An account awaiting a new folder's name. */
internal data class NewFolderTarget(
    val account: AccountSummary,
    val placement: NewFolderPlacement,
)

/** A folder awaiting its new name. [folderName] is its own (leaf) name, the
 *  part the dialog edits; [parent] is the path above it, kept as it is. */
internal data class RenameFolderTarget(
    val accountId: String,
    val folderId: String,
    val folderName: String,
    val parent: String,
)

/** A conversation beside the open one, read ahead for swiping over to it.
 *  [stamp] is the summary's date when it was read: a newer one means new mail
 *  arrived since, and the preview is read again. */
internal data class ThreadPreview(
    val page: ThreadReadPage,
    val stamp: Long,
)

/** The list a conversation was opened from, which swiping it sideways pages through. */
internal sealed interface ThreadBrowseSource {
    data object MailList : ThreadBrowseSource

    data class KanbanColumn(
        val column: KanbanColumnSpec,
    ) : ThreadBrowseSource
}

internal data class KanbanBoardSpec(
    val id: String,
    val name: String,
    val columns: List<KanbanColumnSpec>,
    val avatarUrl: String = "",
    val wallpaperPresetId: String = "",
    val wallpaperUrl: String = "",
)

internal data class KanbanColumnState(
    val threads: List<ThreadSummary> = emptyList(),
    val unreadCount: Int? = null,
    val loading: Boolean = false,
    val loadingMore: Boolean = false,
    val error: String? = null,
    val nextCursor: String = "",
    val accountCursors: Map<String, String> = emptyMap(),
    // The search and filters the cursors were issued for: the next page must be
    // asked for with these, not whatever the board shows by then.
    val cursorView: KanbanColumnView = KanbanColumnView(),
    // The limit the listing has been read with so far, page by page. The core's
    // limit counts message headers, which group into fewer threads, so a reload
    // re-reads this many rather than the number of cards shown.
    val readDepth: Int = MAILBOX_PAGE_SIZE,
)

internal data class KanbanColumnView(
    val query: String = "",
    val filter: FilterMode = FilterMode.All,
    val attachmentsOnly: Boolean = false,
)

internal data class ConversationParticipant(
    val name: String,
    val email: String,
    val count: Int,
    val isSelf: Boolean,
)
