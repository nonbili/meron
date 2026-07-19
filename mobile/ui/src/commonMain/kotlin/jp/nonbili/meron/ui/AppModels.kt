package jp.nonbili.meron.ui

import androidx.compose.ui.graphics.ImageBitmap
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.ThreadSummary

internal enum class Screen { Mail, Starred, Kanban, Thread, Compose, AddAccount, Settings }

internal object AppRoutes {
    const val Mail = "mail"
    const val Starred = "starred"
    const val Kanban = "kanban"
    const val Thread = "thread"
    const val Compose = "compose"
    const val AddAccount = "add-account"
    const val Settings = "settings"
}

internal fun Screen.route(): String =
    when (this) {
        Screen.Mail -> AppRoutes.Mail
        Screen.Starred -> AppRoutes.Starred
        Screen.Kanban -> AppRoutes.Kanban
        Screen.Thread -> AppRoutes.Thread
        Screen.Compose -> AppRoutes.Compose
        Screen.AddAccount -> AppRoutes.AddAccount
        Screen.Settings -> AppRoutes.Settings
    }

internal fun appRouteToScreen(route: String?): Screen? =
    when (route?.substringBefore("?")) {
        AppRoutes.Mail -> Screen.Mail
        AppRoutes.Starred -> Screen.Starred
        AppRoutes.Kanban -> Screen.Kanban
        AppRoutes.Thread -> Screen.Thread
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

data class NotificationThreadTarget(
    val accountId: String,
    val folder: String,
    val threadKey: String,
    val nonce: Long = currentTimeMillis(),
)

internal enum class FilterMode { All, Unread, Starred }

internal enum class SendShortcutMode { Enter, ModEnter }

internal const val UNIFIED_ACCOUNT_ID = "unified"
internal const val INBOX_FOLDER = "inbox"
internal const val STARRED_FOLDER = "starred"
internal const val KANBAN_PREFS = "kanban"
internal const val KANBAN_BOARDS_PREF = "kanban_boards_v1"
internal const val ACTIVE_KANBAN_BOARD_PREF = "active_kanban_board_id_v1"
internal const val KANBAN_FILTER_PREF = "kanban_filter_v1"
internal const val KANBAN_SEARCH_PREF = "kanban_search_v1"
internal const val KANBAN_SEARCH_SCOPE_PREF = "kanban_search_scope_v1"
internal const val APP_PREFS = "meron_app_prefs"
internal const val APPEARANCE_MODE_PREF = "appearance_mode_v1"
internal const val SHOW_UNREAD_BADGES_PREF = "show_unread_badges_v1"
internal const val SHOW_UNIFIED_INBOX_PREF = "show_unified_inbox_v1"
internal const val SHOW_STARRED_NAV_PREF = "show_starred_nav_v1"
internal const val SHOW_SENDER_IMAGES_PREF = "show_sender_images_v1"
internal const val NOTIFICATION_BANNER_DISMISSED_PREF = "notification_banner_dismissed_v1"
internal const val LIVE_MAIL_PUSH_PREF = "live_mail_push_v1"
internal const val BACKGROUND_SYNC_ENABLED_PREF = "background_sync_enabled_v1"
internal const val POLL_INTERVAL_MINUTES_PREF = "poll_interval_minutes_v1"
internal const val SEND_SHORTCUT_PREF = "send_shortcut_v1"
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
)

internal data class MailboxCacheKey(
    val accountId: String,
    val folderId: String,
    val query: String,
    val filter: FilterMode,
)

internal data class KanbanColumnSpec(
    val accountId: String,
    val folderId: String,
)

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
)

internal data class ConversationParticipant(
    val name: String,
    val email: String,
    val count: Int,
    val isSelf: Boolean,
)
