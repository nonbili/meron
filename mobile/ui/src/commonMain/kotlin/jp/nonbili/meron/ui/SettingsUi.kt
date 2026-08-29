package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Chat
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.BugReport
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.FormatSize
import androidx.compose.material.icons.filled.HideImage
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.MarkEmailUnread
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Restore
import androidx.compose.material.icons.filled.RssFeed
import androidx.compose.material.icons.filled.Save
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.ViewKanban
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.backhandler.BackHandler
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.ProxySpec
import jp.nonbili.meron.shared.SignatureSpec
import jp.nonbili.meron.shared.StorageUsage
import jp.nonbili.meron.shared.accountSummaryIsRss

@OptIn(ExperimentalMaterial3Api::class, ExperimentalComposeUiApi::class)
@Composable
internal fun SettingsScreen(
    onBack: () -> Unit,
    initialGeneral: Boolean,
    onConsumeInitialGeneral: () -> Unit,
    initialAccountId: String?,
    initialAccountProxy: Boolean,
    onConsumeInitialAccount: () -> Unit,
    initialKanbanBoardId: String?,
    onConsumeInitialKanbanBoard: () -> Unit,
    accounts: List<AccountSummary>,
    hiddenNavigationAccountIds: Set<String>,
    kanbanBoards: List<KanbanBoardSpec>,
    onSaveKanbanBoard: (
        board: KanbanBoardSpec,
        name: String,
        avatarUrl: String,
        wallpaperPresetId: String,
        wallpaperUrl: String,
    ) -> Unit,
    onDeleteKanbanBoard: (KanbanBoardSpec) -> Unit,
    onCreateKanbanBoard: () -> String,
    onAddMailAccount: () -> Unit,
    onAddFeedAccount: () -> Unit,
    onSaveAccountSettings: (
        account: AccountSummary,
        displayName: String,
        senderName: String,
        avatarUrl: String,
        wallpaperPresetId: String,
        loadRemoteImages: Boolean,
        conversationHtml: Boolean,
        includedInUnified: Boolean,
        showInNavigation: Boolean,
        muted: Boolean,
        paused: Boolean,
        rssSyncIntervalMinutes: Int,
        aliasesText: String,
    ) -> Unit,
    onPickAccountAvatar: (AccountSummary) -> Unit,
    onPickAccountWallpaper: (AccountSummary) -> Unit,
    onPickKanbanBoardAvatar: (KanbanBoardSpec) -> Unit,
    onPickKanbanBoardWallpaper: (KanbanBoardSpec) -> Unit,
    onMoveAccountUp: (AccountSummary) -> Unit,
    onMoveAccountDown: (AccountSummary) -> Unit,
    onRemoveAccount: (AccountSummary) -> Unit,
    appearanceMode: AppAppearanceMode,
    onAppearanceModeChange: (AppAppearanceMode) -> Unit,
    appLanguageTag: String,
    onAppLanguageChange: (String) -> Unit,
    showSenderImages: Boolean,
    onToggleSenderImages: () -> Unit,
    showUnreadBadges: Boolean,
    onToggleUnreadBadges: () -> Unit,
    showUnifiedInboxNav: Boolean,
    onToggleUnifiedInboxNav: () -> Unit,
    sendShortcutMode: SendShortcutMode,
    onToggleSendShortcut: () -> Unit,
    conversationLayout: ConversationLayout,
    onToggleConversationLayout: () -> Unit,
    messageFontScale: Int,
    onMessageFontScaleChange: (Int) -> Unit,
    appProxy: ProxySpec,
    onSaveAppProxy: (ProxySpec) -> Unit,
    appSignatureHtml: String,
    onSaveAppSignature: (String) -> Unit,
    remoteImageSenders: List<String>,
    onRemoveRemoteImageSender: (String) -> Unit,
    onSaveAccountSignature: (AccountSummary, SignatureSpec) -> Unit,
    onSaveAccountProxy: (AccountSummary, ProxySpec) -> Unit,
    onSaveAccountServerSettings: (AccountSummary, ServerSettingsDraft) -> Unit,
    kanbanColumnWidth: Int,
    onCycleKanbanColumnWidth: () -> Unit,
    notificationsNeedPermission: Boolean,
    onEnableNotifications: () -> Unit,
    supportsBackgroundPush: Boolean,
    liveMailPushEnabled: Boolean,
    onToggleLiveMailPush: () -> Unit,
    backgroundSyncEnabled: Boolean,
    onToggleBackgroundSync: () -> Unit,
    onRefreshBackground: () -> Unit,
    readDiagnosticLog: () -> String,
    onShareDiagnosticLog: () -> Unit,
    pollIntervalMinutes: Int,
    onCyclePollInterval: () -> Unit,
    storageUsage: StorageUsage?,
    storageBusy: Boolean,
    storageClearConfirming: Boolean,
    onRefreshStorage: () -> Unit,
    onClearStorageCache: () -> Unit,
    onExportBackup: () -> Unit,
    onRestoreBackup: () -> Unit,
    backupBusy: Boolean,
) {
    var showThemePicker by remember { mutableStateOf(false) }
    var showLanguagePicker by remember { mutableStateOf(false) }
    var showMessageTextSize by remember { mutableStateOf(false) }
    val settingsNavController = rememberNavController()
    val settingsBackStackEntry by settingsNavController.currentBackStackEntryAsState()
    var selectedSettingsAccountId by remember { mutableStateOf<String?>(null) }
    var selectedSettingsBoardId by remember { mutableStateOf<String?>(null) }
    var focusAccountProxy by remember { mutableStateOf(false) }
    var directOpenRoute by remember { mutableStateOf<String?>(null) }
    val page =
        when (settingsBackStackEntry?.destination?.route ?: SettingsRoutes.Root) {
            SettingsRoutes.General -> SettingsPage.General
            SettingsRoutes.Account -> selectedSettingsAccountId?.let { SettingsPage.AccountDetail(it) } ?: SettingsPage.Root
            SettingsRoutes.AccountWallpaper -> selectedSettingsAccountId?.let { SettingsPage.AccountWallpaper(it) } ?: SettingsPage.Root
            SettingsRoutes.KanbanBoard -> selectedSettingsBoardId?.let { SettingsPage.KanbanBoardDetail(it) } ?: SettingsPage.Root
            SettingsRoutes.KanbanBoardWallpaper -> selectedSettingsBoardId?.let { SettingsPage.KanbanBoardWallpaper(it) } ?: SettingsPage.Root
            SettingsRoutes.SyncLog -> SettingsPage.SyncLog
            SettingsRoutes.RemoteSenders -> SettingsPage.RemoteSenders
            else -> SettingsPage.Root
        }
    LaunchedEffect(initialGeneral) {
        if (initialGeneral) {
            directOpenRoute = SettingsRoutes.General
            settingsNavController.navigate(SettingsRoutes.General) {
                launchSingleTop = true
            }
            onConsumeInitialGeneral()
        }
    }
    LaunchedEffect(initialAccountId) {
        if (!initialAccountId.isNullOrBlank()) {
            selectedSettingsAccountId = initialAccountId
            focusAccountProxy = initialAccountProxy
            directOpenRoute = SettingsRoutes.Account
            settingsNavController.navigate(SettingsRoutes.Account) {
                launchSingleTop = true
            }
            onConsumeInitialAccount()
        }
    }
    LaunchedEffect(initialKanbanBoardId) {
        if (!initialKanbanBoardId.isNullOrBlank()) {
            selectedSettingsBoardId = initialKanbanBoardId
            directOpenRoute = SettingsRoutes.KanbanBoard
            settingsNavController.navigate(SettingsRoutes.KanbanBoard) {
                launchSingleTop = true
            }
            onConsumeInitialKanbanBoard()
        }
    }
    if (showThemePicker) {
        ThemePickerDialog(
            current = appearanceMode,
            onSelect = onAppearanceModeChange,
            onDismiss = { showThemePicker = false },
        )
    }
    if (showLanguagePicker) {
        LanguagePickerDialog(
            currentTag = appLanguageTag,
            onSelect = onAppLanguageChange,
            onDismiss = { showLanguagePicker = false },
        )
    }
    if (showMessageTextSize) {
        MessageTextSizeDialog(
            scale = messageFontScale,
            onScaleChange = onMessageFontScaleChange,
            onDismiss = { showMessageTextSize = false },
        )
    }
    val handleBack: () -> Unit = {
        if (settingsBackStackEntry?.destination?.route == directOpenRoute) {
            directOpenRoute = null
            onBack()
        } else if (!settingsNavController.popBackStack()) {
            onBack()
        }
    }
    BackHandler(onBack = handleBack)
    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        when (page) {
                            SettingsPage.Root -> tr("settings.label")
                            SettingsPage.General -> tr("settings.sections.general")
                            is SettingsPage.AccountDetail -> tr("settings.account.account")
                            is SettingsPage.AccountWallpaper -> tr("settings.account.chatBackground")
                            is SettingsPage.KanbanBoardDetail -> tr("kanban.board.label")
                            is SettingsPage.KanbanBoardWallpaper -> tr("settings.account.chatBackground")
                            SettingsPage.SyncLog -> tr("settings.viewSyncLog")
                            SettingsPage.RemoteSenders -> tr("settings.privacy.remoteSenders")
                        },
                    )
                },
                navigationIcon = {
                    IconButton(onClick = handleBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = tr("buttons.back"))
                    }
                },
                actions = {
                    if (page == SettingsPage.SyncLog) {
                        IconButton(onClick = onShareDiagnosticLog) {
                            Icon(Icons.AutoMirrored.Filled.Send, contentDescription = tr("settings.shareSyncLog"))
                        }
                    }
                },
            )
        },
    ) { innerPadding ->
        NavHost(
            navController = settingsNavController,
            startDestination = SettingsRoutes.Root,
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(innerPadding)
                    .consumeWindowInsets(innerPadding)
                    .imePadding(),
        ) {
            composable(SettingsRoutes.General) {
                SettingsGeneralPage(
                    appearanceMode = appearanceMode,
                    onOpenTheme = { showThemePicker = true },
                    appLanguageTag = appLanguageTag,
                    onOpenLanguage = { showLanguagePicker = true },
                    showSenderImages = showSenderImages,
                    onToggleSenderImages = onToggleSenderImages,
                    showUnreadBadges = showUnreadBadges,
                    onToggleUnreadBadges = onToggleUnreadBadges,
                    showUnifiedInboxNav = showUnifiedInboxNav,
                    onToggleUnifiedInboxNav = onToggleUnifiedInboxNav,
                    kanbanColumnWidth = kanbanColumnWidth,
                    onCycleKanbanColumnWidth = onCycleKanbanColumnWidth,
                    sendShortcutMode = sendShortcutMode,
                    onToggleSendShortcut = onToggleSendShortcut,
                    conversationLayout = conversationLayout,
                    onToggleConversationLayout = onToggleConversationLayout,
                    messageFontScale = messageFontScale,
                    onOpenMessageTextSize = { showMessageTextSize = true },
                    appProxy = appProxy,
                    onSaveAppProxy = onSaveAppProxy,
                    appSignatureHtml = appSignatureHtml,
                    onSaveAppSignature = onSaveAppSignature,
                    remoteImageSenderCount = remoteImageSenders.size,
                    onOpenRemoteSenders = { settingsNavController.navigate(SettingsRoutes.RemoteSenders) },
                    notificationsNeedPermission = notificationsNeedPermission,
                    onEnableNotifications = onEnableNotifications,
                    supportsBackgroundPush = supportsBackgroundPush,
                    liveMailPushEnabled = liveMailPushEnabled,
                    onToggleLiveMailPush = onToggleLiveMailPush,
                    backgroundSyncEnabled = backgroundSyncEnabled,
                    onToggleBackgroundSync = onToggleBackgroundSync,
                    onRefreshBackground = onRefreshBackground,
                    onOpenSyncLog = { settingsNavController.navigate(SettingsRoutes.SyncLog) },
                    pollIntervalMinutes = pollIntervalMinutes,
                    onCyclePollInterval = onCyclePollInterval,
                    storageUsage = storageUsage,
                    storageBusy = storageBusy,
                    storageClearConfirming = storageClearConfirming,
                    onRefreshStorage = onRefreshStorage,
                    onClearStorageCache = onClearStorageCache,
                    onExportBackup = onExportBackup,
                    onRestoreBackup = onRestoreBackup,
                    backupBusy = backupBusy,
                    focusProxy = directOpenRoute == SettingsRoutes.General,
                    modifier = Modifier.fillMaxSize(),
                )
            }

            composable(SettingsRoutes.RemoteSenders) {
                RemoteSendersPage(
                    senders = remoteImageSenders,
                    onRemoveSender = onRemoveRemoteImageSender,
                    modifier = Modifier.fillMaxSize(),
                )
            }

            composable(SettingsRoutes.Account) {
                BackHandler(enabled = directOpenRoute == SettingsRoutes.Account) {
                    directOpenRoute = null
                    onBack()
                }
                val account = selectedSettingsAccountId?.let { id -> accounts.firstOrNull { it.id == id } }
                if (account == null) {
                    LaunchedEffect(Unit) { settingsNavController.popBackStack(SettingsRoutes.Root, inclusive = false) }
                } else {
                    val accountIndex = accounts.indexOfFirst { it.id == account.id }
                    SettingsAccountDetailPage(
                        account = account,
                        canMoveUp = accountIndex > 0,
                        canMoveDown = accountIndex >= 0 && accountIndex < accounts.lastIndex,
                        showInNavigation = account.id !in hiddenNavigationAccountIds,
                        onSave = {
                            displayName,
                            senderName,
                            avatarUrl,
                            wallpaperPresetId,
                            loadRemoteImages,
                            conversationHtml,
                            includedInUnified,
                            showInNavigation,
                            muted,
                            paused,
                            interval,
                            aliases,
                            ->
                            onSaveAccountSettings(
                                account,
                                displayName,
                                senderName,
                                avatarUrl,
                                wallpaperPresetId,
                                loadRemoteImages,
                                conversationHtml,
                                includedInUnified,
                                showInNavigation,
                                muted,
                                paused,
                                interval,
                                aliases,
                            )
                        },
                        onSaveProxy = { spec -> onSaveAccountProxy(account, spec) },
                        onSaveServerSettings = { draft -> onSaveAccountServerSettings(account, draft) },
                        onSaveSignature = { spec -> onSaveAccountSignature(account, spec) },
                        onPickAvatar = { onPickAccountAvatar(account) },
                        onOpenWallpaper = { settingsNavController.navigate(SettingsRoutes.AccountWallpaper) },
                        onMoveUp = { onMoveAccountUp(account) },
                        onMoveDown = { onMoveAccountDown(account) },
                        onRemove = { onRemoveAccount(account) },
                        focusProxy = focusAccountProxy && directOpenRoute == SettingsRoutes.Account,
                        modifier = Modifier.fillMaxSize(),
                    )
                }
            }

            composable(SettingsRoutes.KanbanBoard) {
                BackHandler(enabled = directOpenRoute == SettingsRoutes.KanbanBoard) {
                    directOpenRoute = null
                    onBack()
                }
                val board = selectedSettingsBoardId?.let { id -> kanbanBoards.firstOrNull { it.id == id } }
                if (board == null) {
                    LaunchedEffect(Unit) { settingsNavController.popBackStack(SettingsRoutes.Root, inclusive = false) }
                } else {
                    SettingsKanbanBoardDetailPage(
                        board = board,
                        onSave = { name, avatarUrl, wallpaperPresetId, wallpaperUrl ->
                            onSaveKanbanBoard(board, name, avatarUrl, wallpaperPresetId, wallpaperUrl)
                        },
                        onPickAvatar = { onPickKanbanBoardAvatar(board) },
                        onOpenWallpaper = { settingsNavController.navigate(SettingsRoutes.KanbanBoardWallpaper) },
                        onDelete = {
                            onDeleteKanbanBoard(board)
                            settingsNavController.popBackStack(SettingsRoutes.Root, inclusive = false)
                        },
                        modifier = Modifier.fillMaxSize(),
                    )
                }
            }

            composable(SettingsRoutes.AccountWallpaper) {
                val account = selectedSettingsAccountId?.let { id -> accounts.firstOrNull { it.id == id } }
                if (account == null) {
                    LaunchedEffect(Unit) { settingsNavController.popBackStack(SettingsRoutes.Root, inclusive = false) }
                } else {
                    WallpaperPickerPage(
                        selected = if (account.chatWallpaperKind == "custom") "__custom" else account.chatWallpaperPresetId,
                        previewPresetId = account.chatWallpaperPresetId,
                        previewCustomUrl = if (account.chatWallpaperKind == "custom") account.chatWallpaperUrl else "",
                        onSelect = { presetId ->
                            onSaveAccountSettings(
                                account,
                                account.displayName,
                                account.senderName,
                                account.avatarUrl,
                                presetId,
                                account.loadRemoteImages,
                                account.conversationHtml,
                                account.includedInUnified,
                                account.id !in hiddenNavigationAccountIds,
                                account.muted,
                                account.paused,
                                account.rssSyncIntervalMinutes,
                                account.aliases.joinToString("\n") { alias ->
                                    if (alias.name.isBlank()) alias.email else "${alias.email}, ${alias.name}"
                                },
                            )
                        },
                        onUpload = { onPickAccountWallpaper(account) },
                        modifier = Modifier.fillMaxSize(),
                    )
                }
            }

            composable(SettingsRoutes.KanbanBoardWallpaper) {
                val board = selectedSettingsBoardId?.let { id -> kanbanBoards.firstOrNull { it.id == id } }
                if (board == null) {
                    LaunchedEffect(Unit) { settingsNavController.popBackStack(SettingsRoutes.Root, inclusive = false) }
                } else {
                    WallpaperPickerPage(
                        selected = if (board.wallpaperUrl.isNotBlank()) "__custom" else board.wallpaperPresetId,
                        previewPresetId = board.wallpaperPresetId,
                        previewCustomUrl = board.wallpaperUrl,
                        onSelect = { presetId ->
                            onSaveKanbanBoard(board, board.name, board.avatarUrl, presetId, "")
                        },
                        onUpload = { onPickKanbanBoardWallpaper(board) },
                        modifier = Modifier.fillMaxSize(),
                    )
                }
            }

            composable(SettingsRoutes.SyncLog) {
                SettingsSyncLogPage(
                    logText = remember { readDiagnosticLog() },
                    modifier = Modifier.fillMaxSize(),
                )
            }

            composable(SettingsRoutes.Root) {
                // Mirrors the desktop Settings sidebar: a single "General" entry,
                // then Kanban boards, Mail accounts, and Feed accounts sections.
                val mailAccounts = accounts.filter { !accountSummaryIsRss(it) }
                val feedAccounts = accounts.filter { accountSummaryIsRss(it) }
                val rootListState = rememberLazyListState()
                LazyColumn(Modifier.fillMaxSize().appScrollbar(rootListState), state = rootListState) {
                    item {
                        SettingsRow(
                            icon = Icons.Filled.Settings,
                            title = tr("settings.sections.general"),
                            subtitle = null,
                            onClick = { settingsNavController.navigate(SettingsRoutes.General) },
                        )
                    }
                    item { SettingsSectionLabel(tr("settings.sections.kanbanBoards")) }
                    items(kanbanBoards, key = { it.id }) { board ->
                        SettingsRow(
                            icon = Icons.Filled.ViewKanban,
                            leading = { KanbanBoardTile(board, 40.dp) },
                            title = board.name,
                            subtitle = trf("settings.kanban.boardColumns", board.columns.size),
                            onClick = {
                                selectedSettingsBoardId = board.id
                                settingsNavController.navigate(SettingsRoutes.KanbanBoard)
                            },
                        )
                    }
                    item {
                        SettingsRow(
                            icon = Icons.Filled.Add,
                            title = tr("settings.kanban.newBoard"),
                            subtitle = null,
                            onClick = {
                                selectedSettingsBoardId = onCreateKanbanBoard()
                                settingsNavController.navigate(SettingsRoutes.KanbanBoard)
                            },
                        )
                    }
                    item { SettingsSectionLabel(tr("settings.sections.mailAccounts")) }
                    if (mailAccounts.isEmpty()) {
                        item { SettingsEmptyLabel(tr("settings.sections.noMailAccounts")) }
                    } else {
                        items(mailAccounts, key = { it.id }) { account ->
                            SettingsAccountRow(
                                account = account,
                                hidden = account.id in hiddenNavigationAccountIds,
                                onClick = {
                                    selectedSettingsAccountId = account.id
                                    settingsNavController.navigate(SettingsRoutes.Account)
                                },
                            )
                        }
                    }
                    item {
                        SettingsRow(
                            icon = Icons.Filled.Add,
                            title = tr("settings.account.addMailAccount"),
                            subtitle = null,
                            onClick = onAddMailAccount,
                        )
                    }
                    item { SettingsSectionLabel(tr("settings.sections.feedAccounts")) }
                    if (feedAccounts.isEmpty()) {
                        item { SettingsEmptyLabel(tr("settings.sections.noFeedAccounts")) }
                    } else {
                        items(feedAccounts, key = { it.id }) { account ->
                            SettingsAccountRow(
                                account = account,
                                hidden = account.id in hiddenNavigationAccountIds,
                                fallbackTitle = tr("accounts.rssAtomFeeds"),
                                showAccountIdFallback = false,
                                showDefaultSubtitle = false,
                                onClick = {
                                    selectedSettingsAccountId = account.id
                                    settingsNavController.navigate(SettingsRoutes.Account)
                                },
                            )
                        }
                    }
                    item {
                        SettingsRow(
                            icon = Icons.Filled.Add,
                            title = tr("mobile.accounts.addRssAccount"),
                            subtitle = null,
                            onClick = onAddFeedAccount,
                        )
                    }
                    item { Spacer(Modifier.height(24.dp)) }
                }
            }
        }
    }
}

/** In-app viewer for the on-device diagnostic log, so a user can inspect what
 *  happened before deciding to share it (sharing lives in the top bar). */
@Composable
private fun SettingsSyncLogPage(
    logText: String,
    modifier: Modifier = Modifier,
) {
    val scrollState = rememberScrollState()
    // The newest entries are at the end; start there.
    LaunchedEffect(logText) { scrollState.scrollTo(scrollState.maxValue) }
    Column(modifier.appScrollbar(scrollState).verticalScroll(scrollState).padding(16.dp)) {
        if (logText.isBlank()) {
            Text(tr("settings.syncLogEmpty"), color = MaterialTheme.colorScheme.onSurfaceVariant)
        } else {
            SelectionContainer {
                Text(logText, style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace)
            }
        }
    }
}

private sealed class SettingsPage {
    data object Root : SettingsPage()

    data object General : SettingsPage()

    data object RemoteSenders : SettingsPage()

    data class AccountDetail(
        val accountId: String,
    ) : SettingsPage()

    data class AccountWallpaper(
        val accountId: String,
    ) : SettingsPage()

    data class KanbanBoardDetail(
        val boardId: String,
    ) : SettingsPage()

    data class KanbanBoardWallpaper(
        val boardId: String,
    ) : SettingsPage()

    data object SyncLog : SettingsPage()
}

private object SettingsRoutes {
    const val Root = "settings/root"
    const val General = "settings/general"
    const val Account = "settings/account"
    const val AccountWallpaper = "settings/account-wallpaper"
    const val KanbanBoard = "settings/kanban-board"
    const val KanbanBoardWallpaper = "settings/kanban-board-wallpaper"
    const val SyncLog = "settings/sync-log"
    const val RemoteSenders = "settings/remote-senders"
}

// Combines the desktop "General" section: Appearance, Sidebar, Kanban,
// Composer, Sync & notifications, and Storage in one scrollable page.
@Composable
internal fun SettingsGeneralPage(
    appearanceMode: AppAppearanceMode,
    onOpenTheme: () -> Unit,
    appLanguageTag: String,
    onOpenLanguage: () -> Unit,
    showSenderImages: Boolean,
    onToggleSenderImages: () -> Unit,
    showUnreadBadges: Boolean,
    onToggleUnreadBadges: () -> Unit,
    showUnifiedInboxNav: Boolean,
    onToggleUnifiedInboxNav: () -> Unit,
    kanbanColumnWidth: Int,
    onCycleKanbanColumnWidth: () -> Unit,
    sendShortcutMode: SendShortcutMode,
    onToggleSendShortcut: () -> Unit,
    conversationLayout: ConversationLayout,
    onToggleConversationLayout: () -> Unit,
    messageFontScale: Int,
    onOpenMessageTextSize: () -> Unit,
    appProxy: ProxySpec,
    onSaveAppProxy: (ProxySpec) -> Unit,
    appSignatureHtml: String,
    onSaveAppSignature: (String) -> Unit,
    remoteImageSenderCount: Int,
    onOpenRemoteSenders: () -> Unit,
    notificationsNeedPermission: Boolean,
    onEnableNotifications: () -> Unit,
    supportsBackgroundPush: Boolean,
    liveMailPushEnabled: Boolean,
    onToggleLiveMailPush: () -> Unit,
    backgroundSyncEnabled: Boolean,
    onToggleBackgroundSync: () -> Unit,
    onRefreshBackground: () -> Unit,
    onOpenSyncLog: () -> Unit,
    pollIntervalMinutes: Int,
    onCyclePollInterval: () -> Unit,
    storageUsage: StorageUsage?,
    storageBusy: Boolean,
    storageClearConfirming: Boolean,
    onRefreshStorage: () -> Unit,
    onClearStorageCache: () -> Unit,
    onExportBackup: () -> Unit,
    onRestoreBackup: () -> Unit,
    backupBusy: Boolean,
    focusProxy: Boolean = false,
    modifier: Modifier = Modifier,
) {
    val listState = rememberLazyListState(initialFirstVisibleItemIndex = if (focusProxy) 12 else 0)
    LazyColumn(modifier.appScrollbar(listState), state = listState) {
        item { SettingsSectionLabel(tr("settings.pages.appearance")) }
        item {
            val displayedAppearanceMode =
                if (appearanceMode == AppAppearanceMode.System) AppAppearanceMode.Indigo else appearanceMode
            SettingsRow(
                icon = Icons.Filled.Visibility,
                title = tr("common.theme"),
                onClick = onOpenTheme,
                trailing = {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(displayedAppearanceMode.label, color = MaterialTheme.colorScheme.primary)
                        Box(
                            Modifier
                                .width(44.dp)
                                .clip(RoundedCornerShape(6.dp))
                                .border(1.dp, MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(6.dp)),
                        ) {
                            ThemePreviewMock(themePreviewColors(displayedAppearanceMode), height = 26.dp)
                        }
                    }
                },
            )
        }
        item {
            SettingsRow(
                icon = Icons.AutoMirrored.Filled.Chat,
                title = tr("settings.appearance.conversationLayout"),
                hint = tr("settings.appearance.conversationLayoutHint"),
                onClick = onToggleConversationLayout,
                trailing = { Text(conversationLayout.label(), color = MaterialTheme.colorScheme.primary) },
            )
        }
        item {
            SettingsRow(
                icon = Icons.Filled.Settings,
                title = tr("settings.language.label"),
                onClick = onOpenLanguage,
                trailing = {
                    Text(
                        if (appLanguageTag.isBlank()) {
                            tr("settings.language.system")
                        } else {
                            appLanguageDisplayName(appLanguageTag)
                        },
                        color = MaterialTheme.colorScheme.primary,
                    )
                },
            )
        }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.Visibility,
                title = tr("settings.appearance.showSenderImages"),
                hint = tr("settings.appearance.showSenderImagesHint"),
                checked = showSenderImages,
                onToggle = onToggleSenderImages,
            )
        }
        item {
            SettingsRow(
                icon = Icons.Filled.FormatSize,
                title = tr("settings.appearance.messageTextSize"),
                onClick = onOpenMessageTextSize,
                trailing = {
                    Text(
                        trf("settings.appearance.textSizeValue", messageFontScale),
                        color = MaterialTheme.colorScheme.primary,
                    )
                },
            )
        }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.Inbox,
                title = tr("settings.appearance.showUnreadAccountBadge"),
                hint = tr("settings.appearance.showUnreadAccountBadgeHint"),
                checked = showUnreadBadges,
                onToggle = onToggleUnreadBadges,
            )
        }

        item { SettingsSectionLabel(tr("settings.sections.sideNav")) }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.Inbox,
                title = tr("settings.sideNav.showUnifiedInbox"),
                checked = showUnifiedInboxNav,
                onToggle = onToggleUnifiedInboxNav,
            )
        }

        item { SettingsSectionLabel(tr("settings.sections.kanban")) }
        item {
            SettingsRow(
                icon = Icons.Filled.ViewKanban,
                title = tr("settings.kanban.columnWidth"),
                onClick = onCycleKanbanColumnWidth,
                trailing = { Text(trf("settings.kanban.columnWidthValue", kanbanColumnWidth), color = MaterialTheme.colorScheme.primary) },
            )
        }

        item { SettingsSectionLabel(tr("settings.sections.composer")) }
        item {
            SettingsRow(
                icon = Icons.AutoMirrored.Filled.Send,
                title = tr("settings.composer.sendMessageWith"),
                onClick = onToggleSendShortcut,
                trailing = { Text(sendShortcutMode.label(), color = MaterialTheme.colorScheme.primary) },
            )
        }

        item { SettingsSectionLabel(tr("settings.sections.signature")) }
        item {
            SettingsSignatureRow(
                html = appSignatureHtml,
                onSave = onSaveAppSignature,
            )
        }

        item { SettingsSectionLabel(tr("settings.sections.privacy")) }
        item {
            SettingsRow(
                icon = Icons.Filled.HideImage,
                title = tr("settings.privacy.remoteSenders"),
                hint = tr("settings.privacy.remoteSendersHint"),
                onClick = onOpenRemoteSenders,
                trailing = {
                    Text(
                        if (remoteImageSenderCount == 0) {
                            tr("settings.privacy.remoteSendersNone")
                        } else {
                            tr("settings.privacy.remoteSendersCount", mapOf("count" to remoteImageSenderCount))
                        },
                        color = MaterialTheme.colorScheme.primary,
                    )
                },
            )
        }

        item { SettingsSectionLabel(tr("settings.sections.network")) }
        item {
            SettingsProxyRow(
                spec = appProxy,
                accountScoped = false,
                onSave = onSaveAppProxy,
            )
        }

        item { SettingsSectionLabel(tr("settings.syncNotifications")) }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.Refresh,
                title = tr("settings.backgroundSync"),
                hint = tr("settings.backgroundSyncHint"),
                checked = backgroundSyncEnabled,
                onToggle = onToggleBackgroundSync,
            )
        }
        if (supportsBackgroundPush) {
            // Android: a real background channel keeps mail fresh while suspended.
            item {
                SettingsToggleRow(
                    icon = Icons.Filled.MarkEmailUnread,
                    title = tr("settings.liveMailPush"),
                    hint = tr("settings.liveMailPushHint"),
                    checked = liveMailPushEnabled,
                    onToggle = onToggleLiveMailPush,
                )
            }
            item {
                SettingsRow(
                    icon = Icons.Filled.Refresh,
                    title = tr("settings.refreshBackground"),
                    onClick = onRefreshBackground,
                )
            }
        } else {
            // iOS: no persistent background socket. Offer a foreground poll
            // interval instead; background checks fall back to best-effort.
            item {
                SettingsRow(
                    icon = Icons.Filled.Refresh,
                    title = tr("settings.pollInterval"),
                    hint = tr("settings.pollIntervalHint"),
                    onClick = onCyclePollInterval,
                    trailing = {
                        Text(
                            if (pollIntervalMinutes <= 0) {
                                tr("settings.pollIntervalOff")
                            } else {
                                tr("settings.pollIntervalValue", mapOf("minutes" to pollIntervalMinutes))
                            },
                            color = MaterialTheme.colorScheme.primary,
                        )
                    },
                )
            }
        }
        if (notificationsNeedPermission) {
            item {
                SettingsRow(
                    icon = Icons.Filled.MarkEmailUnread,
                    title = tr("mobile.accounts.enableNotifications"),
                    onClick = onEnableNotifications,
                )
            }
        }
        item {
            SettingsRow(
                icon = Icons.Filled.BugReport,
                title = tr("settings.viewSyncLog"),
                hint = tr("settings.syncDiagnosticLogHint"),
                onClick = onOpenSyncLog,
            )
        }

        item { SettingsSectionLabel(tr("settings.sections.backup")) }
        item {
            SettingsRow(
                icon = Icons.Filled.Save,
                title = tr("settings.backup.exportTitle"),
                hint = tr("settings.backup.fileHint"),
                onClick = { if (!backupBusy) onExportBackup() },
            )
        }
        item {
            SettingsRow(
                icon = Icons.Filled.Restore,
                title = tr("settings.backup.restoreTitle"),
                subtitle = tr("settings.backup.exportSubtitle"),
                onClick = { if (!backupBusy) onRestoreBackup() },
            )
        }

        item { SettingsSectionLabel(tr("settings.sections.storage")) }
        item {
            SettingsRow(
                icon = Icons.Filled.Info,
                title = tr("settings.storage.usageTitle"),
                subtitle =
                    storageUsage?.let {
                        trf(
                            "settings.storage.usageSummary",
                            formatBytes(it.cacheBytes),
                            formatBytes(it.dbBytes),
                        )
                    } ?: if (storageBusy) tr("common.loading") else tr("settings.storage.tapToRefresh"),
                onClick = onRefreshStorage,
            )
        }
        item {
            SettingsRow(
                icon = Icons.Filled.Delete,
                title = if (storageClearConfirming) tr("mobile.accounts.confirmClearCache") else tr("settings.storage.clearTitle"),
                subtitle = if (storageBusy) tr("settings.storage.working") else tr("settings.storage.clearCachedAttachmentsOnly"),
                onClick = onClearStorageCache,
                trailing = storageUsage?.cacheBytes?.takeIf { it > 0 }?.let { { Text(formatBytes(it)) } },
            )
        }

        item { Spacer(Modifier.height(24.dp)) }
    }
}

@Composable
internal fun SettingsSectionLabel(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 18.dp, bottom = 2.dp),
    )
}

@Composable
internal fun SettingsEmptyLabel(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

@Composable
internal fun SettingsAccountRow(
    account: AccountSummary,
    hidden: Boolean,
    fallbackTitle: String? = null,
    showAccountIdFallback: Boolean = true,
    showDefaultSubtitle: Boolean = true,
    onClick: () -> Unit,
) {
    val label =
        if (showAccountIdFallback) {
            account.displayName.ifBlank { account.email.ifBlank { account.id } }
        } else {
            account.displayName.ifBlank {
                fallbackTitle ?: account.id.takeIf { showAccountIdFallback }.orEmpty()
            }
        }
    SettingsRow(
        icon = if (accountSummaryIsRss(account)) Icons.Filled.RssFeed else Icons.Filled.Inbox,
        leading = {
            AccountBadgeAvatar(
                label = label,
                avatarUrl = account.avatarUrl,
                size = 40.dp,
                fallbackIcon = accountAvatarFallbackIcon(account),
            )
        },
        title = label,
        subtitle =
            listOfNotNull(
                account.email.takeIf { showAccountIdFallback && it.isNotBlank() && it != label },
                if (hidden) tr("settings.account.hiddenFromNavigation") else null,
            ).joinToString(" · ").ifBlank {
                tr("settings.account.accountSettings").takeIf { showDefaultSubtitle }
            },
        onClick = onClick,
    )
}

@Composable
internal fun SettingsRow(
    icon: ImageVector,
    title: String,
    subtitle: String? = null,
    hint: String? = null,
    onClick: () -> Unit,
    leading: (@Composable () -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
    destructive: Boolean = false,
) {
    val accent = if (destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface
    ListItem(
        headlineContent = { SettingsHeadline(title = title, hint = hint, color = accent) },
        supportingContent = subtitle?.let { { Text(it, maxLines = 2, overflow = TextOverflow.Ellipsis) } },
        leadingContent =
            leading ?: {
                Icon(
                    icon,
                    contentDescription = null,
                    tint = if (destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            },
        trailingContent = trailing,
        modifier = Modifier.clickable(onClick = onClick),
    )
}

// Full-width text input styled to align with the list rows in the detail forms.
@Composable
internal fun SettingsTextRow(
    value: String,
    label: String,
    onValueChange: (String) -> Unit,
    supporting: String? = null,
    singleLine: Boolean = true,
    minLines: Int = 1,
    keyboardDigits: Boolean = false,
) {
    // Sit the field on a full-width surface strip so the row reads as a white
    // section, matching the ListItem-based rows above and below it.
    Surface(color = MaterialTheme.colorScheme.surface, modifier = Modifier.fillMaxWidth()) {
        OutlinedTextField(
            value = value,
            onValueChange = onValueChange,
            label = { Text(label) },
            supportingText = supporting?.let { { Text(it) } },
            singleLine = singleLine,
            minLines = minLines,
            keyboardOptions =
                if (keyboardDigits) {
                    nativeTextKeyboardOptions.copy(keyboardType = KeyboardType.Number)
                } else {
                    nativeTextKeyboardOptions
                },
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
        )
    }
}

@Composable
internal fun SettingsToggleRow(
    icon: ImageVector,
    title: String,
    subtitle: String? = null,
    hint: String? = null,
    checked: Boolean,
    onToggle: () -> Unit,
) {
    ListItem(
        headlineContent = { SettingsHeadline(title = title, hint = hint) },
        supportingContent = subtitle?.let { { Text(it, maxLines = 2, overflow = TextOverflow.Ellipsis) } },
        leadingContent = { Icon(icon, contentDescription = null) },
        trailingContent = {
            Switch(
                checked = checked,
                onCheckedChange = { onToggle() },
                colors = settingsSwitchColors(),
            )
        },
        modifier = Modifier.clickable(onClick = onToggle),
    )
}

@Composable
private fun SettingsHeadline(
    title: String,
    hint: String?,
    color: Color = MaterialTheme.colorScheme.onSurface,
) {
    var showHint by remember { mutableStateOf(false) }
    Row(verticalAlignment = Alignment.CenterVertically) {
        Text(
            title,
            color = color,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f, fill = false),
        )
        if (hint != null) {
            IconButton(onClick = { showHint = true }, modifier = Modifier.size(32.dp)) {
                Icon(
                    Icons.Filled.Info,
                    contentDescription = hint,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(18.dp),
                )
            }
        }
    }
    if (showHint && hint != null) {
        AlertDialog(
            onDismissRequest = { showHint = false },
            title = { Text(title) },
            text = { Text(hint) },
            confirmButton = {
                TextButton(onClick = { showHint = false }) { Text(tr("common.close")) }
            },
        )
    }
}

@Composable
private fun settingsSwitchColors() =
    with(MaterialTheme.colorScheme) {
        val isDark = background.luminance() < 0.5f
        val uncheckedTrack = if (isDark) Color(0xFF475569) else Color(0xFFCBD5E1)
        val uncheckedThumb = if (isDark) Color(0xFFE2E8F0) else Color.White
        SwitchDefaults.colors(
            checkedThumbColor = onPrimary,
            checkedTrackColor = primary,
            uncheckedThumbColor = uncheckedThumb,
            uncheckedTrackColor = uncheckedTrack,
            uncheckedBorderColor = outline,
            disabledCheckedThumbColor = onPrimary.copy(alpha = 0.38f),
            disabledCheckedTrackColor = primary.copy(alpha = 0.38f),
            disabledUncheckedThumbColor = uncheckedThumb.copy(alpha = 0.55f),
            disabledUncheckedTrackColor = uncheckedTrack.copy(alpha = 0.38f),
            disabledUncheckedBorderColor = outline.copy(alpha = 0.38f),
        )
    }
