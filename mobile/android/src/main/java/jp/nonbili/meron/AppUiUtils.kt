package jp.nonbili.meron

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Base64
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Reply
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.FilterList
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.MarkEmailUnread
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.OpenInFull
import androidx.compose.material.icons.filled.PersonAdd
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.RssFeed
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.StarBorder
import androidx.compose.material.icons.filled.ViewKanban
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material.icons.outlined.Drafts
import androidx.compose.material.icons.outlined.FolderOpen
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.ListItem
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.rememberDrawerState
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.FileProvider
import jp.nonbili.meron.shared.AccountAliasParams
import jp.nonbili.meron.shared.AccountAliasesParams
import jp.nonbili.meron.shared.AccountAvatarParams
import jp.nonbili.meron.shared.AccountChatWallpaperParams
import jp.nonbili.meron.shared.AccountFlagParams
import jp.nonbili.meron.shared.AccountIdParams
import jp.nonbili.meron.shared.AccountMediaFileParams
import jp.nonbili.meron.shared.AccountNameParams
import jp.nonbili.meron.shared.AccountReorderParams
import jp.nonbili.meron.shared.AccountRssSyncIntervalParams
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.AttachmentReadParams
import jp.nonbili.meron.shared.AutodiscoverAccountParams
import jp.nonbili.meron.shared.AddPasswordAccountParams
import jp.nonbili.meron.shared.AddOAuthAccountParams
import jp.nonbili.meron.shared.AddRssAccountParams
import jp.nonbili.meron.shared.AddRssFeedParams
import jp.nonbili.meron.shared.ComposeDraft
import jp.nonbili.meron.shared.ContactSuggestion
import jp.nonbili.meron.shared.ContactSuggestParams
import jp.nonbili.meron.shared.CopyThreadParams
import jp.nonbili.meron.shared.DiscardDraftParams
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.ExchangeOAuthCodeParams
import jp.nonbili.meron.shared.ExportOpmlParams
import jp.nonbili.meron.shared.FolderCreateParams
import jp.nonbili.meron.shared.FolderListParams
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.ImportOpmlParams
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.OAuthAuthorizationRequest
import jp.nonbili.meron.shared.MarkAllReadParams
import jp.nonbili.meron.shared.MarkReadParams
import jp.nonbili.meron.shared.MarkStarredParams
import jp.nonbili.meron.shared.MoveRssFeedParams
import jp.nonbili.meron.shared.MoveThreadParams
import jp.nonbili.meron.shared.RemoveRssFeedParams
import jp.nonbili.meron.shared.RssMarkReadParams
import jp.nonbili.meron.shared.RssMarkStarredParams
import jp.nonbili.meron.shared.SendIdentity
import jp.nonbili.meron.shared.RssThreadParams
import jp.nonbili.meron.shared.SharedMobileContract
import jp.nonbili.meron.shared.StarredItemSummary
import jp.nonbili.meron.shared.StorageUsage
import jp.nonbili.meron.shared.SyncMailParams
import jp.nonbili.meron.shared.SyncRssParams
import jp.nonbili.meron.shared.ThreadActionParams
import jp.nonbili.meron.shared.ThreadListParams
import jp.nonbili.meron.shared.ThreadReadParams
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.accountSendIdentities
import jp.nonbili.meron.shared.attachmentToDraftAttachment
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.folderIsTrash
import jp.nonbili.meron.shared.forwardableAttachments
import jp.nonbili.meron.shared.formatContactSuggestion
import jp.nonbili.meron.shared.formatSendIdentity
import jp.nonbili.meron.shared.messageEditAsNewDraft
import jp.nonbili.meron.shared.messageForwardDraft
import jp.nonbili.meron.shared.newDraftMessageId
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseAutodiscoverResponse
import jp.nonbili.meron.shared.parseAttachmentDataResponse
import jp.nonbili.meron.shared.parseContactSuggestResponse
import jp.nonbili.meron.shared.parseFolderListResponse
import jp.nonbili.meron.shared.parseMediaFileUrlResponse
import jp.nonbili.meron.shared.parseOpmlExportResponse
import jp.nonbili.meron.shared.parseOpmlImportCountResponse
import jp.nonbili.meron.shared.parseStarredItemsResponse
import jp.nonbili.meron.shared.parseStorageUsageResponse
import jp.nonbili.meron.shared.parseThreadReadPage
import jp.nonbili.meron.shared.parseThreadListPage
import jp.nonbili.meron.shared.parseThreadListResponse
import jp.nonbili.meron.shared.parseMailtoUrl
import jp.nonbili.meron.shared.recipientTail
import jp.nonbili.meron.shared.replaceRecipientTail
import jp.nonbili.meron.shared.buildOAuthAuthorizationUrl
import jp.nonbili.meron.shared.defaultOAuthRedirectUri
import jp.nonbili.meron.shared.detectReplyFromIdentity
import jp.nonbili.meron.shared.isOAuthCallbackUrl
import jp.nonbili.meron.shared.isPotentialOAuthCallbackUrl
import jp.nonbili.meron.shared.ownAddressList
import jp.nonbili.meron.shared.parseOAuthCallbackUrlForRedirect
import jp.nonbili.meron.shared.threadIdIsRss
import jp.nonbili.meron.shared.toReplyMailParams
import jp.nonbili.meron.shared.toSaveDraftParams
import jp.nonbili.meron.shared.toSendMailParams
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.net.URL
import java.security.MessageDigest
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.UUID
import kotlin.math.abs

// Gradient pairs mirror the desktop avatar palette in
// frontend/src/components/avatar/Avatar.tsx (Tailwind 400 -> 500 shades).
internal val avatarGradients = listOf(
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

internal fun isOutgoing(message: MessageBody, accountEmail: String): Boolean {
    if (accountEmail.isBlank()) return false
    val acct = accountEmail.trim().lowercase()
    val from = message.fromAddr.ifBlank { message.from }.trim().lowercase()
    return from.contains(acct)
}

internal fun folderIcon(name: String): androidx.compose.ui.graphics.vector.ImageVector = when (name.lowercase()) {
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

internal fun FilterMode.label(): String = when (this) {
    FilterMode.All -> "All"
    FilterMode.Unread -> "Unread"
    FilterMode.Starred -> "Starred"
}

internal fun FilterMode.protocolValue(): String = when (this) {
    FilterMode.All -> "all"
    FilterMode.Unread -> "unread"
    FilterMode.Starred -> "starred"
}

internal fun FilterMode.emptyNoun(): String = when (this) {
    FilterMode.All -> "cards"
    FilterMode.Unread -> "unread cards"
    FilterMode.Starred -> "starred cards"
}

internal fun List<ThreadSummary>.filteredKanbanThreads(filter: FilterMode, search: String): List<ThreadSummary> {
    val query = search.trim().lowercase()
    return filter { thread ->
        val filterOk = when (filter) {
            FilterMode.All -> true
            FilterMode.Unread -> thread.unread
            FilterMode.Starred -> thread.starred
        }
        val queryOk = query.isBlank() ||
            thread.subject.lowercase().contains(query) ||
            thread.sender.lowercase().contains(query) ||
            thread.preview.lowercase().contains(query) ||
            thread.accountId.lowercase().contains(query)
        filterOk && queryOk
    }.sortedByDescending { it.dateEpochSeconds }
}

internal fun columnTitle(
    column: KanbanColumnSpec,
    accounts: List<AccountSummary>,
    foldersByAccount: Map<String, List<FolderSummary>>,
): String {
    if (column.accountId == UNIFIED_ACCOUNT_ID) return "Unified inbox"
    val account = accounts.firstOrNull { it.id == column.accountId }
    val folder = foldersByAccount[column.accountId]
        ?.firstOrNull { it.name.equals(column.folderId, ignoreCase = true) }
        ?.name
        ?: column.folderId
    val folderLabel = if (folder.equals(INBOX_FOLDER, ignoreCase = true)) {
        if (account?.let(::accountSummaryIsRss) == true) "Feed" else "Inbox"
    } else {
        folder
    }
    val accountLabel = account?.displayName?.ifBlank { account.email } ?: column.accountId
    return "$accountLabel / $folderLabel"
}

internal fun KanbanBoardSpec.hasBoardStyle(): Boolean {
    return avatarUrl.isNotBlank() || wallpaperPresetId.isNotBlank() || wallpaperUrl.isNotBlank()
}

@Composable
internal fun KanbanBoardTile(board: KanbanBoardSpec?, size: Dp) {
    val styled = board?.hasBoardStyle() == true
    val tileBrush = if (styled) {
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

internal fun defaultKanbanBoard(accounts: List<AccountSummary>): KanbanBoardSpec {
    val columns = mutableListOf(KanbanColumnSpec(UNIFIED_ACCOUNT_ID, INBOX_FOLDER))
    accounts.forEach { account -> columns += KanbanColumnSpec(account.id, INBOX_FOLDER) }
    return KanbanBoardSpec(
        id = "kb-${UUID.randomUUID()}",
        name = "Kanban board",
        columns = columns.distinctBy(::kanbanColumnKey),
    )
}

internal fun ensureKanbanDefaults(
    context: Context,
    boards: List<KanbanBoardSpec>,
    accounts: List<AccountSummary>,
): List<KanbanBoardSpec> {
    val next = if (boards.isEmpty()) {
        listOf(defaultKanbanBoard(accounts))
    } else {
        boards.mapIndexed { index, board ->
            if (index != 0) board else {
                val existing = board.columns.map(::kanbanColumnKey).toMutableSet()
                val columns = board.columns.toMutableList()
                val unified = KanbanColumnSpec(UNIFIED_ACCOUNT_ID, INBOX_FOLDER)
                if (existing.add(kanbanColumnKey(unified))) columns.add(0, unified)
                accounts.forEach { account ->
                    val column = KanbanColumnSpec(account.id, INBOX_FOLDER)
                    if (existing.add(kanbanColumnKey(column))) columns.add(column)
                }
                board.copy(columns = columns)
            }
        }
    }
    if (next != boards) saveKanbanBoards(context, next)
    return next
}

internal fun loadKanbanBoards(context: Context, accounts: List<AccountSummary>): List<KanbanBoardSpec> {
    val raw = context.getSharedPreferences(KANBAN_PREFS, Context.MODE_PRIVATE).getString(KANBAN_BOARDS_PREF, null)
    val parsed = runCatching {
        if (raw.isNullOrBlank()) emptyList() else {
            val array = JSONArray(raw)
            buildList {
                for (i in 0 until array.length()) {
                    val obj = array.optJSONObject(i) ?: continue
                    val id = obj.optString("id").ifBlank { "kb-${UUID.randomUUID()}" }
                    val name = obj.optString("name").ifBlank { "Kanban board" }
                    val avatarUrl = obj.optString("avatarUrl")
                    val wallpaper = obj.optJSONObject("wallpaper")
                    val wallpaperPresetId = wallpaper?.optString("presetId").orEmpty()
                    val wallpaperUrl = wallpaper?.optString("url").orEmpty()
                    val colArray = obj.optJSONArray("columns") ?: JSONArray()
                    val columns = buildList {
                        for (j in 0 until colArray.length()) {
                            val col = colArray.optJSONObject(j) ?: continue
                            val accountId = col.optString("accountId")
                            val folderId = col.optString("folderId")
                            if (accountId.isNotBlank() && folderId.isNotBlank()) add(KanbanColumnSpec(accountId, folderId))
                        }
                    }.distinctBy(::kanbanColumnKey)
                    add(KanbanBoardSpec(id, name, columns, avatarUrl, wallpaperPresetId, wallpaperUrl))
                }
            }
        }
    }.getOrDefault(emptyList())
    return ensureKanbanDefaults(context, parsed, accounts)
}

internal fun saveKanbanBoards(context: Context, boards: List<KanbanBoardSpec>) {
    val array = JSONArray()
    boards.forEach { board ->
        val columns = JSONArray()
        board.columns.forEach { column ->
            columns.put(JSONObject().put("accountId", column.accountId).put("folderId", column.folderId))
        }
        val obj = JSONObject().put("id", board.id).put("name", board.name).put("columns", columns)
        if (board.avatarUrl.isNotBlank()) obj.put("avatarUrl", board.avatarUrl)
        when {
            board.wallpaperUrl.isNotBlank() -> obj.put("wallpaper", JSONObject().put("kind", "custom").put("url", board.wallpaperUrl))
            board.wallpaperPresetId.isNotBlank() -> obj.put("wallpaper", JSONObject().put("kind", "preset").put("presetId", board.wallpaperPresetId))
        }
        array.put(obj)
    }
    context.getSharedPreferences(KANBAN_PREFS, Context.MODE_PRIVATE)
        .edit()
        .putString(KANBAN_BOARDS_PREF, array.toString())
        .apply()
}

internal fun loadActiveKanbanBoardId(context: Context): String {
    return context.getSharedPreferences(KANBAN_PREFS, Context.MODE_PRIVATE).getString(ACTIVE_KANBAN_BOARD_PREF, "").orEmpty()
}

internal fun saveActiveKanbanBoardId(context: Context, boardId: String) {
    context.getSharedPreferences(KANBAN_PREFS, Context.MODE_PRIVATE).edit().putString(ACTIVE_KANBAN_BOARD_PREF, boardId).apply()
}

internal fun loadKanbanFilter(context: Context): FilterMode {
    val raw = context.getSharedPreferences(KANBAN_PREFS, Context.MODE_PRIVATE).getString(KANBAN_FILTER_PREF, "all")
    return when (raw) {
        "unread" -> FilterMode.Unread
        "starred" -> FilterMode.Starred
        else -> FilterMode.All
    }
}

internal fun saveKanbanFilter(context: Context, filter: FilterMode) {
    val value = when (filter) {
        FilterMode.All -> "all"
        FilterMode.Unread -> "unread"
        FilterMode.Starred -> "starred"
    }
    context.getSharedPreferences(KANBAN_PREFS, Context.MODE_PRIVATE).edit().putString(KANBAN_FILTER_PREF, value).apply()
}

internal fun loadKanbanSearch(context: Context): String {
    return context.getSharedPreferences(KANBAN_PREFS, Context.MODE_PRIVATE).getString(KANBAN_SEARCH_PREF, "").orEmpty()
}

internal fun saveKanbanSearch(context: Context, search: String) {
    context.getSharedPreferences(KANBAN_PREFS, Context.MODE_PRIVATE).edit().putString(KANBAN_SEARCH_PREF, search).apply()
}

internal fun appVersionName(context: Context): String {
    return runCatching {
        val packageInfo = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            context.packageManager.getPackageInfo(
                context.packageName,
                android.content.pm.PackageManager.PackageInfoFlags.of(0),
            )
        } else {
            @Suppress("DEPRECATION")
            context.packageManager.getPackageInfo(context.packageName, 0)
        }
        packageInfo.versionName ?: "0.1.0"
    }.getOrDefault("0.1.0")
}

internal fun SendShortcutMode.label(): String = when (this) {
    SendShortcutMode.Enter -> "Enter"
    SendShortcutMode.ModEnter -> "Ctrl+Enter"
}

internal fun SendShortcutMode.storageValue(): String = when (this) {
    SendShortcutMode.Enter -> "enter"
    SendShortcutMode.ModEnter -> "mod_enter"
}

internal fun SendShortcutMode.next(): SendShortcutMode = when (this) {
    SendShortcutMode.Enter -> SendShortcutMode.ModEnter
    SendShortcutMode.ModEnter -> SendShortcutMode.Enter
}

internal fun loadSendShortcutMode(context: Context): SendShortcutMode {
    return when (context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE).getString(SEND_SHORTCUT_PREF, "mod_enter")) {
        "enter" -> SendShortcutMode.Enter
        else -> SendShortcutMode.ModEnter
    }
}

internal fun saveSendShortcutMode(context: Context, mode: SendShortcutMode) {
    context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE)
        .edit()
        .putString(SEND_SHORTCUT_PREF, mode.storageValue())
        .apply()
}

internal fun shouldSendFromEditor(event: KeyEvent, mode: SendShortcutMode): Boolean {
    if (event.type != KeyEventType.KeyDown || event.key != Key.Enter) return false
    return when (mode) {
        SendShortcutMode.Enter -> !event.isShiftPressed && !event.isCtrlPressed && !event.isMetaPressed
        SendShortcutMode.ModEnter -> !event.isShiftPressed && (event.isCtrlPressed || event.isMetaPressed)
    }
}

internal fun loadAppearanceMode(context: Context): AppAppearanceMode {
    val stored = context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE)
        .getString(APPEARANCE_MODE_PREF, AppAppearanceMode.System.storageValue)
        .orEmpty()
    return AppAppearanceMode.entries.firstOrNull { it.storageValue == stored } ?: AppAppearanceMode.System
}

internal fun saveAppearanceMode(context: Context, mode: AppAppearanceMode) {
    context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE)
        .edit()
        .putString(APPEARANCE_MODE_PREF, mode.storageValue)
        .apply()
}

internal fun loadAppBoolean(context: Context, key: String, defaultValue: Boolean): Boolean {
    return context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE).getBoolean(key, defaultValue)
}

internal fun saveAppBoolean(context: Context, key: String, value: Boolean) {
    context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE)
        .edit()
        .putBoolean(key, value)
        .apply()
}

internal fun loadAppInt(context: Context, key: String, defaultValue: Int): Int {
    return context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE).getInt(key, defaultValue)
}

internal fun saveAppInt(context: Context, key: String, value: Int) {
    context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE)
        .edit()
        .putInt(key, value)
        .apply()
}

internal fun nextKanbanColumnWidth(current: Int): Int {
    val next = current + KANBAN_COLUMN_WIDTH_STEP
    return if (next > KANBAN_COLUMN_MAX_WIDTH) KANBAN_COLUMN_MIN_WIDTH else next
}

internal fun loadAppStringSet(context: Context, key: String): Set<String> {
    return context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE)
        .getStringSet(key, emptySet())
        .orEmpty()
        .filter { it.isNotBlank() }
        .toSet()
}

internal fun saveAppStringSet(context: Context, key: String, value: Set<String>) {
    context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE)
        .edit()
        .putStringSet(key, value.filter { it.isNotBlank() }.toSet())
        .apply()
}

internal fun onOffLabel(enabled: Boolean): String = if (enabled) "On" else "Off"

internal fun AppAppearanceMode.next(): AppAppearanceMode {
    val values = AppAppearanceMode.entries
    return values[(ordinal + 1) % values.size]
}

internal fun formatRelativeTime(epochSeconds: Long): String {
    if (epochSeconds <= 0) return ""
    val nowMillis = System.currentTimeMillis()
    val thenMillis = epochSeconds * 1000
    val diff = nowMillis - thenMillis
    return when {
        diff < 60_000 -> "now"
        diff < 3_600_000 -> "${diff / 60_000}m"
        diff < 86_400_000 -> "${diff / 3_600_000}h"
        diff < 7 * 86_400_000L -> SimpleDateFormat("EEE", Locale.getDefault()).format(Date(thenMillis))
        else -> SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(thenMillis))
    }
}

internal fun formatBytes(bytes: Long): String = when {
    bytes >= 1_000_000 -> "${bytes / 1_000_000} MB"
    bytes >= 1_000 -> "${bytes / 1_000} KB"
    else -> "$bytes B"
}

internal fun safeAttachmentFilename(name: String): String {
    val cleaned = name
        .ifBlank { "attachment" }
        .map { ch -> if (ch.isLetterOrDigit() || ch in listOf('.', '-', '_')) ch else '_' }
        .joinToString("")
        .trim('.', '_')
    return cleaned.ifBlank { "attachment" }
}

internal fun android.content.Context.displayNameFor(uri: Uri): String {
    contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
        if (cursor.moveToFirst()) {
            val index = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
            if (index >= 0) {
                val value = cursor.getString(index)
                if (!value.isNullOrBlank()) return value
            }
        }
    }
    return uri.lastPathSegment?.substringAfterLast('/') ?: "attachment"
}

internal fun Intent.toMailtoDraft(): ComposeDraft? {
    val uri = data?.toString() ?: return null
    if (action != Intent.ACTION_SENDTO && action != Intent.ACTION_VIEW) return null
    return parseMailtoUrl(uri)
}

internal fun Intent.toOAuthCallbackUrl(): String? {
    val uri = data?.toString() ?: return null
    return uri.takeIf { isPotentialOAuthCallbackUrl(it) || isOAuthCallbackUrl(it) }
}

internal fun coreStatus(coreInitJson: String): String {
    return if (MeronCoreNative.isLoaded()) {
        "Core protocol ${MeronCoreNative.protocolVersion()} · shared ${SharedMobileContract.protocolVersion}"
    } else {
        "Rust core not packaged yet; using Java fallback."
    }
}

internal fun String.pkceChallenge(): String {
    val digest = MessageDigest.getInstance("SHA-256").digest(toByteArray())
    return Base64.encodeToString(digest, Base64.URL_SAFE or Base64.NO_WRAP or Base64.NO_PADDING)
}
