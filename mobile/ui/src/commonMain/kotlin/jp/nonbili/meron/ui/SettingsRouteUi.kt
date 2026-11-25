package jp.nonbili.meron.ui

import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.ExperimentalComposeUiApi
import jp.nonbili.meron.shared.nextPollIntervalMinutes
import kotlin.io.encoding.ExperimentalEncodingApi

@OptIn(ExperimentalMaterial3Api::class, ExperimentalEncodingApi::class, ExperimentalComposeUiApi::class)
@Composable
internal fun SettingsRouteContent(
    state: MeronMobileState,
    themeChoice: ThemeChoice,
    systemDark: Boolean,
    onThemeChoiceChange: (ThemeChoice) -> Unit,
    customThemes: List<CustomTheme>,
    onImportCustomTheme: (CustomTheme) -> Unit,
    onDeleteCustomTheme: (CustomTheme) -> Unit,
    appLanguageTag: String,
    onAppLanguageChange: (String) -> Unit,
    popAppBack: () -> Unit,
    pickBackupFile: () -> Unit,
    pickAccountMedia: () -> Unit,
    pickKanbanBoardMedia: () -> Unit,
) {
    with(state) {
        LaunchedEffect(Unit) { loadStorageUsage() }
        SettingsScreen(
            rulesPage = { modifier, onEditor -> RulesPage(modifier, onEditor) },
            calendarsPage = { modifier -> CalendarsTasksPage(this@with, modifier) },
            onOpenTags = { showTagsDialog = true },
            onBack = popAppBack,
            initialGeneral = settingsGeneralTarget,
            onConsumeInitialGeneral = { settingsGeneralTarget = false },
            initialAccountId = accountSettingsTargetId,
            initialAccountProxy =
                accountSettingsTargetId != null &&
                    accountSettingsTargetId == accountSettingsProxyTargetId,
            onConsumeInitialAccount = {
                accountSettingsTargetId = null
                accountSettingsProxyTargetId = null
            },
            initialKanbanBoardId = kanbanSettingsTargetId,
            onConsumeInitialKanbanBoard = { kanbanSettingsTargetId = null },
            accounts = coreAccounts,
            hiddenNavigationAccountIds = hiddenNavigationAccountIds,
            kanbanBoards = kanbanBoards,
            onSaveKanbanBoard = { board, name, avatarUrl, wallpaperPresetId, wallpaperUrl ->
                updateKanbanBoard(board.id, name, avatarUrl, wallpaperPresetId, wallpaperUrl)
            },
            onDeleteKanbanBoard = { board ->
                deleteKanbanBoard(board.id)
            },
            onCreateKanbanBoard = ::createKanbanBoard,
            onAddMailAccount = {
                resetPasswordAccountForm()
                addSection = 0
                previousTopScreen = Screen.Settings
                screen = Screen.AddAccount
            },
            onAddFeedAccount = {
                resetPasswordAccountForm()
                addSection = 1
                previousTopScreen = Screen.Settings
                screen = Screen.AddAccount
            },
            onSaveAccountSettings = {
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
                ->
                setAccountNavigationVisible(account, showInNavigation)
                saveAccountSettings(
                    account,
                    displayName,
                    senderName,
                    avatarUrl,
                    wallpaperPresetId,
                    loadRemoteImages,
                    conversationHtml,
                    includedInUnified,
                    muted,
                    paused,
                    interval,
                    aliases,
                )
            },
            onPickAccountAvatar = { account ->
                accountMediaUploadTarget = AccountMediaUploadTarget(account, wallpaper = false)
                pickAccountMedia()
            },
            onPickAccountWallpaper = { account ->
                accountMediaUploadTarget = AccountMediaUploadTarget(account, wallpaper = true)
                pickAccountMedia()
            },
            onPickKanbanBoardAvatar = { board ->
                kanbanBoardMediaTarget = KanbanBoardMediaTarget(board, wallpaper = false)
                pickKanbanBoardMedia()
            },
            onPickKanbanBoardWallpaper = { board ->
                kanbanBoardMediaTarget = KanbanBoardMediaTarget(board, wallpaper = true)
                pickKanbanBoardMedia()
            },
            onMoveAccountUp = { account -> moveAccount(account, -1) },
            onMoveAccountDown = { account -> moveAccount(account, 1) },
            onRemoveAccount = ::removeAccount,
            themeChoice = themeChoice,
            systemDark = systemDark,
            onThemeChoiceChange = onThemeChoiceChange,
            customThemes = customThemes,
            onImportCustomTheme = onImportCustomTheme,
            onDeleteCustomTheme = onDeleteCustomTheme,
            onShareCustomTheme = { theme -> mobileHost.shareText(serializeThemeSource(theme.source)) },
            appLanguageTag = appLanguageTag,
            onAppLanguageChange = onAppLanguageChange,
            showSenderImages = showSenderImages,
            onToggleSenderImages = {
                showSenderImages = !showSenderImages
                saveAppBoolean(prefs, SHOW_SENDER_IMAGES_PREF, showSenderImages)
            },
            showThreadAvatars = showThreadAvatars,
            onToggleThreadAvatars = {
                showThreadAvatars = !showThreadAvatars
                saveAppBoolean(prefs, SHOW_THREAD_AVATARS_PREF, showThreadAvatars)
            },
            darkMailBodies = darkMailBodies,
            onToggleDarkMailBodies = {
                darkMailBodies = !darkMailBodies
                saveAppBoolean(prefs, DARK_MAIL_BODIES_PREF, darkMailBodies)
            },
            chatFullMessages = chatFullMessages,
            onToggleChatFullMessages = {
                chatFullMessages = !chatFullMessages
                saveAppBoolean(prefs, CHAT_FULL_MESSAGES_PREF, chatFullMessages)
            },
            pullUpToClose = pullUpToClose,
            onTogglePullUpToClose = {
                pullUpToClose = !pullUpToClose
                saveAppBoolean(prefs, PULL_UP_TO_CLOSE_PREF, pullUpToClose)
            },
            autoFitMessages = autoFitMessages,
            onToggleAutoFitMessages = {
                autoFitMessages = !autoFitMessages
                saveAppBoolean(prefs, AUTO_FIT_MESSAGES_PREF, autoFitMessages)
            },
            scrollLongHtmlMessages = scrollLongHtmlMessages,
            onToggleScrollLongHtmlMessages = {
                scrollLongHtmlMessages = !scrollLongHtmlMessages
                saveAppBoolean(prefs, SCROLL_LONG_HTML_MESSAGES_PREF, scrollLongHtmlMessages)
            },
            conversationFooter = conversationFooter,
            onConversationFooterChange = { footer ->
                setConversationFooter(footer)
                saveConversationFooter(prefs, footer)
            },
            conversationToolbar = conversationToolbar,
            onConversationToolbarChange = { actions ->
                conversationToolbar = actions
                saveConversationToolbar(prefs, actions)
            },
            showUnreadBadges = showUnreadBadges,
            onToggleUnreadBadges = {
                showUnreadBadges = !showUnreadBadges
                saveAppBoolean(prefs, SHOW_UNREAD_BADGES_PREF, showUnreadBadges)
            },
            showUnifiedInboxNav = showUnifiedInboxNav,
            onToggleUnifiedInboxNav = {
                showUnifiedInboxNav = !showUnifiedInboxNav
                saveAppBoolean(prefs, SHOW_UNIFIED_INBOX_PREF, showUnifiedInboxNav)
            },
            sendShortcutMode = sendShortcutMode,
            onToggleSendShortcut = {
                val next = sendShortcutMode.next()
                sendShortcutMode = next
                saveSendShortcutMode(prefs, next)
            },
            quoteReplies = quoteReplies,
            onToggleQuoteReplies = {
                quoteReplies = !quoteReplies
                saveAppBoolean(prefs, QUOTE_REPLIES_PREF, quoteReplies)
            },
            conversationLayout = conversationLayout,
            onToggleConversationLayout = {
                val next = conversationLayout.next()
                conversationLayout = next
                saveConversationLayout(prefs, next)
            },
            messageFontScale = messageFontScale,
            onMessageFontScaleChange = { scale ->
                messageFontScale = coerceMessageFontScale(scale)
                saveMessageFontScale(prefs, messageFontScale)
            },
            appProxy = appProxy,
            onSaveAppProxy = ::saveAppProxy,
            appSignatureHtml = appSignatureHtml,
            onSaveAppSignature = ::saveAppSignature,
            remoteImageSenders = remoteImageSenders,
            detectOneTimeCodes = detectOneTimeCodes,
            onToggleDetectOneTimeCodes = { saveDetectOneTimeCodes(!detectOneTimeCodes) },
            onAddRemoteImageSender = { addr -> setRemoteImageSender(addr, true) },
            onRemoveRemoteImageSender = { addr -> setRemoteImageSender(addr, false) },
            onSaveAccountSignature = ::saveAccountSignature,
            onSaveAccountColor = ::saveAccountColor,
            onSaveAccountProxy = ::saveAccountProxy,
            onSaveAccountServerSettings = ::saveAccountServerSettings,
            foldersByAccount = foldersByAccount,
            onRequestAccountFolders = { account -> ensureAccountFolders(account.id) },
            onSetFolderNotify = { folder, enabled ->
                setMailFolderNotify(
                    accountId = folder.accountId,
                    folderId = folder.name,
                    folderName = folder.displayName,
                    enabled = enabled,
                )
            },
            kanbanColumnWidth = kanbanColumnWidth,
            onCycleKanbanColumnWidth = {
                val next = nextKanbanColumnWidth(kanbanColumnWidth)
                kanbanColumnWidth = next
                saveAppInt(prefs, KANBAN_COLUMN_WIDTH_PREF, next)
            },
            notificationsNeedPermission = !notificationPermissionGranted,
            onEnableNotifications = {
                mobileHost.requestNotificationPermission()
                notificationPermissionGranted = mobileHost.notificationsEnabled()
            },
            supportsBackgroundPush = mobileHost.supportsBackgroundPush,
            liveMailPushEnabled = liveMailPushEnabled,
            onToggleLiveMailPush = {
                val next = !liveMailPushEnabled
                liveMailPushEnabled = next
                saveAppBoolean(prefs, LIVE_MAIL_PUSH_PREF, next)
                mobileHost.syncLiveMailPush(next)
                status = trs(if (next) "mobile.status.liveMailPushEnabled" else "mobile.ios.liveMailPushDisabled")
            },
            notificationActions = notificationActions,
            onToggleNotificationAction = { action ->
                val next = if (action in notificationActions) notificationActions - action else notificationActions + action
                notificationActions = next
                prefs.putStringSet(NOTIFICATION_ACTIONS_PREF, next)
            },
            backgroundSyncEnabled = backgroundSyncEnabled,
            onToggleBackgroundSync = {
                val next = !backgroundSyncEnabled
                backgroundSyncEnabled = next
                saveAppBoolean(prefs, BACKGROUND_SYNC_ENABLED_PREF, next)
                mobileHost.syncBackgroundRefresh(next)
                status = trs(if (next) "mobile.status.backgroundSyncEnabled" else "mobile.status.backgroundSyncDisabled")
            },
            onRefreshBackground = {
                mobileHost.runBackgroundRefreshOnce()
                status = trs("mobile.status.backgroundRefreshQueued")
            },
            readDiagnosticLog = { mobileHost.readDiagnosticLog() },
            onShareDiagnosticLog = { mobileHost.shareDiagnosticLog() },
            pollIntervalMinutes = pollIntervalMinutes,
            onCyclePollInterval = {
                val next = nextPollIntervalMinutes(pollIntervalMinutes)
                pollIntervalMinutes = next
                saveAppInt(prefs, POLL_INTERVAL_MINUTES_PREF, next)
            },
            storageUsage = storageUsage,
            storageBusy = storageBusy,
            storageClearConfirming = storageClearConfirming,
            onRefreshStorage = { loadStorageUsage(showStatus = true) },
            onClearStorageCache = ::clearStorageCache,
            onExportBackup = {
                backupPassphraseError = ""
                backupPassphraseMode = BackupPassphraseMode.Export
            },
            onRestoreBackup = pickBackupFile,
            backupBusy = backupBusy,
        )
        backupPassphraseMode?.let { mode ->
            BackupPassphraseDialog(
                mode = mode,
                busy = backupBusy,
                error = backupPassphraseError,
                onDismiss = ::closeBackupPassphrase,
                onConfirm = { passphrase, includeSecrets ->
                    when (mode) {
                        BackupPassphraseMode.Export -> exportBackup(includeSecrets, passphrase)
                        BackupPassphraseMode.Restore -> retryBackupRestore(passphrase)
                    }
                },
            )
        }
    }
}
