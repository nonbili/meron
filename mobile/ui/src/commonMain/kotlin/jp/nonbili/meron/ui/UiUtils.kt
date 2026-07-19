package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.ViewKanban
import androidx.compose.material.icons.outlined.Drafts
import androidx.compose.material.icons.outlined.FolderOpen
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.type
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.SendIdentity
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.accountSummaryIsRss
import kotlin.math.abs
import kotlin.uuid.ExperimentalUuidApi
import kotlin.uuid.Uuid

// Gradient pairs mirror the desktop avatar palette in
// frontend/src/components/avatar/Avatar.tsx (Tailwind 400 -> 500 shades).
internal val avatarGradients =
    listOf(
        Color(0xFF818CF8) to Color(0xFF6366F1), // indigo
        Color(0xFFA78BFA) to Color(0xFF8B5CF6), // violet
        Color(0xFF2DD4BF) to Color(0xFF14B8A6), // teal
        Color(0xFF34D399) to Color(0xFF10B981), // emerald
        Color(0xFFFB7185) to Color(0xFFF43F5E), // rose
        Color(0xFFFBBF24) to Color(0xFFF59E0B), // amber
        Color(0xFF38BDF8) to Color(0xFF0EA5E9), // sky
        Color(0xFFE879F9) to Color(0xFFD946EF), // fuchsia
    )

internal fun avatarBrush(name: String): Brush {
    val key = name.ifBlank { "?" }
    var hash = 0
    for (ch in key) hash = ch.code + ((hash shl 5) - hash)
    val (start, end) = avatarGradients[abs(hash) % avatarGradients.size]
    return Brush.linearGradient(listOf(start, end))
}

internal fun avatarInitials(value: String): String {
    val parts = value.split(' ', '\t', '\n', '@').filter { it.isNotBlank() }
    if (parts.isEmpty()) return "?"
    return parts.take(2).mapNotNull { it.firstOrNull()?.uppercaseChar() }.joinToString("")
}

internal fun isAuthError(message: String): Boolean {
    val m = message.lowercase()
    return listOf("auth", "login", "credential", "password", "unauthor", "permission", "token", "401", "535")
        .any { m.contains(it) }
}

internal fun isOutgoing(
    message: MessageBody,
    accountEmail: String,
): Boolean {
    // The core's classification (own address or Sent-folder provenance) covers
    // alias-sent mail; the address match below remains for rows shaped before
    // the flag existed.
    if (message.outgoing) return true
    if (accountEmail.isBlank()) return false
    val acct = accountEmail.trim().lowercase()
    val from =
        message.fromAddr
            .ifBlank { message.from }
            .trim()
            .lowercase()
    return from.contains(acct)
}

internal fun folderIcon(name: String): ImageVector =
    when (name.lowercase()) {
        "inbox" -> Icons.Filled.Inbox
        "sent" -> Icons.AutoMirrored.Filled.Send
        "drafts" -> Icons.Outlined.Drafts
        "archive" -> Icons.Filled.Archive
        "trash", "deleted" -> Icons.Filled.Delete
        "starred" -> Icons.Filled.Star
        else -> Icons.Outlined.FolderOpen
    }

internal fun kanbanColumnKey(column: KanbanColumnSpec): String = "${column.accountId}\n${column.folderId}"

internal fun identityKey(identity: SendIdentity): String = "${identity.accountId}|${identity.email}"

internal fun FilterMode.label(): String =
    when (this) {
        FilterMode.All -> "All"
        FilterMode.Unread -> "Unread"
        FilterMode.Starred -> "Starred"
    }

internal fun FilterMode.protocolValue(): String =
    when (this) {
        FilterMode.All -> "all"
        FilterMode.Unread -> "unread"
        FilterMode.Starred -> "starred"
    }

internal fun FilterMode.emptyNoun(): String =
    when (this) {
        FilterMode.All -> "cards"
        FilterMode.Unread -> "unread cards"
        FilterMode.Starred -> "starred cards"
    }

internal fun List<ThreadSummary>.filteredKanbanThreads(
    filter: FilterMode,
    search: String,
    searchAlreadyApplied: Boolean = false,
): List<ThreadSummary> {
    val query = search.trim().lowercase()
    return filter { thread ->
        val filterOk =
            when (filter) {
                FilterMode.All -> true
                FilterMode.Unread -> thread.unread
                FilterMode.Starred -> thread.starred
            }
        val queryOk =
            searchAlreadyApplied ||
                query.isBlank() ||
                thread.subject.lowercase().contains(query) ||
                thread.sender.lowercase().contains(query) ||
                thread.preview.lowercase().contains(query) ||
                thread.accountId.lowercase().contains(query)
        filterOk && queryOk
    }.sortedByDescending { it.dateEpochSeconds }
}

internal fun folderUnread(
    folders: List<FolderSummary>?,
    folderId: String,
): Int {
    if (folders.isNullOrEmpty()) return 0
    return folders
        .firstOrNull { folder ->
            if (folderId.equals(INBOX_FOLDER, ignoreCase = true)) {
                folder.name.equals(INBOX_FOLDER, ignoreCase = true)
            } else {
                folder.name == folderId
            }
        }?.unread ?: 0
}

internal fun loadedUnreadCount(threads: List<ThreadSummary>): Int = threads.sumOf { if (it.unread) it.unreadCount.coerceAtLeast(1) else 0 }

internal fun kanbanColumnUnreadCount(
    column: KanbanColumnSpec,
    folderUnread: Int?,
    loadedThreads: List<ThreadSummary> = emptyList(),
): Int {
    if (isUnifiedStarredColumn(column)) return loadedUnreadCount(loadedThreads)
    return folderUnread ?: loadedUnreadCount(loadedThreads)
}

internal fun columnTitle(
    column: KanbanColumnSpec,
    accounts: List<AccountSummary>,
    foldersByAccount: Map<String, List<FolderSummary>>,
): String {
    if (column.accountId == UNIFIED_ACCOUNT_ID) {
        return if (column.folderId.equals(STARRED_FOLDER, ignoreCase = true)) "Unified starred" else "Unified inbox"
    }
    val account = accounts.firstOrNull { it.id == column.accountId }
    val folder =
        foldersByAccount[column.accountId]
            ?.firstOrNull { it.name.equals(column.folderId, ignoreCase = true) }
            ?.name
            ?: column.folderId
    val folderLabel =
        if (folder.equals(INBOX_FOLDER, ignoreCase = true)) {
            if (account?.let(::accountSummaryIsRss) == true) "Feed" else "Inbox"
        } else {
            folder
        }
    return folderLabel
}

internal fun KanbanBoardSpec.hasBoardStyle(): Boolean = avatarUrl.isNotBlank() || wallpaperPresetId.isNotBlank() || wallpaperUrl.isNotBlank()

@Composable
internal fun KanbanBoardTile(
    board: KanbanBoardSpec?,
    size: Dp,
) {
    val styled = board?.hasBoardStyle() == true
    val tileBrush =
        if (styled) {
            avatarBrush(board.avatarUrl.ifBlank { board.name })
        } else {
            Brush.linearGradient(listOf(MaterialTheme.colorScheme.primaryContainer, MaterialTheme.colorScheme.primaryContainer))
        }
    Box(
        Modifier
            .size(size)
            .clip(RoundedCornerShape(8.dp))
            .background(tileBrush),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            Icons.Filled.ViewKanban,
            contentDescription = null,
            tint = if (styled) Color.White else MaterialTheme.colorScheme.onPrimaryContainer,
            modifier = Modifier.size(size * 0.55f),
        )
    }
}

@Composable
internal fun boardBackgroundBrush(board: KanbanBoardSpec?): Brush? {
    if (board == null || !board.hasBoardStyle()) return null
    return Brush.verticalGradient(
        listOf(
            MaterialTheme.colorScheme.background,
            MaterialTheme.colorScheme.surfaceContainerHighest.copy(alpha = 0.92f),
            MaterialTheme.colorScheme.background,
        ),
    )
}

@OptIn(ExperimentalUuidApi::class)
internal fun defaultKanbanBoard(accounts: List<AccountSummary>): KanbanBoardSpec {
    val columns = mutableListOf(KanbanColumnSpec(UNIFIED_ACCOUNT_ID, INBOX_FOLDER))
    accounts.forEach { account -> columns += KanbanColumnSpec(account.id, INBOX_FOLDER) }
    return KanbanBoardSpec(
        id = "kb-${Uuid.random()}",
        name = "Kanban board",
        columns = columns.distinctBy(::kanbanColumnKey),
    )
}

internal fun SendShortcutMode.label(): String =
    when (this) {
        SendShortcutMode.Enter -> "Enter"
        SendShortcutMode.ModEnter -> "Ctrl+Enter"
    }

internal fun SendShortcutMode.storageValue(): String =
    when (this) {
        SendShortcutMode.Enter -> "enter"
        SendShortcutMode.ModEnter -> "mod_enter"
    }

internal fun SendShortcutMode.next(): SendShortcutMode =
    when (this) {
        SendShortcutMode.Enter -> SendShortcutMode.ModEnter
        SendShortcutMode.ModEnter -> SendShortcutMode.Enter
    }

internal fun shouldSendFromEditor(
    event: KeyEvent,
    mode: SendShortcutMode,
): Boolean {
    if (event.type != KeyEventType.KeyDown || event.key != Key.Enter) return false
    return when (mode) {
        SendShortcutMode.Enter -> !event.isShiftPressed && !event.isCtrlPressed && !event.isMetaPressed
        SendShortcutMode.ModEnter -> !event.isShiftPressed && (event.isCtrlPressed || event.isMetaPressed)
    }
}

internal fun nextKanbanColumnWidth(current: Int): Int {
    val next = current + KANBAN_COLUMN_WIDTH_STEP
    return if (next > KANBAN_COLUMN_MAX_WIDTH) KANBAN_COLUMN_MIN_WIDTH else next
}

internal val supportedAppLanguageTags =
    listOf(
        "en",
        "ar",
        "zh-Hans",
        "zh-Hant",
        "de",
        "el",
        "es",
        "et",
        "fr",
        "it",
        "ja",
        "ko",
        "lv",
        "pl",
        "pt",
        "pt-BR",
        "sv",
        "tr",
        "vi",
    )

private val languageEndonyms =
    mapOf(
        "en" to "English",
        "ar" to "العربية",
        "zh-Hans" to "简体中文",
        "zh-Hant" to "繁體中文",
        "de" to "Deutsch",
        "el" to "Ελληνικά",
        "es" to "Español",
        "et" to "Eesti",
        "fr" to "Français",
        "it" to "Italiano",
        "ja" to "日本語",
        "ko" to "한국어",
        "lv" to "Latviešu",
        "pl" to "Polski",
        "pt" to "Português",
        "pt-BR" to "Português (Brasil)",
        "sv" to "Svenska",
        "tr" to "Türkçe",
        "vi" to "Tiếng Việt",
    )

internal fun appLanguageDisplayName(tag: String): String = languageEndonyms[tag] ?: tag

internal fun onOffLabel(enabled: Boolean): String = if (enabled) "On" else "Off"

internal fun AppAppearanceMode.next(): AppAppearanceMode {
    val values = AppAppearanceMode.entries.filterNot { it == AppAppearanceMode.System }
    val index = values.indexOf(this).takeIf { it >= 0 } ?: 0
    return values[(index + 1) % values.size]
}

internal fun formatBytes(bytes: Long): String =
    when {
        bytes >= 1_000_000 -> "${bytes / 1_000_000} MB"
        bytes >= 1_000 -> "${bytes / 1_000} KB"
        else -> "$bytes B"
    }

internal fun safeAttachmentFilename(name: String): String {
    val cleaned =
        name
            .ifBlank { "attachment" }
            .map { ch -> if (ch.isLetterOrDigit() || ch in listOf('.', '-', '_')) ch else '_' }
            .joinToString("")
            .trim('.', '_')
    return cleaned.ifBlank { "attachment" }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun FilterModeSegmentedControl(
    filter: FilterMode,
    onFilterChange: (FilterMode) -> Unit,
    modifier: Modifier = Modifier,
) {
    SingleChoiceSegmentedButtonRow(
        modifier = modifier,
    ) {
        FilterMode.values().forEachIndexed { index, mode ->
            SegmentedButton(
                selected = filter == mode,
                onClick = { onFilterChange(mode) },
                shape =
                    SegmentedButtonDefaults.itemShape(
                        index = index,
                        count = FilterMode.values().size,
                    ),
                icon = {},
                label = { Text(mode.label()) },
            )
        }
    }
}
