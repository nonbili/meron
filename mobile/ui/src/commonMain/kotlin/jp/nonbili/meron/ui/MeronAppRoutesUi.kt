package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.MarkEmailUnread
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.PersonAdd
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.StarBorder
import androidx.compose.material.icons.outlined.Drafts
import androidx.compose.material.icons.outlined.FolderOpen
import androidx.compose.material.icons.outlined.NotificationsNone
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DrawerState
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.threadIdIsRss
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

@Composable
internal fun MailSelectionTitle(
    selectedCount: Int,
    height: Dp,
) {
    Box(
        modifier = Modifier.fillMaxWidth().height(height),
        contentAlignment = Alignment.CenterStart,
    ) {
        Text(
            "$selectedCount selected",
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun StarredRouteContent(
    state: MeronMobileState,
    drawerState: DrawerState,
    drawerFolders: List<FolderSummary>,
) {
    with(state) {
        ModalNavigationDrawer(
            drawerState = drawerState,
            drawerContent = {
                MailDrawer(
                    accounts = coreAccounts.filterNot { it.id in hiddenNavigationAccountIds },
                    selectedAccountId = selectedCoreAccountId,
                    folders = drawerFolders,
                    currentScreen = screen,
                    showUnreadBadges = showUnreadBadges,
                    showUnifiedInboxNav = showUnifiedInboxNav,
                    showStarredNav = showStarredNav,
                    kanbanBoards = kanbanBoards,
                    activeKanbanBoardId = activeKanbanBoardId,
                    onSelectUnified = {
                        screen = Screen.Mail
                        selectCoreMailbox(UNIFIED_ACCOUNT_ID, INBOX_FOLDER)
                        syncCoreThreads(accountOverride = UNIFIED_ACCOUNT_ID, folderOverride = INBOX_FOLDER, syncFirst = false)
                        scope.launch { drawerState.close() }
                    },
                    onSelectAccount = { account ->
                        screen = Screen.Mail
                        selectCoreMailbox(account.id, INBOX_FOLDER)
                        syncCoreThreads(accountOverride = account.id, folderOverride = INBOX_FOLDER, syncFirst = false)
                        scope.launch { drawerState.close() }
                    },
                    onSelectStarred = { scope.launch { drawerState.close() } },
                    onSelectKanban = {
                        screen = Screen.Kanban
                        previousTopScreen = Screen.Kanban
                        loadKanbanBoard(refresh = false)
                        scope.launch { drawerState.close() }
                    },
                    onSelectKanbanBoard = { board ->
                        activeKanbanBoardId = board.id
                        saveActiveKanbanBoardId(kanbanPrefs, board.id)
                        screen = Screen.Kanban
                        previousTopScreen = Screen.Kanban
                        loadKanbanBoard(refresh = false)
                        scope.launch { drawerState.close() }
                    },
                    onAddAccount = {
                        addSection = 0
                        passwordServerSettingsOpen = false
                        previousTopScreen = Screen.Starred
                        screen = Screen.AddAccount
                        scope.launch { drawerState.close() }
                    },
                    onOpenSettings = {
                        previousTopScreen = screen
                        screen = Screen.Settings
                        scope.launch { drawerState.close() }
                    },
                    onShowAbout = {
                        showAboutDialog = true
                        scope.launch { drawerState.close() }
                    },
                    googleReauthAccountId = googleReauthAccountId,
                    onReconnectGoogle = {
                        connectGoogleDeviceAccount()
                        scope.launch { drawerState.close() }
                    },
                )
            },
        ) {
            Scaffold(
                topBar = {
                    TopAppBar(
                        title = {
                            Column {
                                Text(tr("mobile.tabs.starred"), fontWeight = FontWeight.SemiBold)
                                Text(
                                    tr("mobile.mail.starredSubtitle"),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        },
                        navigationIcon = {
                            IconButton(onClick = { scope.launch { drawerState.open() } }) {
                                Icon(Icons.Filled.Menu, contentDescription = tr("mobile.actions.openNavigation"))
                            }
                        },
                        actions = {
                            IconButton(onClick = ::loadStarredItems) {
                                Icon(Icons.Filled.Refresh, contentDescription = tr("mobile.actions.refreshStarred"))
                            }
                        },
                    )
                },
                snackbarHost = { SnackbarHost(snackbarHost) },
            ) { innerPadding ->
                Box(Modifier.fillMaxSize().padding(innerPadding)) {
                    if (syncing) {
                        CircularProgressIndicator(Modifier.padding(top = 4.dp).align(Alignment.TopCenter).size(28.dp))
                    }
                    if (starredItems.isEmpty()) {
                        EmptyState(
                            icon = Icons.Filled.StarBorder,
                            title = tr("empty.noStarredItems"),
                            text = tr("empty.noStarredItemsText"),
                            actionLabel = tr("mobile.actions.refresh"),
                            onAction = ::loadStarredItems,
                        )
                    } else {
                        StarredItemList(
                            items = starredItems,
                            showSenderImages = showSenderImages,
                            onOpen = ::readStarredItem,
                            onToggleRead = ::toggleStarredItemRead,
                            onUnstar = ::unstarStarredItem,
                            onDelete = ::deleteStarredMailItem,
                        )
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun KanbanRouteContent(
    state: MeronMobileState,
    drawerState: DrawerState,
    drawerFolders: List<FolderSummary>,
    mailSelectionActive: Boolean,
    selectedMailThreads: List<ThreadSummary>,
    activeKanbanBoard: KanbanBoardSpec?,
) {
    with(state) {
        ModalNavigationDrawer(
            drawerState = drawerState,
            drawerContent = {
                MailDrawer(
                    accounts = coreAccounts.filterNot { it.id in hiddenNavigationAccountIds },
                    selectedAccountId = selectedCoreAccountId,
                    folders = drawerFolders,
                    currentScreen = screen,
                    showUnreadBadges = showUnreadBadges,
                    showUnifiedInboxNav = showUnifiedInboxNav,
                    showStarredNav = showStarredNav,
                    kanbanBoards = kanbanBoards,
                    activeKanbanBoardId = activeKanbanBoardId,
                    onSelectUnified = {
                        screen = Screen.Mail
                        if (selectedCoreAccountId != UNIFIED_ACCOUNT_ID) {
                            selectCoreMailbox(UNIFIED_ACCOUNT_ID, INBOX_FOLDER)
                            syncCoreThreads(accountOverride = UNIFIED_ACCOUNT_ID, folderOverride = INBOX_FOLDER, syncFirst = false)
                        }
                        scope.launch { drawerState.close() }
                    },
                    onSelectAccount = { account ->
                        screen = Screen.Mail
                        if (selectedCoreAccountId != account.id) {
                            selectCoreMailbox(account.id, INBOX_FOLDER)
                            syncCoreThreads(accountOverride = account.id, folderOverride = INBOX_FOLDER, syncFirst = false)
                        }
                        scope.launch { drawerState.close() }
                    },
                    onSelectStarred = {
                        screen = Screen.Starred
                        previousTopScreen = Screen.Starred
                        loadStarredItems()
                        scope.launch { drawerState.close() }
                    },
                    onSelectKanban = { scope.launch { drawerState.close() } },
                    onSelectKanbanBoard = { board ->
                        if (activeKanbanBoardId != board.id) {
                            activeKanbanBoardId = board.id
                            saveActiveKanbanBoardId(kanbanPrefs, board.id)
                            loadKanbanBoard(refresh = false)
                        }
                        scope.launch { drawerState.close() }
                    },
                    onAddAccount = {
                        addSection = 0
                        passwordServerSettingsOpen = false
                        previousTopScreen = Screen.Kanban
                        screen = Screen.AddAccount
                        scope.launch { drawerState.close() }
                    },
                    onOpenSettings = {
                        previousTopScreen = screen
                        screen = Screen.Settings
                        scope.launch { drawerState.close() }
                    },
                    onShowAbout = {
                        showAboutDialog = true
                        scope.launch { drawerState.close() }
                    },
                    googleReauthAccountId = googleReauthAccountId,
                    onReconnectGoogle = {
                        connectGoogleDeviceAccount()
                        scope.launch { drawerState.close() }
                    },
                )
            },
        ) {
            Scaffold(
                topBar = {
                    TopAppBar(
                        title = {
                            if (mailSelectionActive) {
                                MailSelectionTitle(selectedMailThreads.size, height = 52.dp)
                            } else {
                                KanbanHeaderSearchField(
                                    search = kanbanSearch,
                                    searchScope = kanbanSearchScope,
                                    columns = activeKanbanBoard?.columns.orEmpty(),
                                    accounts = coreAccounts.filterNot { it.id in hiddenNavigationAccountIds },
                                    foldersByAccount = foldersByAccount,
                                    onSearchChange = ::persistKanbanSearch,
                                    onSearchScopeChange = ::persistKanbanSearchScope,
                                    onSearchSubmit = { loadKanbanBoard(refresh = true) },
                                )
                            }
                        },
                        navigationIcon = {
                            if (mailSelectionActive) {
                                IconButton(onClick = {
                                    selectedMailThreadIds = emptySet()
                                    mailSelectionMenuOpen = false
                                }) {
                                    Icon(Icons.Filled.Close, contentDescription = tr("mobile.actions.clearSelection"))
                                }
                            } else {
                                IconButton(onClick = { scope.launch { drawerState.open() } }) {
                                    Icon(Icons.Filled.Menu, contentDescription = tr("mobile.actions.openNavigation"))
                                }
                            }
                        },
                        actions = {
                            if (mailSelectionActive) {
                                IconButton(onClick = {
                                    selectedMailThreads.forEach(::archiveOrRemove)
                                    selectedMailThreadIds = emptySet()
                                }) {
                                    Icon(Icons.Filled.Archive, contentDescription = tr("threads.actions.archiveThread"))
                                }
                                IconButton(onClick = {
                                    selectedMailThreads.forEach(::deleteThread)
                                    selectedMailThreadIds = emptySet()
                                }) {
                                    Icon(Icons.Filled.Delete, contentDescription = tr("buttons.delete"))
                                }
                                Box {
                                    IconButton(onClick = { mailSelectionMenuOpen = true }) {
                                        Icon(Icons.Filled.MoreVert, contentDescription = tr("chat.moreActions"))
                                    }
                                    DropdownMenu(
                                        expanded = mailSelectionMenuOpen,
                                        onDismissRequest = { mailSelectionMenuOpen = false },
                                    ) {
                                        val markRead = selectedMailThreads.any { it.unread }
                                        DropdownMenuItem(
                                            text = { Text(if (markRead) tr("threads.actions.markAsRead") else tr("threads.actions.markAsUnread")) },
                                            leadingIcon = { Icon(Icons.Filled.MarkEmailUnread, contentDescription = null) },
                                            onClick = {
                                                mailSelectionMenuOpen = false
                                                selectedMailThreads
                                                    .filter { it.unread == markRead }
                                                    .forEach(::toggleRead)
                                                selectedMailThreadIds = emptySet()
                                            },
                                        )
                                        val star = selectedMailThreads.any { !it.starred }
                                        DropdownMenuItem(
                                            text = { Text(if (star) tr("chat.star") else tr("chat.unstar")) },
                                            leadingIcon = {
                                                Icon(
                                                    if (star) Icons.Filled.StarBorder else Icons.Filled.Star,
                                                    contentDescription = null,
                                                )
                                            },
                                            onClick = {
                                                mailSelectionMenuOpen = false
                                                selectedMailThreads
                                                    .filter { it.starred != star }
                                                    .forEach(::toggleStar)
                                                selectedMailThreadIds = emptySet()
                                            },
                                        )
                                        val singleSelectedMailThread =
                                            selectedMailThreads
                                                .singleOrNull()
                                                ?.takeUnless { threadIdIsRss(it.id) }
                                        if (singleSelectedMailThread != null) {
                                            HorizontalDivider(Modifier.padding(vertical = 4.dp))
                                            DropdownMenuItem(
                                                text = { Text(tr("threads.actions.moveTo")) },
                                                leadingIcon = { Icon(Icons.Outlined.FolderOpen, contentDescription = null) },
                                                onClick = {
                                                    mailSelectionMenuOpen = false
                                                    ensureThreadActionFolders(
                                                        thread = singleSelectedMailThread,
                                                        includeAllMailAccounts = false,
                                                    ) {
                                                        selectedMailMoveThread = singleSelectedMailThread
                                                    }
                                                },
                                            )
                                            DropdownMenuItem(
                                                text = { Text(tr("threads.actions.copyTo")) },
                                                leadingIcon = { Icon(Icons.Filled.ContentCopy, contentDescription = null) },
                                                onClick = {
                                                    mailSelectionMenuOpen = false
                                                    ensureThreadActionFolders(
                                                        thread = singleSelectedMailThread,
                                                        includeAllMailAccounts = true,
                                                    ) {
                                                        selectedMailCopyThread = singleSelectedMailThread
                                                    }
                                                },
                                            )
                                        }
                                    }
                                }
                            } else {
                                Box {
                                    IconButton(onClick = { kanbanMenuOpen = true }) {
                                        Icon(Icons.Filled.MoreVert, contentDescription = tr("kanban.actions.boardOptions"))
                                    }
                                    DropdownMenu(
                                        expanded = kanbanMenuOpen,
                                        onDismissRequest = { kanbanMenuOpen = false },
                                        modifier = Modifier.width(260.dp),
                                    ) {
                                        FilterModeSegmentedControl(
                                            filter = kanbanFilter,
                                            onFilterChange = { mode ->
                                                persistKanbanFilter(mode)
                                                loadKanbanBoard(refresh = true)
                                            },
                                            modifier =
                                                Modifier
                                                    .fillMaxWidth()
                                                    .padding(horizontal = 12.dp, vertical = 8.dp),
                                        )
                                        HorizontalDivider(Modifier.padding(vertical = 4.dp))
                                        DropdownMenuItem(
                                            text = { Text(tr("mobile.actions.refreshBoard")) },
                                            leadingIcon = { Icon(Icons.Filled.Refresh, contentDescription = null) },
                                            onClick = {
                                                kanbanMenuOpen = false
                                                loadKanbanBoard(refresh = true)
                                            },
                                        )
                                        HorizontalDivider(Modifier.padding(vertical = 4.dp))
                                        DropdownMenuItem(
                                            text = { Text(tr("kanban.actions.addColumn")) },
                                            leadingIcon = { Icon(Icons.Filled.Add, contentDescription = null) },
                                            onClick = {
                                                kanbanMenuOpen = false
                                                coreAccounts.forEach { account ->
                                                    scope.launch {
                                                        runCatching {
                                                            withContext(ioDispatcher) {
                                                                val client = MobileMailCommandClient(core)
                                                                loadAccountFolders(client, account)
                                                            }
                                                        }.onSuccess { foldersByAccount = foldersByAccount + (account.id to it) }
                                                    }
                                                }
                                                showKanbanColumnDialog = true
                                            },
                                        )
                                        DropdownMenuItem(
                                            text = { Text(tr("kanban.actions.boardOptions")) },
                                            leadingIcon = { Icon(Icons.Filled.Settings, contentDescription = null) },
                                            enabled = activeKanbanBoard != null,
                                            onClick = {
                                                kanbanMenuOpen = false
                                                activeKanbanBoard?.let { board ->
                                                    kanbanSettingsTargetId = board.id
                                                    previousTopScreen = Screen.Kanban
                                                    screen = Screen.Settings
                                                }
                                            },
                                        )
                                    }
                                }
                            }
                        },
                    )
                },
                floatingActionButton = {
                    if (coreAccounts.any { !accountSummaryIsRss(it) }) {
                        ExtendedFloatingActionButton(
                            onClick = ::openCompose,
                            icon = { Icon(Icons.Filled.Edit, contentDescription = tr("mobile.tabs.compose")) },
                            text = { Text(tr("mobile.tabs.compose")) },
                        )
                    }
                },
                snackbarHost = { SnackbarHost(snackbarHost) },
            ) { innerPadding ->
                KanbanScreen(
                    modifier = Modifier.fillMaxSize().padding(innerPadding),
                    accounts = coreAccounts.filterNot { it.id in hiddenNavigationAccountIds },
                    accountsLoading = !initialAccountsLoaded || accountsLoading,
                    board = activeKanbanBoard,
                    columns = kanbanColumns,
                    foldersByAccount = foldersByAccount,
                    filter = kanbanFilter,
                    search = kanbanSearch,
                    searchScope = kanbanSearchScope,
                    onOpen = { thread, column -> readCoreThread(thread, sourceFolder = column.folderId) },
                    selectedThreadIds = selectedMailThreadIds,
                    selectionActive = mailSelectionActive,
                    onToggleSelected = { thread ->
                        selectedMailThreadIds =
                            if (thread.id in selectedMailThreadIds) {
                                selectedMailThreadIds - thread.id
                            } else {
                                selectedMailThreadIds + thread.id
                            }
                        if (selectedMailThreadIds.isEmpty()) {
                            mailSelectionMenuOpen = false
                        }
                    },
                    onLongPress = { thread ->
                        selectedMailThreadIds =
                            if (thread.id in selectedMailThreadIds) {
                                selectedMailThreadIds
                            } else {
                                selectedMailThreadIds + thread.id
                            }
                    },
                    onToggleStar = ::toggleStar,
                    onRefreshColumn = { loadKanbanColumn(it, refresh = true) },
                    onLoadMoreColumn = ::loadMoreKanbanColumn,
                    onMarkColumnAllRead = ::markKanbanColumnAllRead,
                    onRemoveColumn = ::removeKanbanColumn,
                    onMoveColumn = ::moveKanbanColumn,
                    onSearchColumn = { column ->
                        persistKanbanSearchScope(kanbanColumnKey(column))
                        if (kanbanSearch.isNotBlank()) loadKanbanBoard(refresh = true)
                    },
                    onAddColumn = { showKanbanColumnDialog = true },
                    showSenderImages = showSenderImages,
                    kanbanColumnWidth = kanbanColumnWidth.dp,
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun MailRouteContent(
    state: MeronMobileState,
    drawerState: DrawerState,
    drawerFolders: List<FolderSummary>,
    selectedAccount: AccountSummary?,
    selectedAccountIsRss: Boolean,
    mailSelectionActive: Boolean,
    selectedMailThreads: List<ThreadSummary>,
    importOpml: (PickedFile?) -> Unit,
) {
    with(state) {
        ModalNavigationDrawer(
            drawerState = drawerState,
            drawerContent = {
                MailDrawer(
                    accounts = coreAccounts.filterNot { it.id in hiddenNavigationAccountIds },
                    selectedAccountId = selectedCoreAccountId,
                    folders = drawerFolders,
                    currentScreen = screen,
                    showUnreadBadges = showUnreadBadges,
                    showUnifiedInboxNav = showUnifiedInboxNav,
                    showStarredNav = showStarredNav,
                    kanbanBoards = kanbanBoards,
                    activeKanbanBoardId = activeKanbanBoardId,
                    onSelectUnified = {
                        if (selectedCoreAccountId != UNIFIED_ACCOUNT_ID) {
                            selectCoreMailbox(UNIFIED_ACCOUNT_ID, INBOX_FOLDER)
                            syncCoreThreads(accountOverride = UNIFIED_ACCOUNT_ID, folderOverride = INBOX_FOLDER, syncFirst = false)
                        }
                        screen = Screen.Mail
                        scope.launch { drawerState.close() }
                    },
                    onSelectAccount = { account ->
                        if (selectedCoreAccountId != account.id) {
                            selectCoreMailbox(account.id, INBOX_FOLDER)
                            syncCoreThreads(accountOverride = account.id, folderOverride = INBOX_FOLDER, syncFirst = false)
                        }
                        screen = Screen.Mail
                        scope.launch { drawerState.close() }
                    },
                    onSelectStarred = {
                        screen = Screen.Starred
                        previousTopScreen = Screen.Starred
                        loadStarredItems()
                        scope.launch { drawerState.close() }
                    },
                    onSelectKanban = {
                        screen = Screen.Kanban
                        previousTopScreen = Screen.Kanban
                        loadKanbanBoard(refresh = false)
                        scope.launch { drawerState.close() }
                    },
                    onSelectKanbanBoard = { board ->
                        activeKanbanBoardId = board.id
                        saveActiveKanbanBoardId(kanbanPrefs, board.id)
                        screen = Screen.Kanban
                        previousTopScreen = Screen.Kanban
                        loadKanbanBoard(refresh = false)
                        scope.launch { drawerState.close() }
                    },
                    onAddAccount = {
                        addSection = 0
                        passwordServerSettingsOpen = false
                        previousTopScreen = screen
                        screen = Screen.AddAccount
                        scope.launch { drawerState.close() }
                    },
                    onOpenSettings = {
                        previousTopScreen = screen
                        screen = Screen.Settings
                        scope.launch { drawerState.close() }
                    },
                    onShowAbout = {
                        showAboutDialog = true
                        scope.launch { drawerState.close() }
                    },
                    googleReauthAccountId = googleReauthAccountId,
                    onReconnectGoogle = {
                        connectGoogleDeviceAccount()
                        scope.launch { drawerState.close() }
                    },
                )
            },
        ) {
            Scaffold(
                topBar = {
                    TopAppBar(
                        title = {
                            if (mailSelectionActive) {
                                MailSelectionTitle(selectedMailThreads.size, height = 44.dp)
                            } else {
                                MailHeaderSearchField(
                                    search = mailSearch,
                                    placeholder =
                                        if (selectedAccountIsRss) {
                                            tr("threads.searchFeeds")
                                        } else {
                                            null
                                        },
                                    onSearchChange = { mailSearch = it },
                                    onSearchSubmit = { syncCoreThreads(syncFirst = false) },
                                )
                            }
                        },
                        navigationIcon = {
                            if (mailSelectionActive) {
                                IconButton(onClick = {
                                    selectedMailThreadIds = emptySet()
                                    mailSelectionMenuOpen = false
                                }) {
                                    Icon(Icons.Filled.Close, contentDescription = tr("mobile.actions.clearSelection"))
                                }
                            } else {
                                IconButton(onClick = { scope.launch { drawerState.open() } }) {
                                    Icon(Icons.Filled.Menu, contentDescription = tr("mobile.actions.openNavigation"))
                                }
                            }
                        },
                        actions = {
                            if (mailSelectionActive) {
                                IconButton(onClick = {
                                    selectedMailThreads.forEach(::archiveOrRemove)
                                    selectedMailThreadIds = emptySet()
                                }) {
                                    Icon(Icons.Filled.Archive, contentDescription = tr("threads.actions.archiveThread"))
                                }
                                IconButton(onClick = {
                                    selectedMailThreads.forEach(::deleteThread)
                                    selectedMailThreadIds = emptySet()
                                }) {
                                    Icon(Icons.Filled.Delete, contentDescription = tr("buttons.delete"))
                                }
                                Box {
                                    IconButton(onClick = { mailSelectionMenuOpen = true }) {
                                        Icon(Icons.Filled.MoreVert, contentDescription = tr("chat.moreActions"))
                                    }
                                    DropdownMenu(
                                        expanded = mailSelectionMenuOpen,
                                        onDismissRequest = { mailSelectionMenuOpen = false },
                                    ) {
                                        val markRead = selectedMailThreads.any { it.unread }
                                        DropdownMenuItem(
                                            text = { Text(if (markRead) tr("threads.actions.markAsRead") else tr("threads.actions.markAsUnread")) },
                                            leadingIcon = { Icon(Icons.Filled.MarkEmailUnread, contentDescription = null) },
                                            onClick = {
                                                mailSelectionMenuOpen = false
                                                selectedMailThreads
                                                    .filter { it.unread == markRead }
                                                    .forEach(::toggleRead)
                                                selectedMailThreadIds = emptySet()
                                            },
                                        )
                                        val star = selectedMailThreads.any { !it.starred }
                                        DropdownMenuItem(
                                            text = { Text(if (star) tr("chat.star") else tr("chat.unstar")) },
                                            leadingIcon = {
                                                Icon(
                                                    if (star) Icons.Filled.StarBorder else Icons.Filled.Star,
                                                    contentDescription = null,
                                                )
                                            },
                                            onClick = {
                                                mailSelectionMenuOpen = false
                                                selectedMailThreads
                                                    .filter { it.starred != star }
                                                    .forEach(::toggleStar)
                                                selectedMailThreadIds = emptySet()
                                            },
                                        )
                                        val singleSelectedMailThread =
                                            selectedMailThreads
                                                .singleOrNull()
                                                ?.takeUnless { threadIdIsRss(it.id) }
                                        if (singleSelectedMailThread != null) {
                                            HorizontalDivider(Modifier.padding(vertical = 4.dp))
                                            DropdownMenuItem(
                                                text = { Text(tr("threads.actions.moveTo")) },
                                                leadingIcon = { Icon(Icons.Outlined.FolderOpen, contentDescription = null) },
                                                onClick = {
                                                    mailSelectionMenuOpen = false
                                                    ensureThreadActionFolders(
                                                        thread = singleSelectedMailThread,
                                                        includeAllMailAccounts = false,
                                                    ) {
                                                        selectedMailMoveThread = singleSelectedMailThread
                                                    }
                                                },
                                            )
                                            DropdownMenuItem(
                                                text = { Text(tr("threads.actions.copyTo")) },
                                                leadingIcon = { Icon(Icons.Filled.ContentCopy, contentDescription = null) },
                                                onClick = {
                                                    mailSelectionMenuOpen = false
                                                    ensureThreadActionFolders(
                                                        thread = singleSelectedMailThread,
                                                        includeAllMailAccounts = true,
                                                    ) {
                                                        selectedMailCopyThread = singleSelectedMailThread
                                                    }
                                                },
                                            )
                                        }
                                    }
                                }
                            } else {
                                val loadedUnread = coreThreads.any { it.unread }
                                val folderUnreadTotal =
                                    if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID) {
                                        val includedAccountIds =
                                            coreAccounts.filter { it.includedInUnified }.map { it.id }.toSet()
                                        coreFolders
                                            .filter { it.accountId in includedAccountIds }
                                            .groupBy { it.accountId }
                                            .values
                                            .sumOf { accountFolders -> folderUnread(accountFolders, INBOX_FOLDER) }
                                    } else {
                                        folderUnread(
                                            coreFolders.filter { it.accountId == selectedCoreAccountId },
                                            selectedCoreFolder,
                                        )
                                    }
                                val showMarkAllRead = folderUnreadTotal > 0 || loadedUnread
                                val showAccountActions = selectedAccount != null
                                Box {
                                    IconButton(onClick = { mailboxMenuOpen = true }) {
                                        Icon(
                                            Icons.Filled.MoreVert,
                                            contentDescription =
                                                if (selectedAccountIsRss) {
                                                    tr("feeds.manageSubscription")
                                                } else {
                                                    tr("threads.actions.title")
                                                },
                                        )
                                    }
                                    DropdownMenu(
                                        expanded = mailboxMenuOpen,
                                        onDismissRequest = { mailboxMenuOpen = false },
                                        modifier = Modifier.width(260.dp),
                                    ) {
                                        FilterModeSegmentedControl(
                                            filter = mailFilter,
                                            onFilterChange = { mode ->
                                                mailFilter = mode
                                                syncCoreThreads(syncFirst = false)
                                            },
                                            modifier =
                                                Modifier
                                                    .fillMaxWidth()
                                                    .padding(horizontal = 12.dp, vertical = 8.dp),
                                        )
                                        if (showMarkAllRead || showAccountActions) {
                                            HorizontalDivider(Modifier.padding(vertical = 4.dp))
                                        }
                                        if (showMarkAllRead) {
                                            DropdownMenuItem(
                                                text = { Text(tr("threads.actions.markAllAsRead")) },
                                                leadingIcon = {
                                                    Icon(Icons.Filled.MarkEmailUnread, contentDescription = null)
                                                },
                                                onClick = {
                                                    mailboxMenuOpen = false
                                                    markVisibleMailboxAllRead()
                                                },
                                            )
                                        }
                                        if (showAccountActions) {
                                            if (showMarkAllRead) {
                                                HorizontalDivider(Modifier.padding(vertical = 4.dp))
                                            }
                                            DropdownMenuItem(
                                                text = { Text(tr("settings.account.accountSettings")) },
                                                leadingIcon = {
                                                    Icon(Icons.Filled.Settings, contentDescription = null)
                                                },
                                                onClick = {
                                                    mailboxMenuOpen = false
                                                    accountSettingsTargetId = selectedAccount.id
                                                    previousTopScreen = Screen.Mail
                                                    screen = Screen.Settings
                                                },
                                            )
                                            if (selectedAccount.let(::accountSummaryIsRss)) {
                                                HorizontalDivider(Modifier.padding(vertical = 4.dp))
                                                DropdownMenuItem(
                                                    text = { Text(tr("feeds.actions.addFeed")) },
                                                    onClick = {
                                                        mailboxMenuOpen = false
                                                        addFeedUrl = ""
                                                        addFeedError = ""
                                                        showAddFeedDialog = true
                                                    },
                                                )
                                                DropdownMenuItem(
                                                    text = { Text(tr("common.import")) },
                                                    onClick = {
                                                        mailboxMenuOpen = false
                                                        services.pickFile(
                                                            listOf("text/xml", "application/xml", "text/*", "*/*"),
                                                            importOpml,
                                                        )
                                                    },
                                                )
                                                DropdownMenuItem(
                                                    text = { Text(tr("common.export")) },
                                                    onClick = {
                                                        mailboxMenuOpen = false
                                                        exportOpmlForSelectedAccount()
                                                    },
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    )
                },
                floatingActionButton = {
                    when {
                        selectedAccountIsRss -> {
                            ExtendedFloatingActionButton(
                                onClick = {
                                    addFeedUrl = ""
                                    addFeedError = ""
                                    showAddFeedDialog = true
                                },
                                icon = { Icon(Icons.Filled.Add, contentDescription = tr("feeds.actions.addFeed")) },
                                text = { Text(tr("feeds.actions.addFeed")) },
                            )
                        }

                        coreAccounts.any { !accountSummaryIsRss(it) } -> {
                            ExtendedFloatingActionButton(
                                onClick = ::openCompose,
                                icon = { Icon(Icons.Filled.Edit, contentDescription = tr("mobile.tabs.compose")) },
                                text = { Text(tr("mobile.tabs.compose")) },
                            )
                        }
                    }
                },
                snackbarHost = { SnackbarHost(snackbarHost) },
            ) { innerPadding ->
                Column(Modifier.fillMaxSize().padding(innerPadding)) {
                    val reconnectAccount2 = selectedAccount?.takeIf { it.needsReconnect }
                    when {
                        reconnectAccount2 != null -> {
                            StatusBanner(
                                message = "Can't sign in to ${reconnectAccount2.email.ifBlank {
                                    reconnectAccount2.displayName
                                }}. Update the credentials to reconnect.",
                                isError = true,
                                actionLabel = "Reconnect",
                                onAction = { reconnectAccount(reconnectAccount2) },
                                onDismiss = null,
                            )
                        }

                        errorBanner != null -> {
                            val authLike = isAuthError(errorBanner!!)
                            StatusBanner(
                                message = errorBanner!!,
                                isError = true,
                                actionLabel = if (authLike && selectedAccount != null) "Reconnect" else "Retry",
                                onAction = {
                                    if (authLike && selectedAccount != null) {
                                        reconnectAccount(selectedAccount)
                                    } else {
                                        syncCoreThreads()
                                    }
                                },
                                onDismiss = { errorBanner = null },
                            )
                        }

                        !notificationPermissionGranted &&
                            !notificationBannerDismissed &&
                            coreAccounts.isNotEmpty() &&
                            initialThreadsLoaded &&
                            !syncing -> {
                            StatusBanner(
                                message = tr("mobile.mail.notificationsBannerText"),
                                isError = false,
                                icon = Icons.Outlined.NotificationsNone,
                                actionLabel = tr("mobile.mail.notificationsBannerEnable"),
                                onAction = {
                                    mobileHost.requestNotificationPermission()
                                    notificationPermissionGranted = mobileHost.notificationsEnabled()
                                },
                                onDismiss = {
                                    notificationBannerDismissed = true
                                    saveAppBoolean(prefs, NOTIFICATION_BANNER_DISMISSED_PREF, true)
                                },
                            )
                        }
                    }
                    val showingBlockingInboxLoad =
                        !initialAccountsLoaded ||
                            accountsLoading ||
                            (coreThreads.isEmpty() && (syncing || !initialThreadsLoaded))
                    LaunchedEffect(
                        screen,
                        initialAccountsLoaded,
                        accountsLoading,
                        coreAccounts.size,
                        coreThreads.size,
                        syncing,
                        initialThreadsLoaded,
                        selectedCoreAccountId,
                        selectedCoreFolder,
                    ) {
                        if (screen == Screen.Mail) {
                            Log.i(
                                "MailLoad",
                                "render mail blocking=$showingBlockingInboxLoad initialAccountsLoaded=$initialAccountsLoaded accountsLoading=$accountsLoading accounts=${coreAccounts.size} threads=${coreThreads.size} syncing=$syncing initialThreadsLoaded=$initialThreadsLoaded selectedAccount=$selectedCoreAccountId folder=$selectedCoreFolder",
                            )
                        }
                    }
                    LaunchedEffect(
                        screen,
                        showingBlockingInboxLoad,
                        activeMailboxLoadKey,
                        activeMailboxLoadStartedAtMillis,
                    ) {
                        if (screen != Screen.Mail || !showingBlockingInboxLoad) return@LaunchedEffect
                        delay(MAILBOX_BLOCKING_WARN_AFTER_MS)
                        val stillBlocking =
                            screen == Screen.Mail &&
                                (
                                    !initialAccountsLoaded ||
                                        accountsLoading ||
                                        (coreThreads.isEmpty() && (syncing || !initialThreadsLoaded))
                                )
                        if (stillBlocking && !blockingMailboxLoadWarned) {
                            blockingMailboxLoadWarned = true
                            Log.w(
                                "MailLoad",
                                "mail UI still blocking after ${MAILBOX_BLOCKING_WARN_AFTER_MS}ms accountsLoaded=$initialAccountsLoaded accountsLoading=$accountsLoading accounts=${coreAccounts.size} threads=${coreThreads.size} syncing=$syncing initialThreadsLoaded=$initialThreadsLoaded activeLoad=$activeMailboxLoadKey selectedAccount=$selectedCoreAccountId folder=$selectedCoreFolder",
                            )
                        }
                        delay(MAILBOX_BLOCKING_TIMEOUT_MS - MAILBOX_BLOCKING_WARN_AFTER_MS)
                        val stillBlocked = {
                            screen == Screen.Mail &&
                                coreAccounts.isNotEmpty() &&
                                coreThreads.isEmpty() &&
                                (syncing || !initialThreadsLoaded)
                        }
                        if (stillBlocked() && syncing) {
                            // The sync request is still in flight — a slow
                            // first sync, not a failure. Switch the loader
                            // copy and give it until the hard cap before
                            // treating it as an error.
                            Log.w(
                                "MailLoad",
                                "mail sync still running after ${MAILBOX_BLOCKING_TIMEOUT_MS}ms accounts=${coreAccounts.size} activeLoad=$activeMailboxLoadKey selectedAccount=$selectedCoreAccountId folder=$selectedCoreFolder",
                            )
                            blockingMailboxLoadSlow = true
                            delay(MAILBOX_BLOCKING_HARD_TIMEOUT_MS - MAILBOX_BLOCKING_TIMEOUT_MS)
                        }
                        if (stillBlocked()) {
                            Log.w(
                                "MailLoad",
                                "mail UI unblocked accounts=${coreAccounts.size} syncing=$syncing initialThreadsLoaded=$initialThreadsLoaded activeLoad=$activeMailboxLoadKey selectedAccount=$selectedCoreAccountId folder=$selectedCoreFolder",
                            )
                            syncing = false
                            initialThreadsLoaded = true
                            blockingMailboxLoadSlow = false
                            errorBanner = "Inbox load timed out. Pull to refresh or tap Retry."
                        }
                    }
                    PullToRefreshBox(
                        isRefreshing = syncing && !showingBlockingInboxLoad,
                        onRefresh = { syncCoreThreads() },
                        modifier = Modifier.fillMaxSize(),
                    ) {
                        when {
                            !initialAccountsLoaded || accountsLoading -> {
                                LoadingState("Loading your inbox…")
                            }

                            coreAccounts.isEmpty() -> {
                                EmptyState(
                                    icon = Icons.Filled.PersonAdd,
                                    title = tr("empty.welcomeTitle"),
                                    text = tr("empty.mailOrRssSetupText"),
                                    actionLabel = tr("accounts.actions.addAccount"),
                                    onAction = {
                                        addSection = 0
                                        passwordServerSettingsOpen = false
                                        screen = Screen.AddAccount
                                    },
                                )
                            }

                            coreThreads.isEmpty() && (syncing || !initialThreadsLoaded) -> {
                                LoadingState(
                                    if (selectedAccountIsRss) {
                                        if (blockingMailboxLoadSlow) {
                                            "Still syncing feeds… The first sync can take a while."
                                        } else {
                                            "Loading feeds…"
                                        }
                                    } else {
                                        if (blockingMailboxLoadSlow) {
                                            "Still syncing your inbox… The first sync can take a while."
                                        } else {
                                            "Loading your inbox…"
                                        }
                                    },
                                )
                            }

                            coreThreads.isEmpty() -> {
                                EmptyState(
                                    icon = Icons.Outlined.Drafts,
                                    title =
                                        if (mailSearch.isBlank() && mailFilter == FilterMode.All) {
                                            if (selectedAccountIsRss) tr("empty.noFeeds") else tr("empty.nothingHereYet")
                                        } else {
                                            tr("empty.noMatchingMail")
                                        },
                                    text =
                                        if (mailSearch.isBlank() && mailFilter == FilterMode.All) {
                                            if (selectedAccountIsRss) tr("empty.addFeedToStart") else tr("empty.pullLatestMessages")
                                        } else {
                                            tr("empty.adjustSearchFilter")
                                        },
                                    actionLabel = if (selectedAccountIsRss) null else tr("mobile.mail.syncMailbox"),
                                    onAction = if (selectedAccountIsRss) null else ({ syncCoreThreads() }),
                                )
                            }

                            else -> {
                                MailList(
                                    threads = coreThreads,
                                    accounts = coreAccounts,
                                    canLoadMore = pageableCoreAccounts().isNotEmpty(),
                                    loadingMore = loadingMoreThreads,
                                    onOpen = ::readCoreThread,
                                    onToggleStar = ::toggleStar,
                                    onArchive = ::archiveOrRemove,
                                    onDelete = ::deleteThread,
                                    onCopyFeedUrl = { thread ->
                                        services.copyText("Feed URL", thread.feedUrl)
                                        status = "Copied feed URL"
                                    },
                                    selectedThreadIds = selectedMailThreadIds,
                                    selectionActive = mailSelectionActive,
                                    onToggleSelected = { thread ->
                                        selectedMailThreadIds =
                                            if (thread.id in selectedMailThreadIds) {
                                                selectedMailThreadIds - thread.id
                                            } else {
                                                selectedMailThreadIds + thread.id
                                            }
                                        if (selectedMailThreadIds.isEmpty()) {
                                            mailSelectionMenuOpen = false
                                        }
                                    },
                                    onLongPress = { thread ->
                                        selectedMailThreadIds =
                                            if (thread.id in selectedMailThreadIds) {
                                                selectedMailThreadIds
                                            } else {
                                                selectedMailThreadIds + thread.id
                                            }
                                    },
                                    onLoadMore = { userInitiated ->
                                        loadMoreCoreThreads(quiet = !userInitiated)
                                    },
                                    showSenderImages = showSenderImages,
                                    showAccountBadge = selectedCoreAccountId == UNIFIED_ACCOUNT_ID,
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}
