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
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Reply
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.BugReport
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Code
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.FilterList
import androidx.compose.material.icons.filled.Image
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.MarkEmailUnread
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.NotificationsOff
import androidx.compose.material.icons.filled.OpenInFull
import androidx.compose.material.icons.filled.PauseCircle
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
import androidx.compose.material3.SwitchDefaults
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
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.backhandler.BackHandler
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.luminance
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
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
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
import jp.nonbili.meron.shared.SharedMobileContract
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
import jp.nonbili.meron.ui.resources.Res
import jp.nonbili.meron.ui.resources.wp_aurora
import jp.nonbili.meron.ui.resources.wp_breeze
import jp.nonbili.meron.ui.resources.wp_desert
import jp.nonbili.meron.ui.resources.wp_forest
import jp.nonbili.meron.ui.resources.wp_galaxy
import jp.nonbili.meron.ui.resources.wp_marble
import jp.nonbili.meron.ui.resources.wp_mountain
import jp.nonbili.meron.ui.resources.wp_nebula
import jp.nonbili.meron.ui.resources.wp_ocean
import jp.nonbili.meron.ui.resources.wp_raindrops
import jp.nonbili.meron.ui.resources.wp_sakura
import jp.nonbili.meron.ui.resources.wp_shapes
import jp.nonbili.meron.ui.resources.wp_sunset
import jp.nonbili.meron.ui.resources.wp_vintage
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.jetbrains.compose.resources.DrawableResource
import org.jetbrains.compose.resources.painterResource
import kotlin.math.abs

@OptIn(ExperimentalMaterial3Api::class, ExperimentalComposeUiApi::class)
@Composable
internal fun SettingsScreen(
    onBack: () -> Unit,
    initialAccountId: String?,
    onConsumeInitialAccount: () -> Unit,
    initialKanbanBoardId: String?,
    onConsumeInitialKanbanBoard: () -> Unit,
    accounts: List<AccountSummary>,
    hiddenNavigationAccountIds: Set<String>,
    kanbanBoards: List<KanbanBoardSpec>,
    activeKanbanBoardId: String,
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
    showStarredNav: Boolean,
    onToggleStarredNav: () -> Unit,
    sendShortcutMode: SendShortcutMode,
    onToggleSendShortcut: () -> Unit,
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
    syncDiagnosticLogEnabled: Boolean,
    onToggleSyncDiagnosticLog: () -> Unit,
    onShareDiagnosticLog: () -> Unit,
    pollIntervalMinutes: Int,
    onCyclePollInterval: () -> Unit,
    storageUsage: StorageUsage?,
    storageBusy: Boolean,
    storageClearConfirming: Boolean,
    onRefreshStorage: () -> Unit,
    onClearStorageCache: () -> Unit,
) {
    var showThemePicker by remember { mutableStateOf(false) }
    var showLanguagePicker by remember { mutableStateOf(false) }
    val settingsNavController = rememberNavController()
    val settingsBackStackEntry by settingsNavController.currentBackStackEntryAsState()
    var selectedSettingsAccountId by remember { mutableStateOf<String?>(null) }
    var selectedSettingsBoardId by remember { mutableStateOf<String?>(null) }
    var directOpenRoute by remember { mutableStateOf<String?>(null) }
    val page =
        when (settingsBackStackEntry?.destination?.route ?: SettingsRoutes.Root) {
            SettingsRoutes.General -> SettingsPage.General
            SettingsRoutes.Account -> selectedSettingsAccountId?.let { SettingsPage.AccountDetail(it) } ?: SettingsPage.Root
            SettingsRoutes.AccountWallpaper -> selectedSettingsAccountId?.let { SettingsPage.AccountWallpaper(it) } ?: SettingsPage.Root
            SettingsRoutes.KanbanBoard -> selectedSettingsBoardId?.let { SettingsPage.KanbanBoardDetail(it) } ?: SettingsPage.Root
            SettingsRoutes.KanbanBoardWallpaper -> selectedSettingsBoardId?.let { SettingsPage.KanbanBoardWallpaper(it) } ?: SettingsPage.Root
            else -> SettingsPage.Root
        }
    LaunchedEffect(initialAccountId) {
        if (!initialAccountId.isNullOrBlank()) {
            selectedSettingsAccountId = initialAccountId
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
                        },
                    )
                },
                navigationIcon = {
                    IconButton(onClick = handleBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = tr("buttons.back"))
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
                    showStarredNav = showStarredNav,
                    onToggleStarredNav = onToggleStarredNav,
                    kanbanColumnWidth = kanbanColumnWidth,
                    onCycleKanbanColumnWidth = onCycleKanbanColumnWidth,
                    sendShortcutMode = sendShortcutMode,
                    onToggleSendShortcut = onToggleSendShortcut,
                    notificationsNeedPermission = notificationsNeedPermission,
                    onEnableNotifications = onEnableNotifications,
                    supportsBackgroundPush = supportsBackgroundPush,
                    liveMailPushEnabled = liveMailPushEnabled,
                    onToggleLiveMailPush = onToggleLiveMailPush,
                    backgroundSyncEnabled = backgroundSyncEnabled,
                    onToggleBackgroundSync = onToggleBackgroundSync,
                    onRefreshBackground = onRefreshBackground,
                    syncDiagnosticLogEnabled = syncDiagnosticLogEnabled,
                    onToggleSyncDiagnosticLog = onToggleSyncDiagnosticLog,
                    onShareDiagnosticLog = onShareDiagnosticLog,
                    pollIntervalMinutes = pollIntervalMinutes,
                    onCyclePollInterval = onCyclePollInterval,
                    storageUsage = storageUsage,
                    storageBusy = storageBusy,
                    storageClearConfirming = storageClearConfirming,
                    onRefreshStorage = onRefreshStorage,
                    onClearStorageCache = onClearStorageCache,
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
                        onPickAvatar = { onPickAccountAvatar(account) },
                        onOpenWallpaper = { settingsNavController.navigate(SettingsRoutes.AccountWallpaper) },
                        onMoveUp = { onMoveAccountUp(account) },
                        onMoveDown = { onMoveAccountDown(account) },
                        onRemove = { onRemoveAccount(account) },
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
                        active = board.id == activeKanbanBoardId,
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

            composable(SettingsRoutes.Root) {
                // Mirrors the desktop Settings sidebar: a single "General" entry,
                // then Kanban boards, Mail accounts, and Feed accounts sections.
                val mailAccounts = accounts.filter { !accountSummaryIsRss(it) }
                val feedAccounts = accounts.filter { accountSummaryIsRss(it) }
                LazyColumn(Modifier.fillMaxSize()) {
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

private sealed class SettingsPage {
    data object Root : SettingsPage()

    data object General : SettingsPage()

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
}

private object SettingsRoutes {
    const val Root = "settings/root"
    const val General = "settings/general"
    const val Account = "settings/account"
    const val AccountWallpaper = "settings/account-wallpaper"
    const val KanbanBoard = "settings/kanban-board"
    const val KanbanBoardWallpaper = "settings/kanban-board-wallpaper"
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
    showStarredNav: Boolean,
    onToggleStarredNav: () -> Unit,
    kanbanColumnWidth: Int,
    onCycleKanbanColumnWidth: () -> Unit,
    sendShortcutMode: SendShortcutMode,
    onToggleSendShortcut: () -> Unit,
    notificationsNeedPermission: Boolean,
    onEnableNotifications: () -> Unit,
    supportsBackgroundPush: Boolean,
    liveMailPushEnabled: Boolean,
    onToggleLiveMailPush: () -> Unit,
    backgroundSyncEnabled: Boolean,
    onToggleBackgroundSync: () -> Unit,
    onRefreshBackground: () -> Unit,
    syncDiagnosticLogEnabled: Boolean,
    onToggleSyncDiagnosticLog: () -> Unit,
    onShareDiagnosticLog: () -> Unit,
    pollIntervalMinutes: Int,
    onCyclePollInterval: () -> Unit,
    storageUsage: StorageUsage?,
    storageBusy: Boolean,
    storageClearConfirming: Boolean,
    onRefreshStorage: () -> Unit,
    onClearStorageCache: () -> Unit,
    modifier: Modifier = Modifier,
) {
    LazyColumn(modifier) {
        item { SettingsSectionLabel(tr("settings.pages.appearance")) }
        item {
            val displayedAppearanceMode =
                if (appearanceMode == AppAppearanceMode.System) AppAppearanceMode.Indigo else appearanceMode
            SettingsRow(
                icon = Icons.Filled.Visibility,
                title = tr("common.theme"),
                subtitle = tr("settings.generalThemeHint"),
                onClick = onOpenTheme,
                trailing = { Text(displayedAppearanceMode.label, color = MaterialTheme.colorScheme.primary) },
            )
        }
        item {
            SettingsRow(
                icon = Icons.Filled.Settings,
                title = tr("settings.language.label"),
                subtitle = tr("settings.language.hint"),
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
                subtitle = tr("settings.appearance.showSenderImagesHint"),
                checked = showSenderImages,
                onToggle = onToggleSenderImages,
            )
        }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.Inbox,
                title = tr("settings.appearance.showUnreadAccountBadge"),
                subtitle = tr("settings.appearance.showUnreadAccountBadgeHint"),
                checked = showUnreadBadges,
                onToggle = onToggleUnreadBadges,
            )
        }

        item { SettingsSectionLabel(tr("settings.sections.sideNav")) }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.Inbox,
                title = tr("settings.sideNav.showUnifiedInbox"),
                subtitle = tr("settings.navigationDrawerHint"),
                checked = showUnifiedInboxNav,
                onToggle = onToggleUnifiedInboxNav,
            )
        }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.Star,
                title = tr("settings.sideNav.showStarred"),
                subtitle = tr("settings.navigationDrawerHint"),
                checked = showStarredNav,
                onToggle = onToggleStarredNav,
            )
        }

        item { SettingsSectionLabel(tr("settings.sections.kanban")) }
        item {
            SettingsRow(
                icon = Icons.Filled.ViewKanban,
                title = tr("settings.kanban.columnWidth"),
                subtitle = tr("settings.kanban.columnWidthHint"),
                onClick = onCycleKanbanColumnWidth,
                trailing = { Text(trf("settings.kanban.columnWidthValue", kanbanColumnWidth), color = MaterialTheme.colorScheme.primary) },
            )
        }

        item { SettingsSectionLabel(tr("settings.sections.composer")) }
        item {
            SettingsRow(
                icon = Icons.AutoMirrored.Filled.Send,
                title = tr("settings.composer.sendMessageWith"),
                subtitle = tr("settings.composer.sendShortcutHint"),
                onClick = onToggleSendShortcut,
                trailing = { Text(sendShortcutMode.label(), color = MaterialTheme.colorScheme.primary) },
            )
        }

        item { SettingsSectionLabel(tr("settings.syncNotifications")) }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.Refresh,
                title = tr("settings.backgroundSync"),
                subtitle = tr("settings.backgroundSyncHint"),
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
                    subtitle = tr("settings.liveMailPushHint"),
                    checked = liveMailPushEnabled,
                    onToggle = onToggleLiveMailPush,
                )
            }
            item {
                SettingsRow(
                    icon = Icons.Filled.Refresh,
                    title = tr("settings.refreshBackground"),
                    subtitle = tr("settings.refreshBackgroundHint"),
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
                    subtitle = tr("settings.pollIntervalHint"),
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
        item {
            SettingsToggleRow(
                icon = Icons.Filled.BugReport,
                title = tr("settings.syncDiagnosticLog"),
                subtitle = tr("settings.syncDiagnosticLogHint"),
                checked = syncDiagnosticLogEnabled,
                onToggle = onToggleSyncDiagnosticLog,
            )
        }
        if (syncDiagnosticLogEnabled) {
            item {
                SettingsRow(
                    icon = Icons.AutoMirrored.Filled.Send,
                    title = tr("settings.shareSyncLog"),
                    subtitle = tr("settings.shareSyncLogHint"),
                    onClick = onShareDiagnosticLog,
                )
            }
        }
        if (notificationsNeedPermission) {
            item {
                SettingsRow(
                    icon = Icons.Filled.MarkEmailUnread,
                    title = tr("mobile.accounts.enableNotifications"),
                    subtitle = tr("settings.notificationsEnableHint"),
                    onClick = onEnableNotifications,
                )
            }
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
internal fun SettingsAccountDetailPage(
    account: AccountSummary,
    canMoveUp: Boolean,
    canMoveDown: Boolean,
    showInNavigation: Boolean,
    onSave: (
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
    onPickAvatar: () -> Unit,
    onOpenWallpaper: () -> Unit,
    onMoveUp: () -> Unit,
    onMoveDown: () -> Unit,
    onRemove: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val isRss = accountSummaryIsRss(account)
    var displayName by remember(account.id) { mutableStateOf(account.displayName) }
    var senderName by remember(account.id) { mutableStateOf(account.senderName) }
    var avatarUrl by remember(account.id) { mutableStateOf(account.avatarUrl) }
    var wallpaperPresetId by remember(account.id) { mutableStateOf(account.chatWallpaperPresetId) }
    var wallpaperCustomUrl by remember(account.id, account.chatWallpaperKind, account.chatWallpaperUrl) {
        mutableStateOf(if (account.chatWallpaperKind == "custom") account.chatWallpaperUrl else "")
    }
    var loadRemoteImages by remember(account.id) { mutableStateOf(account.loadRemoteImages || isRss) }
    var conversationHtml by remember(account.id) { mutableStateOf(account.conversationHtml) }
    var includedInUnified by remember(account.id) { mutableStateOf(account.includedInUnified) }
    var visibleInNavigation by remember(account.id, showInNavigation) { mutableStateOf(showInNavigation) }
    var muted by remember(account.id) { mutableStateOf(account.muted) }
    var paused by remember(account.id) { mutableStateOf(account.paused) }
    var intervalText by remember(account.id) { mutableStateOf(account.rssSyncIntervalMinutes.toString()) }
    var aliasEntries by remember(account.id) {
        mutableStateOf(account.aliases.map { it.email to it.name })
    }
    var confirmRemove by remember(account.id) { mutableStateOf(false) }
    val accountTitle =
        if (isRss) {
            displayName.ifBlank { tr("accounts.rssAtomFeeds") }
        } else {
            displayName.ifBlank { account.email.ifBlank { account.id } }
        }
    val accountSubtitle = if (isRss) "" else account.email.ifBlank { account.id }

    // Autosave: every control persists immediately, matching the desktop panel.
    // The lambda reads the live state values at call time, so flipping a toggle
    // then calling persist() writes the just-updated value.
    val persist = {
        onSave(
            displayName,
            senderName,
            avatarUrl,
            wallpaperPresetId,
            loadRemoteImages,
            conversationHtml,
            includedInUnified,
            visibleInNavigation,
            muted,
            paused,
            intervalText.toIntOrNull()?.coerceIn(5, 1440) ?: account.rssSyncIntervalMinutes.coerceIn(5, 1440),
            aliasEntries
                .filter { it.first.isNotBlank() }
                .joinToString("\n") { (email, name) ->
                    if (name.isBlank()) email else "$email, $name"
                },
        )
    }

    if (confirmRemove) {
        AlertDialog(
            onDismissRequest = { confirmRemove = false },
            title = { Text(tr("settings.account.removeAccountTitle")) },
            text = { Text(trf("settings.account.removeAccountText", accountTitle)) },
            confirmButton = {
                TextButton(onClick = {
                    confirmRemove = false
                    onRemove()
                }) {
                    Text(tr("settings.account.removeAccount"), color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmRemove = false }) { Text(tr("buttons.cancel")) }
            },
        )
    }
    LazyColumn(modifier) {
        item {
            Row(
                modifier = Modifier.fillMaxWidth().padding(16.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                AccountAvatarEditor(
                    name = accountTitle,
                    url = avatarUrl,
                    onPick = onPickAvatar,
                )
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        accountTitle,
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    if (accountSubtitle.isNotBlank()) {
                        Text(
                            accountSubtitle,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
        }

        item { SettingsSectionLabel(tr("settings.account.profile")) }
        item {
            SettingsTextRow(
                value = displayName,
                label = if (isRss) tr("settings.account.feedGroupName") else tr("accounts.fields.accountName"),
                onValueChange = {
                    displayName = it
                    persist()
                },
            )
        }
        if (!isRss) {
            item {
                SettingsTextRow(
                    value = senderName,
                    label = tr("settings.account.senderName"),
                    onValueChange = {
                        senderName = it
                        persist()
                    },
                )
            }
        }
        item { SettingsSectionLabel(tr("settings.account.chatBackground")) }
        item {
            SettingsRow(
                icon = Icons.Filled.Image,
                title = tr("settings.account.chatBackground"),
                subtitle =
                    if (wallpaperCustomUrl.isNotBlank()) {
                        tr("wallpaper.customBackground")
                    } else {
                        wallpaperPresetDisplayName(wallpaperPresetId)
                    },
                onClick = onOpenWallpaper,
                trailing = {
                    WallpaperRowPreview(
                        presetId = wallpaperPresetId,
                        customUrl = wallpaperCustomUrl,
                    )
                },
            )
        }

        item { SettingsSectionLabel(tr("settings.account.visibility")) }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.Inbox,
                title = tr("settings.account.showInUnifiedInbox"),
                subtitle = null,
                checked = includedInUnified,
            ) {
                includedInUnified = !includedInUnified
                persist()
            }
        }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.Visibility,
                title = tr("settings.account.showInSideNav"),
                subtitle = null,
                checked = visibleInNavigation,
            ) {
                visibleInNavigation = !visibleInNavigation
                persist()
            }
        }

        item { SettingsSectionLabel(tr("settings.account.notificationsSync")) }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.NotificationsOff,
                title = tr("settings.account.muteNotifications"),
                subtitle = null,
                checked = muted,
            ) {
                muted = !muted
                persist()
            }
        }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.PauseCircle,
                title = tr("settings.account.pauseAccount"),
                subtitle = null,
                checked = paused,
            ) {
                paused = !paused
                persist()
            }
        }
        if (isRss) {
            item {
                SettingsTextRow(
                    value = intervalText,
                    label = tr("settings.account.syncIntervalMinutes"),
                    supporting = tr("settings.account.syncIntervalRange"),
                    keyboardDigits = true,
                    onValueChange = {
                        intervalText = it.filter(Char::isDigit).take(4)
                        persist()
                    },
                )
            }
        }

        item { SettingsSectionLabel(tr("settings.account.content")) }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.Image,
                title = tr("settings.account.loadRemoteImages"),
                subtitle = null,
                checked = loadRemoteImages,
            ) {
                loadRemoteImages = !loadRemoteImages
                persist()
            }
        }
        item {
            SettingsToggleRow(
                icon = Icons.Filled.Code,
                title = tr("settings.account.renderHtmlMessages"),
                subtitle = null,
                checked = conversationHtml,
            ) {
                conversationHtml = !conversationHtml
                persist()
            }
        }

        if (!isRss) {
            item { SettingsSectionLabel(tr("settings.account.aliases")) }
            itemsIndexed(aliasEntries) { index, (email, name) ->
                AliasEditorRow(
                    email = email,
                    name = name,
                    onEmailChange = { value ->
                        aliasEntries = aliasEntries.toMutableList().also { it[index] = value to it[index].second }
                        persist()
                    },
                    onNameChange = { value ->
                        aliasEntries = aliasEntries.toMutableList().also { it[index] = it[index].first to value }
                        persist()
                    },
                    onRemove = {
                        aliasEntries = aliasEntries.toMutableList().also { it.removeAt(index) }
                        persist()
                    },
                )
            }
            item {
                SettingsRow(
                    icon = Icons.Filled.Add,
                    title = tr("settings.account.addAlias"),
                    subtitle = tr("settings.account.addAliasHint"),
                    onClick = { aliasEntries = aliasEntries + ("" to "") },
                )
            }
        }

        if (canMoveUp || canMoveDown) {
            item { SettingsSectionLabel(tr("settings.account.order")) }
            if (canMoveUp) {
                item {
                    SettingsRow(
                        icon = Icons.Filled.KeyboardArrowUp,
                        title = tr("mobile.accounts.moveUp"),
                        subtitle = null,
                        onClick = onMoveUp,
                    )
                }
            }
            if (canMoveDown) {
                item {
                    SettingsRow(
                        icon = Icons.Filled.KeyboardArrowDown,
                        title = tr("mobile.accounts.moveDown"),
                        subtitle = null,
                        onClick = onMoveDown,
                    )
                }
            }
        }

        item { SettingsSectionLabel(tr("settings.dangerZone")) }
        item {
            SettingsRow(
                icon = Icons.Filled.Delete,
                title = tr("settings.account.removeAccount"),
                subtitle = tr("settings.account.deleteCachedMailHint"),
                onClick = { confirmRemove = true },
                destructive = true,
            )
        }
        item { Spacer(Modifier.height(24.dp)) }
    }
}

// Mirrors frontend/src/lib/wallpapers.ts WALLPAPER_PRESETS. Photographic
// presets resolve to bundled drawables (see wallpaperImageRes); the rest are
// drawn as approximations.
private val wallpaperPresets =
    listOf(
        "plain" to "Plain",
        "doodle" to "Doodle",
        "dots" to "Linear Dots",
        "grid" to "Classic Grid",
        "stripes" to "Diagonal Stripes",
        "hexagon" to "Hexagon Grid",
        "isometric" to "Isometric Cubes",
        "waves" to "Flowing Waves",
        "nordic" to "Nordic Pattern",
        "topography" to "Topography",
        "constellation" to "Constellation",
        "aurora" to "Aurora",
        "nebula" to "Nebula",
        "sunset" to "Sunset Glow",
        "forest" to "Forest Mist",
        "desert" to "Desert Dunes",
        "ocean" to "Tranquil Ocean",
        "mountain" to "Mountain Range",
        "breeze" to "Soft Breeze",
        "galaxy" to "Spiral Galaxy",
        "shapes" to "Abstract Shapes",
        "sakura" to "Sakura Watercolor",
        "vintage" to "Vintage Parchment",
        "raindrops" to "Raindrops",
        "marble" to "Sleek Marble",
        "cyberpunk" to "Cyberpunk Grid",
        "matrix" to "Digital Matrix",
        "autumn" to "Autumn Leaves",
        "nightsky" to "Celestial Night",
    )

// Photographic presets ship as bundled drawables so the preview matches desktop.
private fun wallpaperImageRes(presetId: String): DrawableResource? =
    when (presetId) {
        "aurora" -> Res.drawable.wp_aurora
        "nebula" -> Res.drawable.wp_nebula
        "sunset" -> Res.drawable.wp_sunset
        "forest" -> Res.drawable.wp_forest
        "desert" -> Res.drawable.wp_desert
        "ocean" -> Res.drawable.wp_ocean
        "mountain" -> Res.drawable.wp_mountain
        "breeze" -> Res.drawable.wp_breeze
        "galaxy" -> Res.drawable.wp_galaxy
        "shapes" -> Res.drawable.wp_shapes
        "sakura" -> Res.drawable.wp_sakura
        "vintage" -> Res.drawable.wp_vintage
        "raindrops" -> Res.drawable.wp_raindrops
        "marble" -> Res.drawable.wp_marble
        else -> null
    }

private fun normalizedWallpaperPresetId(presetId: String): String = presetId.ifBlank { "plain" }

private fun wallpaperPresetDisplayName(presetId: String): String = wallpaperPresets.firstOrNull { it.first == normalizedWallpaperPresetId(presetId) }?.second ?: "Plain"

@Composable
internal fun WallpaperPickerPage(
    selected: String,
    previewPresetId: String,
    previewCustomUrl: String,
    onSelect: (String) -> Unit,
    onUpload: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val normalizedSelected = if (selected == "__custom") selected else normalizedWallpaperPresetId(selected)
    val normalizedPreviewPresetId = normalizedWallpaperPresetId(previewPresetId)
    var localSelected by remember(selected, previewPresetId, previewCustomUrl) { mutableStateOf(normalizedSelected) }
    var localPresetId by remember(selected, previewPresetId, previewCustomUrl) { mutableStateOf(normalizedPreviewPresetId) }
    var localCustomUrl by remember(selected, previewPresetId, previewCustomUrl) { mutableStateOf(previewCustomUrl) }
    LazyColumn(modifier) {
        item {
            ChatWallpaperPreview(
                presetId = localPresetId,
                customUrl = localCustomUrl,
            )
        }
        item {
            WallpaperPresetGrid(
                selected = localSelected,
                onSelect = {
                    localSelected = it
                    localPresetId = it
                    localCustomUrl = ""
                    onSelect(it)
                },
                onUpload = onUpload,
                uploadLabel = tr("wallpaper.uploadCustom"),
            )
        }
        item { Spacer(Modifier.height(24.dp)) }
    }
}

@Composable
private fun WallpaperRowPreview(
    presetId: String,
    customUrl: String,
) {
    Box(
        modifier =
            Modifier
                .size(width = 56.dp, height = 40.dp)
                .clip(RoundedCornerShape(10.dp))
                .border(1.dp, MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(10.dp)),
    ) {
        ChatWallpaperBackground(
            presetId = presetId,
            customUrl = customUrl,
            modifier = Modifier.matchParentSize(),
            fallback = MaterialTheme.colorScheme.surface,
        )
    }
}

@Composable
internal fun WallpaperPresetGrid(
    selected: String,
    onSelect: (String) -> Unit,
    onUpload: () -> Unit,
    uploadLabel: String,
) {
    val rows =
        remember {
            (listOf<Pair<String, String>?>(null) + wallpaperPresets.map { it.first to it.second }).chunked(3)
        }
    Column(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        rows.forEach { row ->
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                row.forEach { preset ->
                    if (preset == null) {
                        WallpaperUploadTile(
                            label = uploadLabel,
                            onClick = onUpload,
                            modifier = Modifier.weight(1f),
                        )
                    } else {
                        val (id, label) = preset
                        WallpaperPresetTile(
                            presetId = id,
                            label = label,
                            selected = selected == id,
                            onClick = { onSelect(id) },
                            modifier = Modifier.weight(1f),
                        )
                    }
                }
                repeat(3 - row.size) {
                    Spacer(Modifier.weight(1f))
                }
            }
        }
    }
}

@Composable
private fun WallpaperUploadTile(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val shape = RoundedCornerShape(14.dp)
    Box(
        modifier =
            modifier
                .aspectRatio(1.58f)
                .clip(shape)
                .border(1.dp, MaterialTheme.colorScheme.outlineVariant, shape)
                .clickable(onClick = onClick)
                .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Icon(
                Icons.Filled.Image,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(20.dp),
            )
            Text(
                label,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
private fun WallpaperPresetTile(
    presetId: String,
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val shape = RoundedCornerShape(14.dp)
    val borderColor = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant
    Box(
        modifier =
            modifier
                .aspectRatio(1.58f)
                .clip(shape)
                .border(if (selected) 2.dp else 1.dp, borderColor, shape)
                .clickable(onClick = onClick)
                .semantics { contentDescription = label },
    ) {
        ChatWallpaperBackground(
            presetId = presetId,
            customUrl = "",
            modifier = Modifier.matchParentSize(),
            fallback = MaterialTheme.colorScheme.surface,
        )
        if (selected) {
            Box(
                modifier =
                    Modifier
                        .align(Alignment.TopEnd)
                        .padding(6.dp)
                        .size(22.dp)
                        .clip(CircleShape)
                        .background(MaterialTheme.colorScheme.primary),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    Icons.Filled.Check,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onPrimary,
                    modifier = Modifier.size(15.dp),
                )
            }
        }
    }
}

// Paints a chat wallpaper as a fill layer: a bundled photographic drawable, a
// loaded custom image, or a drawn pattern/gradient approximation. Place it as
// the first child of a Box with the content drawn on top. Shared by the
// settings preview and the conversation thread background.
@Composable
internal fun ChatWallpaperBackground(
    presetId: String,
    customUrl: String,
    modifier: Modifier = Modifier,
    fallback: Color = MaterialTheme.colorScheme.background,
) {
    val dark = MaterialTheme.colorScheme.background.luminance() < 0.5f
    var customImage by remember(customUrl) { mutableStateOf<ImageBitmap?>(null) }
    LaunchedEffect(customUrl) {
        customImage = null
        if (customUrl.isNotBlank()) {
            customImage = withContext(ioDispatcher) { loadImageBitmapRef(customUrl) }
        }
    }

    val imageRes = if (customImage == null) wallpaperImageRes(presetId) else null
    Box(
        modifier =
            modifier.then(
                if (customImage == null && imageRes == null) {
                    Modifier.chatWallpaper(presetId, dark, fallback)
                } else {
                    Modifier
                },
            ),
    ) {
        val bmp = customImage
        if (bmp != null) {
            Image(
                bitmap = bmp,
                contentDescription = null,
                modifier = Modifier.fillMaxSize(),
                contentScale = ContentScale.Crop,
            )
        } else if (imageRes != null) {
            Image(
                painter = painterResource(imageRes),
                contentDescription = null,
                modifier = Modifier.fillMaxSize(),
                contentScale = ContentScale.Crop,
            )
        }
    }
}

// Telegram-style preview: paints the selected wallpaper with a couple of sample
// chat bubbles on top, so the choice reads at a glance before opening a thread.
@Composable
internal fun ChatWallpaperPreview(
    presetId: String,
    customUrl: String,
    modifier: Modifier = Modifier,
) {
    val chat = LocalChatColors.current
    Box(
        modifier =
            modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 4.dp)
                .height(160.dp)
                .clip(RoundedCornerShape(16.dp)),
    ) {
        ChatWallpaperBackground(
            presetId = presetId,
            customUrl = customUrl,
            modifier = Modifier.matchParentSize(),
            fallback = MaterialTheme.colorScheme.surface,
        )
        Column(
            modifier = Modifier.fillMaxSize().padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            PreviewBubble("Welcome to the conversation", chat.bubbleIn, chat.bubbleInText, false)
            PreviewBubble("This is your chat background", chat.bubbleOut, chat.bubbleOutText, true)
        }
    }
}

@Composable
private fun PreviewBubble(
    text: String,
    color: Color,
    textColor: Color,
    outgoing: Boolean,
) {
    val shape =
        if (outgoing) {
            RoundedCornerShape(topStart = 14.dp, topEnd = 4.dp, bottomEnd = 14.dp, bottomStart = 14.dp)
        } else {
            RoundedCornerShape(topStart = 4.dp, topEnd = 14.dp, bottomEnd = 14.dp, bottomStart = 14.dp)
        }
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = if (outgoing) Arrangement.End else Arrangement.Start,
    ) {
        Text(
            text,
            color = textColor,
            style = MaterialTheme.typography.bodySmall,
            modifier =
                Modifier
                    .widthIn(max = 220.dp)
                    .clip(shape)
                    .background(color)
                    .padding(horizontal = 12.dp, vertical = 7.dp),
        )
    }
}

// Approximates the desktop wallpaper presets: a base gradient plus an optional
// grid/dot pattern overlay drawn on top.
private fun Modifier.chatWallpaper(
    presetId: String,
    dark: Boolean,
    fallback: Color,
): Modifier {
    val colors =
        when (presetId) {
            "doodle" -> if (dark) listOf(Color(0xFF11131F), Color(0xFF0C0A16)) else listOf(Color(0xFFEEF2FF), Color(0xFFF5F3FF), Color(0xFFFEF2F2))
            "dots" -> if (dark) listOf(Color(0xFF121212), Color(0xFF171717)) else listOf(Color(0xFFFAFAF9), Color(0xFFF5F5F7))
            "grid" -> if (dark) listOf(Color(0xFF090E1A), Color(0xFF020617)) else listOf(Color(0xFFF3F4F6), Color(0xFFE5E7EB))
            "stripes" -> if (dark) listOf(Color(0xFF04120E), Color(0xFF020706)) else listOf(Color(0xFFF0FDF4), Color(0xFFDCFCE7))
            "hexagon" -> if (dark) listOf(Color(0xFF021613), Color(0xFF010807)) else listOf(Color(0xFFF0FDFA), Color(0xFFCCFBF1))
            "isometric" -> if (dark) listOf(Color(0xFF0B0F17), Color(0xFF05070C)) else listOf(Color(0xFFF1F5F9), Color(0xFFE2E8F0))
            "waves" -> if (dark) listOf(Color(0xFF06101A), Color(0xFF030A12)) else listOf(Color(0xFFF0FDF4), Color(0xFFE0F2FE))
            "nordic" -> if (dark) listOf(Color(0xFF0E0F12), Color(0xFF070809)) else listOf(Color(0xFFF9FAFB), Color(0xFFF3F4F6))
            "topography" -> if (dark) listOf(Color(0xFF0A1018), Color(0xFF05090F)) else listOf(Color(0xFFF0F4F8), Color(0xFFE2E8F0))
            "constellation" -> if (dark) listOf(Color(0xFF070B1A), Color(0xFF03050F)) else listOf(Color(0xFFE0F2FE), Color(0xFFE0E7FF))
            "cyberpunk" -> listOf(Color(0xFF0F0A1C), Color(0xFF07040D))
            "matrix" -> listOf(Color(0xFF02040A), Color(0xFF010204))
            "nightsky" -> listOf(Color(0xFF0B0F19), Color(0xFF050510), Color(0xFF020617))
            "autumn" -> if (dark) listOf(Color(0xFF1C1206), Color(0xFF0E0903)) else listOf(Color(0xFFFFFBEB), Color(0xFFFFEDD5), Color(0xFFFEF3C7))
            else -> listOf(fallback, fallback)
        }
    val base = Brush.linearGradient(colors)
    val accentPattern = if (dark) Color.White.copy(alpha = 0.05f) else Color(0xFF6366F1).copy(alpha = 0.10f)
    return this
        .background(base)
        .drawBehind {
            when (presetId) {
                "grid", "isometric", "nordic" -> {
                    val step = 24.dp.toPx()
                    var x = step
                    while (x < size.width) {
                        drawLine(accentPattern, Offset(x, 0f), Offset(x, size.height), 1f)
                        x += step
                    }
                    var y = step
                    while (y < size.height) {
                        drawLine(accentPattern, Offset(0f, y), Offset(size.width, y), 1f)
                        y += step
                    }
                }

                "dots", "doodle", "topography", "raindrops" -> {
                    val step = 20.dp.toPx()
                    val radius = 1.5.dp.toPx()
                    var y = step / 2
                    while (y < size.height) {
                        var x = step / 2
                        while (x < size.width) {
                            drawCircle(accentPattern, radius, Offset(x, y))
                            x += step
                        }
                        y += step
                    }
                }

                "stripes", "waves", "cyberpunk", "matrix" -> {
                    val patternColor =
                        when (presetId) {
                            "stripes", "waves" -> if (dark) Color(0xFF34D399).copy(alpha = 0.10f) else Color(0xFF10B981).copy(alpha = 0.12f)
                            else -> Color(0xFF22D3EE).copy(alpha = 0.16f)
                        }
                    val step = 22.dp.toPx()
                    var x = -size.height
                    while (x < size.width) {
                        drawLine(patternColor, Offset(x, size.height), Offset(x + size.height, 0f), 1.5f)
                        x += step
                    }
                }

                "constellation", "nightsky", "galaxy" -> {
                    val star = Color.White.copy(alpha = if (dark) 0.5f else 0.35f)
                    val seedPoints =
                        listOf(
                            0.12f to 0.22f,
                            0.3f to 0.55f,
                            0.48f to 0.18f,
                            0.62f to 0.7f,
                            0.78f to 0.32f,
                            0.88f to 0.6f,
                            0.2f to 0.82f,
                            0.55f to 0.4f,
                        )
                    seedPoints.forEach { (fx, fy) ->
                        drawCircle(star, 1.4.dp.toPx(), Offset(fx * size.width, fy * size.height))
                    }
                }
            }
        }
}

// A single send-as alias rendered as an editable row instead of a free-form
// textarea: an email field, an optional display-name field, and a remove button.
@Composable
internal fun AliasEditorRow(
    email: String,
    name: String,
    onEmailChange: (String) -> Unit,
    onNameChange: (String) -> Unit,
    onRemove: () -> Unit,
) {
    Surface(color = MaterialTheme.colorScheme.surface, modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp, top = 8.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    OutlinedTextField(
                        value = email,
                        onValueChange = onEmailChange,
                        label = { Text(tr("accounts.fields.emailAddress")) },
                        singleLine = true,
                        keyboardOptions = nativeTextKeyboardOptions.copy(keyboardType = KeyboardType.Email),
                        modifier = Modifier.weight(1f),
                    )
                    if (email.isEmpty()) {
                        val clipboardManager = LocalClipboardManager.current
                        IconButton(
                            onClick = {
                                val text = clipboardManager.getText()?.text.orEmpty()
                                if (text.isNotEmpty()) {
                                    onEmailChange(text)
                                }
                            },
                        ) {
                            Icon(
                                Icons.Filled.ContentPaste,
                                contentDescription = "Paste",
                            )
                        }
                    }
                }
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    OutlinedTextField(
                        value = name,
                        onValueChange = onNameChange,
                        label = { Text(tr("accounts.fields.displayNameMeronOnly")) },
                        singleLine = true,
                        keyboardOptions = nativeTextKeyboardOptions,
                        modifier = Modifier.weight(1f),
                    )
                    if (name.isEmpty()) {
                        val clipboardManager = LocalClipboardManager.current
                        IconButton(
                            onClick = {
                                val text = clipboardManager.getText()?.text.orEmpty()
                                if (text.isNotEmpty()) {
                                    onNameChange(text)
                                }
                            },
                        ) {
                            Icon(
                                Icons.Filled.ContentPaste,
                                contentDescription = "Paste",
                            )
                        }
                    }
                }
            }
            IconButton(onClick = onRemove) {
                Icon(
                    Icons.Filled.Close,
                    contentDescription = tr("settings.account.removeAlias"),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
internal fun AccountAvatarEditor(
    name: String,
    url: String,
    onPick: (() -> Unit)?,
    modifier: Modifier = Modifier,
) {
    var image by remember(url) { mutableStateOf<ImageBitmap?>(null) }
    LaunchedEffect(url) {
        image = null
        if (url.isNotBlank()) {
            image = withContext(ioDispatcher) { loadImageBitmapRef(url) }
        }
    }
    Box(
        modifier =
            modifier
                .size(64.dp)
                .clip(CircleShape)
                .then(if (onPick != null) Modifier.clickable(onClick = onPick) else Modifier),
        contentAlignment = Alignment.Center,
    ) {
        val bmp = image
        if (bmp != null) {
            Image(
                bitmap = bmp,
                contentDescription = tr("avatar.edit"),
                modifier = Modifier.size(64.dp).clip(CircleShape),
                contentScale = ContentScale.Crop,
            )
        } else {
            Avatar(name, 64.dp)
        }
        if (onPick != null) {
            Box(
                modifier =
                    Modifier
                        .align(Alignment.BottomEnd)
                        .size(22.dp)
                        .clip(CircleShape)
                        .background(MaterialTheme.colorScheme.primary),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    Icons.Filled.Edit,
                    contentDescription = tr("settings.account.changeAvatar"),
                    tint = MaterialTheme.colorScheme.onPrimary,
                    modifier = Modifier.size(13.dp),
                )
            }
        }
    }
}

@Composable
internal fun SettingsKanbanBoardDetailPage(
    board: KanbanBoardSpec,
    active: Boolean,
    onSave: (
        name: String,
        avatarUrl: String,
        wallpaperPresetId: String,
        wallpaperUrl: String,
    ) -> Unit,
    onPickAvatar: () -> Unit,
    onOpenWallpaper: () -> Unit,
    onDelete: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var name by remember(board.id, board.name) { mutableStateOf(board.name) }
    var avatarUrl by remember(board.id, board.avatarUrl) { mutableStateOf(board.avatarUrl) }
    var wallpaperPresetId by remember(board.id, board.wallpaperPresetId) { mutableStateOf(board.wallpaperPresetId) }
    var wallpaperUrl by remember(board.id, board.wallpaperUrl) { mutableStateOf(board.wallpaperUrl) }
    var confirmDelete by remember(board.id) { mutableStateOf(false) }

    // Autosave on every change, like the desktop board panel.
    val persist: (
        nextName: String,
        nextAvatarUrl: String,
        nextWallpaperPresetId: String,
        nextWallpaperUrl: String,
    ) -> Unit = { nextName, nextAvatarUrl, nextWallpaperPresetId, nextWallpaperUrl ->
        onSave(nextName, nextAvatarUrl, nextWallpaperPresetId, nextWallpaperUrl)
    }

    if (confirmDelete) {
        AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text(tr("kanban.board.deleteTitle")) },
            text = { Text(trf("settings.kanban.deleteBoardText", board.name)) },
            confirmButton = {
                TextButton(onClick = {
                    confirmDelete = false
                    onDelete()
                }) {
                    Text(tr("kanban.board.delete"), color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmDelete = false }) { Text(tr("buttons.cancel")) }
            },
        )
    }
    LazyColumn(modifier) {
        item {
            Row(
                modifier = Modifier.fillMaxWidth().padding(16.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                AccountAvatarEditor(
                    name = name.ifBlank { board.name },
                    url = avatarUrl,
                    onPick = onPickAvatar,
                )
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        name.ifBlank { board.name },
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        if (active) {
                            trf("settings.kanban.boardColumnsActive", board.columns.size)
                        } else {
                            trf("settings.kanban.boardColumns", board.columns.size)
                        },
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }

        item { SettingsSectionLabel(tr("settings.account.profile")) }
        item {
            SettingsTextRow(
                value = name,
                label = tr("kanban.board.name"),
                onValueChange = {
                    name = it
                    persist(it, avatarUrl, wallpaperPresetId, wallpaperUrl)
                },
            )
        }

        item { SettingsSectionLabel(tr("settings.account.chatBackground")) }
        item {
            SettingsRow(
                icon = Icons.Filled.Image,
                title = tr("settings.account.chatBackground"),
                subtitle =
                    if (wallpaperUrl.isNotBlank()) {
                        tr("wallpaper.customBackground")
                    } else {
                        wallpaperPresetDisplayName(wallpaperPresetId)
                    },
                onClick = onOpenWallpaper,
                trailing = {
                    WallpaperRowPreview(
                        presetId = wallpaperPresetId,
                        customUrl = wallpaperUrl,
                    )
                },
            )
        }

        item { SettingsSectionLabel(tr("settings.dangerZone")) }
        item {
            SettingsRow(
                icon = Icons.Filled.Delete,
                title = tr("kanban.board.delete"),
                subtitle = tr("settings.kanban.deleteBoardHint"),
                onClick = { confirmDelete = true },
                destructive = true,
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
        leading = { AccountBadgeAvatar(label = label, avatarUrl = account.avatarUrl, size = 40.dp) },
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
    subtitle: String?,
    onClick: () -> Unit,
    leading: (@Composable () -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
    destructive: Boolean = false,
) {
    val accent = if (destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface
    ListItem(
        headlineContent = { Text(title, color = accent) },
        supportingContent = subtitle?.let { { Text(it) } },
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
    subtitle: String?,
    checked: Boolean,
    onToggle: () -> Unit,
) {
    ListItem(
        headlineContent = { Text(title) },
        supportingContent = subtitle?.let { { Text(it) } },
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

@Composable
internal fun ThemePickerDialog(
    current: AppAppearanceMode,
    onSelect: (AppAppearanceMode) -> Unit,
    onDismiss: () -> Unit,
) {
    val selectableModes = AppAppearanceMode.entries.filterNot { it == AppAppearanceMode.System }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("common.theme")) },
        text = {
            LazyColumn(modifier = Modifier.heightIn(max = 480.dp)) {
                items(selectableModes) { mode ->
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clickable {
                                onSelect(mode)
                                onDismiss()
                            }.padding(vertical = 2.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = mode == current, onClick = {
                            onSelect(mode)
                            onDismiss()
                        })
                        Text(mode.label, modifier = Modifier.padding(start = 8.dp))
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.done")) } },
    )
}

@Composable
internal fun LanguagePickerDialog(
    currentTag: String,
    onSelect: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("settings.language.label")) },
        text = {
            LazyColumn(modifier = Modifier.heightIn(max = 480.dp)) {
                item {
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clickable {
                                onSelect("")
                                onDismiss()
                            }.padding(vertical = 2.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = currentTag.isBlank(), onClick = {
                            onSelect("")
                            onDismiss()
                        })
                        Text(tr("settings.language.system"), modifier = Modifier.padding(start = 8.dp))
                    }
                }
                items(supportedAppLanguageTags) { tag ->
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clickable {
                                onSelect(tag)
                                onDismiss()
                            }.padding(vertical = 2.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = tag == currentTag, onClick = {
                            onSelect(tag)
                            onDismiss()
                        })
                        Text(appLanguageDisplayName(tag), modifier = Modifier.padding(start = 8.dp))
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.done")) } },
    )
}
