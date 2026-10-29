package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.TextAutoSize
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.automirrored.outlined.StickyNote2
import androidx.compose.material.icons.filled.AllInbox
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.Outbox
import androidx.compose.material.icons.filled.Report
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.ViewKanban
import androidx.compose.material.icons.outlined.Drafts
import androidx.compose.material.icons.outlined.FolderOpen
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
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
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.SendIdentity
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.accountSummaryIsRss
import kotlin.math.abs
import kotlin.uuid.ExperimentalUuidApi
import kotlin.uuid.Uuid

// Gradient pairs mirror the desktop avatar palette in
// desktop/frontend/src/components/avatar/Avatar.tsx (Tailwind 400 -> 500 shades).
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

/**
 * Icon for a folder: its RFC 6154 special-use role when the server advertises
 * one, otherwise a guess from the folder's own name.
 */
internal fun folderIcon(folder: FolderSummary): ImageVector =
    when (folder.role.lowercase()) {
        "inbox" -> Icons.Filled.Inbox

        "sent" -> Icons.AutoMirrored.Filled.Send

        "drafts" -> Icons.Outlined.Drafts

        "trash" -> Icons.Filled.Delete

        "junk" -> Icons.Filled.Report

        "archive" -> Icons.Filled.Archive

        "all" -> Icons.Filled.AllInbox

        // Hierarchical names arrive as full paths ("INBOX.Sent"); only the leaf
        // carries the meaning.
        else -> folderIcon(folder.displayName.split('/', '.').last())
    }

/**
 * Icon guessed from a folder name alone, for servers that don't advertise
 * special-use. Kept to exact well-known names: a user folder called "Archives"
 * or "Notebook" stays a plain folder.
 */
internal fun folderIcon(name: String): ImageVector =
    when (name.trim().lowercase()) {
        "inbox" -> Icons.Filled.Inbox
        "sent", "sent mail", "sent items", "sent messages" -> Icons.AutoMirrored.Filled.Send
        "drafts", "draft" -> Icons.Outlined.Drafts
        "outbox" -> Icons.Filled.Outbox
        "archive" -> Icons.Filled.Archive
        "all mail" -> Icons.Filled.AllInbox
        "trash", "bin", "deleted", "deleted items", "deleted messages" -> Icons.Filled.Delete
        "junk", "spam", "junk e-mail", "junk email", "bulk mail" -> Icons.Filled.Report
        "notes" -> Icons.AutoMirrored.Outlined.StickyNote2
        "starred", "flagged" -> Icons.Filled.Star
        else -> Icons.Outlined.FolderOpen
    }

// A saved search is its own column beside the folder's plain one, so its query is
// part of the key. Splitting a key on "\n" still yields account and folder first.
internal fun kanbanColumnKey(column: KanbanColumnSpec): String {
    val base = "${column.accountId}\n${column.folderId}"
    return if (column.query.isBlank()) base else "$base\n${column.query}"
}

/** The account and folder a column key names, whatever query follows them. */
internal fun kanbanKeyAccountFolder(key: String): Pair<String, String> {
    val parts = key.split("\n")
    return parts[0] to parts.getOrElse(1) { "" }
}

/** A saved search query as a column stores it: one line, trimmed. The column key
 *  joins its parts with newlines, so a query must not carry one. */
internal fun normalizeKanbanColumnQuery(query: String): String = query.replace(Regex("\\s+"), " ").trim()

/** The board search inside a saved search column narrows it. The core reads each
 *  side of an `AND` as its own query that must also match, so free text, dates
 *  and read state narrow the saved query rather than merging into or replacing
 *  its own. The search starts a line of its own, which no quote or group left
 *  open in the saved query reaches into. A leading `OR` would be a stray word
 *  there, so it is dropped. */
internal fun narrowKanbanQuery(
    saved: String,
    query: String,
): String {
    if (saved.isBlank()) return query.trim()
    val extra = query.trim().replace(Regex("^(?:OR(?:\\s+|$))+"), "")
    return if (extra.isBlank()) saved else "$saved\nAND $extra"
}

internal fun identityKey(identity: SendIdentity): String = "${identity.accountId}|${identity.email}"

@Composable
internal fun FilterMode.label(): String =
    when (this) {
        FilterMode.All -> tr("filters.all")
        FilterMode.Unread -> tr("filters.unread")
        FilterMode.Starred -> tr("filters.starred")
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

/** The cards a Kanban column shows: its loaded threads, narrowed by the board's
 *  filter and by its search where that reaches this column. */
internal fun kanbanColumnVisibleThreads(
    column: KanbanColumnSpec,
    threads: List<ThreadSummary>,
    filter: FilterMode,
    search: String,
    searchScope: String,
    attachmentsOnly: Boolean,
): List<ThreadSummary> {
    val boardSearch = if (searchScope.ifBlank { "all" } == "all" || searchScope == kanbanColumnKey(column)) search else ""
    val columnSearch = narrowKanbanQuery(column.query, boardSearch)
    return threads.filteredKanbanThreads(
        filter = filter,
        search = columnSearch,
        searchAlreadyApplied = columnSearch.isNotBlank(),
        attachmentsOnly = attachmentsOnly,
    )
}

internal fun List<ThreadSummary>.filteredKanbanThreads(
    filter: FilterMode,
    search: String,
    searchAlreadyApplied: Boolean = false,
    attachmentsOnly: Boolean = false,
): List<ThreadSummary> {
    val query = search.trim().lowercase()
    return filter { thread ->
        val filterOk =
            when (filter) {
                FilterMode.All -> true
                FilterMode.Unread -> thread.unread
                FilterMode.Starred -> thread.starred || thread.hasStarredItems
            } && (!attachmentsOnly || thread.hasAttachments)
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

/**
 * Find a just-created folder in a fresh LIST. The user types UTF-8, so the match
 * runs against the decoded label, while callers address the mailbox by the wire
 * name (modified UTF-7 on servers without UTF8=ACCEPT). `displayName` falls back
 * to `name`, so ASCII folders match either way.
 */
internal fun List<FolderSummary>.folderCreatedAs(typedName: String): FolderSummary? = firstOrNull { it.displayName.equals(typedName, ignoreCase = true) }

/** IMAP requires INBOX to be case-insensitive; other mailbox names retain their casing. */
internal fun kanbanFolderIdsEqual(
    first: String,
    second: String,
): Boolean =
    if (first.equals(INBOX_FOLDER, ignoreCase = true) && second.equals(INBOX_FOLDER, ignoreCase = true)) {
        true
    } else {
        first == second
    }

internal fun loadedUnreadCount(threads: List<ThreadSummary>): Int = threads.sumOf { if (it.unread) it.unreadCount.coerceAtLeast(1) else 0 }

internal fun kanbanColumnUnreadCount(
    column: KanbanColumnSpec,
    folderUnread: Int?,
    loadedThreads: List<ThreadSummary> = emptyList(),
): Int {
    // The folder's total says nothing about how many of a saved search's matches are unread.
    if (isUnifiedStarredColumn(column) || column.query.isNotBlank()) return loadedUnreadCount(loadedThreads)
    return folderUnread ?: loadedUnreadCount(loadedThreads)
}

@Composable
internal fun columnTitle(
    column: KanbanColumnSpec,
    accounts: List<AccountSummary>,
    foldersByAccount: Map<String, List<FolderSummary>>,
): String {
    // A column names itself: the mail list can say "Sent" because the unified
    // mailbox is the whole screen, but a board mixes unified and per-account
    // columns whose folders share those names.
    if (column.accountId == UNIFIED_ACCOUNT_ID) return unifiedColumnLabel(column.folderId)
    return folderTitle(column.accountId, column.folderId, accounts, foldersByAccount)
}

/** A column as the search scope names it: a saved search shares its folder's
 *  title, so its query tells the two apart. */
@Composable
internal fun searchScopeTitle(
    column: KanbanColumnSpec,
    accounts: List<AccountSummary>,
    foldersByAccount: Map<String, List<FolderSummary>>,
): String {
    val title = columnTitle(column, accounts, foldersByAccount)
    return if (column.query.isBlank()) title else "$title · ${column.query}"
}

/** The display name of a mailbox, as a kanban column or the mail list shows it. */
@Composable
internal fun folderTitle(
    accountId: String,
    folderId: String,
    accounts: List<AccountSummary>,
    foldersByAccount: Map<String, List<FolderSummary>>,
): String {
    if (accountId == UNIFIED_ACCOUNT_ID) {
        if (folderId.equals(STARRED_FOLDER, ignoreCase = true)) return tr("kanban.columns.unifiedStarred")
        // The unified view now switches folders, so the header names the folder
        // ("Sent", "Archive") the way a single account's does.
        return unifiedFolderLabel(folderId)
    }
    val account = accounts.firstOrNull { it.id == accountId }
    val folder =
        foldersByAccount[accountId]
            ?.firstOrNull { it.name.equals(folderId, ignoreCase = true) }
            ?.name
            ?: folderId
    val folderLabel =
        if (folder.equals(INBOX_FOLDER, ignoreCase = true)) {
            tr(if (account?.let(::accountSummaryIsRss) == true) "mobile.feedTitle" else "folders.roles.inbox")
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

@OptIn(ExperimentalUuidApi::class)
internal fun defaultKanbanBoard(
    accounts: List<AccountSummary>,
    name: String = "Kanban board",
): KanbanBoardSpec {
    val columns = mutableListOf(KanbanColumnSpec(UNIFIED_ACCOUNT_ID, INBOX_FOLDER))
    accounts.forEach { account -> columns += KanbanColumnSpec(account.id, INBOX_FOLDER) }
    return KanbanBoardSpec(
        id = "kb-${Uuid.random()}",
        name = name,
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

@Composable
@ReadOnlyComposable
internal fun ConversationLayout.label(): String =
    when (this) {
        ConversationLayout.Chat -> tr("settings.appearance.conversationLayoutChat")
        ConversationLayout.Traditional -> tr("settings.appearance.conversationLayoutTraditional")
    }

internal fun ConversationLayout.storageValue(): String =
    when (this) {
        ConversationLayout.Chat -> "chat"
        ConversationLayout.Traditional -> "traditional"
    }

internal fun ConversationLayout.next(): ConversationLayout =
    when (this) {
        ConversationLayout.Chat -> ConversationLayout.Traditional
        ConversationLayout.Traditional -> ConversationLayout.Chat
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

internal fun formatBytes(bytes: Long): String =
    when {
        bytes >= 1_000_000 -> "${bytes / 1_000_000} MB"
        bytes >= 1_000 -> "${bytes / 1_000} KB"
        else -> "$bytes B"
    }

// Inline MIME parts can have no filename extension even though the cached
// media key has one. Keep it when saving, sharing, or opening the attachment.
internal fun safeAttachmentFilename(attachment: MessageAttachment): String {
    val name = safeAttachmentFilename(attachment.filename)
    val mediaMime =
        attachment.mimeType
            .substringBefore(';')
            .trim()
            .lowercase()
    if (!mediaMime.startsWith("image/") && !mediaMime.startsWith("video/")) return name
    val extension = Regex("\\.[a-z0-9]+$", RegexOption.IGNORE_CASE)
    val imageExtension = Regex("\\.(?:a?png|jpe?g|jpe|jfif|gif|webp|bmp|tiff?|heic|heif|avif|svg|ico|jxl|jp2|jpx)$", RegexOption.IGNORE_CASE)
    val cachedSuffix = extension.find(attachment.key)?.value.orEmpty()
    val mimeSuffix =
        when (
            attachment.mimeType
                .substringBefore(';')
                .trim()
                .lowercase()
        ) {
            "image/png" -> ".png"
            "image/jpeg" -> ".jpg"
            "image/gif" -> ".gif"
            "image/webp" -> ".webp"
            "image/bmp", "image/x-ms-bmp" -> ".bmp"
            "image/tiff" -> ".tiff"
            "image/heic" -> ".heic"
            "image/heif" -> ".heif"
            "image/avif" -> ".avif"
            "image/svg+xml" -> ".svg"
            "image/x-icon", "image/vnd.microsoft.icon" -> ".ico"
            else -> ""
        }
    val suffix = cachedSuffix.takeIf { imageExtension.matches(it) } ?: mimeSuffix.ifBlank { cachedSuffix }
    val hasExtension =
        if (imageExtension.matches(suffix)) imageExtension.containsMatchIn(name) else extension.containsMatchIn(name)
    return if (hasExtension) name else name + suffix
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
                // A label longer than its third of the menu, like Japanese スター付き,
                // shrinks to one line instead of wrapping and growing the whole row.
                contentPadding = PaddingValues(horizontal = 4.dp),
                label = {
                    Text(
                        mode.label(),
                        maxLines = 1,
                        softWrap = false,
                        autoSize = TextAutoSize.StepBased(minFontSize = 10.sp, maxFontSize = LocalTextStyle.current.fontSize),
                    )
                },
            )
        }
    }
}

// The attachments toggle under the filter control: an on/off narrowing of
// whichever read-state filter is chosen above, not a fourth choice beside them.
@Composable
internal fun AttachmentsOnlyMenuItem(
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    DropdownMenuItem(
        text = { Text(tr("chat.attachments")) },
        leadingIcon = { Icon(Icons.Filled.AttachFile, contentDescription = null) },
        trailingIcon = {
            if (checked) Icon(Icons.Filled.Check, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
        },
        onClick = { onCheckedChange(!checked) },
    )
}

// Unified starred contains starred cards only, including after optimistic flag changes.
internal fun List<ThreadSummary>.forStarredView(
    accountId: String,
    folderId: String,
): List<ThreadSummary> = if (accountId == UNIFIED_ACCOUNT_ID && isUnifiedStarredFolder(folderId)) filter { it.starred } else this
