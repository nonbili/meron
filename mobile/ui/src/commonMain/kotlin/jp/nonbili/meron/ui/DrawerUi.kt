package jp.nonbili.meron.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
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
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Code
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.CreditCard
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.FilterList
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.Info
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
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.rememberDrawerState
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
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
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
import jp.nonbili.meron.shared.AddOAuthAccountParams
import jp.nonbili.meron.shared.AddPasswordAccountParams
import jp.nonbili.meron.shared.AddRssAccountParams
import jp.nonbili.meron.shared.AddRssFeedParams
import jp.nonbili.meron.shared.AttachmentReadParams
import jp.nonbili.meron.shared.AutodiscoverAccountParams
import jp.nonbili.meron.shared.ComposeDraft
import jp.nonbili.meron.shared.ContactSuggestParams
import jp.nonbili.meron.shared.ContactSuggestion
import jp.nonbili.meron.shared.CopyThreadParams
import jp.nonbili.meron.shared.DiscardDraftParams
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.ExchangeOAuthCodeParams
import jp.nonbili.meron.shared.ExportOpmlParams
import jp.nonbili.meron.shared.FolderCreateParams
import jp.nonbili.meron.shared.FolderListParams
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.ImportOpmlParams
import jp.nonbili.meron.shared.MarkAllReadParams
import jp.nonbili.meron.shared.MarkReadParams
import jp.nonbili.meron.shared.MarkStarredParams
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.MoveRssFeedParams
import jp.nonbili.meron.shared.MoveThreadParams
import jp.nonbili.meron.shared.OAuthAuthorizationRequest
import jp.nonbili.meron.shared.RemoveRssFeedParams
import jp.nonbili.meron.shared.RssMarkReadParams
import jp.nonbili.meron.shared.RssMarkStarredParams
import jp.nonbili.meron.shared.RssThreadParams
import jp.nonbili.meron.shared.SendIdentity
import jp.nonbili.meron.shared.StarredItemSummary
import jp.nonbili.meron.shared.StorageUsage
import jp.nonbili.meron.shared.SyncMailParams
import jp.nonbili.meron.shared.SyncRssParams
import jp.nonbili.meron.shared.ThreadActionParams
import jp.nonbili.meron.shared.ThreadListParams
import jp.nonbili.meron.shared.ThreadReadParams
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.accountSendIdentities
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.attachmentToDraftAttachment
import jp.nonbili.meron.shared.buildOAuthAuthorizationUrl
import jp.nonbili.meron.shared.defaultOAuthRedirectUri
import jp.nonbili.meron.shared.detectReplyFromIdentity
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.folderIsTrash
import jp.nonbili.meron.shared.formatContactSuggestion
import jp.nonbili.meron.shared.formatSendIdentity
import jp.nonbili.meron.shared.forwardableAttachments
import jp.nonbili.meron.shared.isOAuthCallbackUrl
import jp.nonbili.meron.shared.isPotentialOAuthCallbackUrl
import jp.nonbili.meron.shared.messageEditAsNewDraft
import jp.nonbili.meron.shared.messageForwardDraft
import jp.nonbili.meron.shared.newDraftMessageId
import jp.nonbili.meron.shared.ownAddressList
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseAttachmentDataResponse
import jp.nonbili.meron.shared.parseAutodiscoverResponse
import jp.nonbili.meron.shared.parseContactSuggestResponse
import jp.nonbili.meron.shared.parseFolderListResponse
import jp.nonbili.meron.shared.parseMailtoUrl
import jp.nonbili.meron.shared.parseMediaFileUrlResponse
import jp.nonbili.meron.shared.parseOAuthCallbackUrlForRedirect
import jp.nonbili.meron.shared.parseOpmlExportResponse
import jp.nonbili.meron.shared.parseOpmlImportCountResponse
import jp.nonbili.meron.shared.parseStarredItemsResponse
import jp.nonbili.meron.shared.parseStorageUsageResponse
import jp.nonbili.meron.shared.parseThreadListPage
import jp.nonbili.meron.shared.parseThreadListResponse
import jp.nonbili.meron.shared.parseThreadReadPage
import jp.nonbili.meron.shared.recipientTail
import jp.nonbili.meron.shared.replaceRecipientTail
import jp.nonbili.meron.shared.threadIdIsRss
import jp.nonbili.meron.shared.toReplyMailParams
import jp.nonbili.meron.shared.toSaveDraftParams
import jp.nonbili.meron.shared.toSendMailParams
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.math.abs

@Composable
internal fun MailDrawer(
    accounts: List<AccountSummary>,
    selectedAccountId: String,
    folders: List<FolderSummary>,
    currentScreen: Screen,
    showUnreadBadges: Boolean,
    showUnifiedInboxNav: Boolean,
    showStarredNav: Boolean,
    kanbanBoards: List<KanbanBoardSpec>,
    activeKanbanBoardId: String,
    onSelectUnified: () -> Unit,
    onSelectAccount: (AccountSummary) -> Unit,
    onSelectStarred: () -> Unit,
    onSelectKanban: () -> Unit,
    onSelectKanbanBoard: (KanbanBoardSpec) -> Unit,
    onAddAccount: () -> Unit,
    onOpenSettings: () -> Unit,
    onShowAbout: () -> Unit,
    googleReauthAccountId: String? = null,
    onReconnectGoogle: (AccountSummary) -> Unit = {},
) {
    val chat = LocalChatColors.current
    ModalDrawerSheet(
        drawerContainerColor = chat.sidebar,
        drawerContentColor = chat.onSidebar,
    ) {
        LazyColumn(contentPadding = PaddingValues(vertical = 16.dp)) {
            item {
                Text(
                    "Meron",
                    style = MaterialTheme.typography.headlineSmall,
                    fontWeight = FontWeight.Bold,
                    color = chat.onSidebar,
                    modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp),
                )
            }
            if (accounts.isNotEmpty()) {
                item { DrawerLabel(tr("drawer.inboxes"), chat) }
                if (showUnifiedInboxNav) {
                    item {
                        val includedAccountIds = accounts.filter { it.includedInUnified }.map { it.id }.toSet()
                        val unread =
                            folders
                                .filter { it.accountId in includedAccountIds }
                                .groupBy { it.accountId }
                                .values
                                .sumOf { accountFolders -> folderUnread(accountFolders, INBOX_FOLDER) }
                        SidebarRow(
                            selected = currentScreen == Screen.Mail && selectedAccountId == UNIFIED_ACCOUNT_ID,
                            chat = chat,
                            onClick = onSelectUnified,
                            leading = { Icon(Icons.Filled.Inbox, contentDescription = null, modifier = Modifier.size(20.dp)) },
                            title = tr("kanban.columns.unifiedInbox"),
                            trailing = if (showUnreadBadges) unread.takeIf { it > 0 }?.toString() else null,
                        )
                    }
                }
                items(accounts, key = { it.id }) { account ->
                    val label = account.displayName.ifBlank { account.email.ifBlank { account.id } }
                    val unread = folderUnread(folders.filter { it.accountId == account.id }, INBOX_FOLDER)
                    val needsGoogleReauth = account.id == googleReauthAccountId
                    SidebarRow(
                        selected = currentScreen == Screen.Mail && account.id == selectedAccountId,
                        chat = chat,
                        onClick = {
                            if (needsGoogleReauth) onReconnectGoogle(account) else onSelectAccount(account)
                        },
                        leading = {
                            AccountBadgeAvatar(
                                label = label,
                                avatarUrl = account.avatarUrl,
                                size = 24.dp,
                            )
                        },
                        title = label,
                        trailing =
                            if (account.needsReconnect || needsGoogleReauth) {
                                "!"
                            } else if (showUnreadBadges) {
                                unread.takeIf { it > 0 }?.toString()
                            } else {
                                null
                            },
                    )
                }
            }
            if (accounts.isNotEmpty()) {
                item { DrawerLabel(tr("drawer.views"), chat) }
                if (showStarredNav) {
                    item {
                        SidebarRow(
                            selected = currentScreen == Screen.Starred,
                            chat = chat,
                            onClick = onSelectStarred,
                            leading = { Icon(Icons.Filled.Star, contentDescription = null, modifier = Modifier.size(20.dp)) },
                            title = tr("mobile.tabs.starred"),
                            trailing = null,
                        )
                    }
                }
                if (kanbanBoards.isEmpty()) {
                    item {
                        SidebarRow(
                            selected = currentScreen == Screen.Kanban,
                            chat = chat,
                            onClick = onSelectKanban,
                            leading = { Icon(Icons.Filled.ViewKanban, contentDescription = null, modifier = Modifier.size(20.dp)) },
                            title = tr("mobile.tabs.kanban"),
                            trailing = null,
                        )
                    }
                } else {
                    items(kanbanBoards, key = { it.id }) { board ->
                        SidebarRow(
                            selected = currentScreen == Screen.Kanban && board.id == activeKanbanBoardId,
                            chat = chat,
                            onClick = { onSelectKanbanBoard(board) },
                            leading = {
                                if (board.avatarUrl.isNotBlank()) {
                                    AccountBadgeAvatar(
                                        label = board.name,
                                        avatarUrl = board.avatarUrl,
                                        size = 24.dp,
                                    )
                                } else {
                                    Icon(Icons.Filled.ViewKanban, contentDescription = null, modifier = Modifier.size(20.dp))
                                }
                            },
                            title = board.name,
                            trailing = null,
                        )
                    }
                }
            }
            item {
                HorizontalDivider(
                    Modifier.padding(vertical = 10.dp, horizontal = 20.dp),
                    color = chat.onSidebarMuted.copy(alpha = 0.25f),
                )
            }
            item {
                SidebarRow(
                    selected = false,
                    chat = chat,
                    onClick = onAddAccount,
                    leading = { Icon(Icons.Filled.PersonAdd, contentDescription = null, modifier = Modifier.size(20.dp)) },
                    title = tr("accounts.actions.addAccount"),
                    trailing = null,
                )
            }
            item {
                SidebarRow(
                    selected = false,
                    chat = chat,
                    onClick = onOpenSettings,
                    leading = { Icon(Icons.Filled.Settings, contentDescription = null, modifier = Modifier.size(20.dp)) },
                    title = tr("settings.label"),
                    trailing = null,
                )
            }
            item {
                SidebarRow(
                    selected = false,
                    chat = chat,
                    onClick = onShowAbout,
                    leading = { Icon(Icons.Filled.Info, contentDescription = null, modifier = Modifier.size(20.dp)) },
                    title = tr("about.title"),
                    trailing = null,
                )
            }
        }
    }
}

@Composable
internal fun AboutDialog(
    appVersion: String,
    onOpenUrl: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("about.title")) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text("Meron", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                Text(
                    tr("about.version", mapOf("version" to appVersion)),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    AboutLinkButton(
                        icon = Icons.Filled.Code,
                        label = tr("about.sourceCode"),
                        onClick = { onOpenUrl("https://github.com/nonbili/meron") },
                    )
                    Text(
                        tr("about.donate"),
                        style = MaterialTheme.typography.labelLarge,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        AboutLinkButton(
                            icon = Icons.Filled.Favorite,
                            label = tr("about.githubSponsors"),
                            onClick = { onOpenUrl("https://github.com/sponsors/nonbili") },
                        )
                        AboutLinkButton(
                            icon = Icons.Filled.Favorite,
                            label = tr("about.liberapay"),
                            onClick = { onOpenUrl("https://liberapay.com/nonbili") },
                        )
                        AboutLinkButton(
                            icon = Icons.Filled.CreditCard,
                            label = tr("about.paypal"),
                            onClick = { onOpenUrl("https://www.paypal.com/paypalme/nonbili") },
                        )
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) {
                Text(tr("buttons.close"))
            }
        },
    )
}

@Composable
private fun AboutLinkButton(
    icon: ImageVector,
    label: String,
    onClick: () -> Unit,
) {
    OutlinedButton(
        onClick = onClick,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(
            Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(icon, contentDescription = null, modifier = Modifier.size(18.dp))
            Spacer(Modifier.width(8.dp))
            Text(label)
        }
    }
}

@Composable
internal fun SidebarRow(
    selected: Boolean,
    chat: ChatColors,
    onClick: () -> Unit,
    leading: @Composable () -> Unit,
    title: String,
    trailing: String?,
) {
    val tint = if (selected) MaterialTheme.colorScheme.primary else chat.onSidebarMuted
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 2.dp)
            .clip(RoundedCornerShape(10.dp))
            .background(if (selected) MaterialTheme.colorScheme.primary.copy(alpha = 0.18f) else Color.Transparent)
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 11.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(contentAlignment = Alignment.Center) {
            androidx.compose.runtime.CompositionLocalProvider(
                androidx.compose.material3.LocalContentColor provides tint,
            ) { leading() }
        }
        Text(
            title,
            color = if (selected) chat.onSidebar else chat.onSidebar.copy(alpha = 0.9f),
            fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium,
            fontSize = 14.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        if (trailing != null) {
            Box(
                Modifier
                    .height(28.dp)
                    .widthIn(min = 28.dp)
                    .clip(CircleShape)
                    .background(MaterialTheme.colorScheme.primary)
                    .padding(horizontal = 8.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    trailing,
                    color = Color.White,
                    fontSize = 12.sp,
                    lineHeight = 12.sp,
                    fontWeight = FontWeight.Bold,
                    maxLines = 1,
                )
            }
        }
    }
}

@Composable
internal fun DrawerLabel(
    text: String,
    chat: ChatColors,
) {
    Text(
        text.uppercase(),
        style = MaterialTheme.typography.labelSmall,
        fontWeight = FontWeight.Bold,
        color = chat.onSidebarMuted,
        modifier = Modifier.padding(start = 24.dp, top = 14.dp, bottom = 6.dp),
    )
}

@Composable
internal fun StatusBanner(
    message: String,
    isError: Boolean,
    actionLabel: String?,
    onAction: (() -> Unit)?,
    onDismiss: (() -> Unit)?,
) {
    val container = if (isError) MaterialTheme.colorScheme.errorContainer else MaterialTheme.colorScheme.primaryContainer
    val content = if (isError) MaterialTheme.colorScheme.onErrorContainer else MaterialTheme.colorScheme.onPrimaryContainer
    Surface(color = container, contentColor = content) {
        Row(
            Modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, top = 10.dp, bottom = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Icon(Icons.Filled.ErrorOutline, contentDescription = null, modifier = Modifier.size(20.dp))
            Text(message, fontSize = 13.sp, modifier = Modifier.weight(1f), maxLines = 3, overflow = TextOverflow.Ellipsis)
            if (actionLabel != null && onAction != null) {
                TextButton(onClick = onAction) {
                    Text(actionLabel, color = content, fontWeight = FontWeight.SemiBold)
                }
            }
            if (onDismiss != null) {
                IconButton(onClick = onDismiss, modifier = Modifier.size(32.dp)) {
                    Icon(Icons.Filled.Close, contentDescription = tr("buttons.close"), modifier = Modifier.size(18.dp))
                }
            }
        }
    }
}

@Composable
internal fun EmptyState(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    title: String,
    text: String,
    actionLabel: String,
    onAction: () -> Unit,
) {
    Column(
        Modifier.fillMaxSize().padding(32.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(icon, contentDescription = null, modifier = Modifier.size(56.dp), tint = MaterialTheme.colorScheme.primary)
        Spacer(Modifier.height(16.dp))
        Text(title, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.height(8.dp))
        Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(20.dp))
        Button(onClick = onAction) { Text(actionLabel) }
    }
}

// Shown while the first load is still in flight, so a cold start reads as
// "loading" rather than an empty inbox.
@Composable
internal fun LoadingState(text: String = "Loading…") {
    Column(
        Modifier.fillMaxSize().padding(32.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        CircularProgressIndicator()
        Spacer(Modifier.height(16.dp))
        Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}
