package jp.nonbili.meron.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.runtime.setValue
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.backhandler.BackHandler
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.lifecycle.Lifecycle
import androidx.navigation.NavBackStackEntry
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import jp.nonbili.meron.shared.ComposeDraft
import jp.nonbili.meron.shared.ContactSuggestion
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.MobileCommand
import jp.nonbili.meron.shared.accountSummaryIsRss
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.io.encoding.ExperimentalEncodingApi

internal const val MAILBOX_BLOCKING_WARN_AFTER_MS = 10_000L
internal const val MAILBOX_BLOCKING_TIMEOUT_MS = 15_000L

// Hard cap for a blocking load whose sync is still in flight: past the soft
// timeout the loader shows "still syncing" copy, past this one we give up and
// surface the timeout error.
internal const val MAILBOX_BLOCKING_HARD_TIMEOUT_MS = 60_000L
private const val CORE_STATUS_POLL_MS = 500L

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MeronApp(
    core: MeronCore,
    coreLoaded: Boolean,
    prefs: AppPreferences,
    kanbanPrefs: AppPreferences,
    services: PlatformServices,
    locale: LocaleController,
    mobileHost: MobileHost = DefaultMobileHost(),
    coreInitJson: String,
    incomingMailtoDraft: ComposeDraft? = null,
    onMailtoDraftConsumed: () -> Unit = {},
    incomingOAuthCallbackUrl: String? = null,
    onOAuthCallbackConsumed: () -> Unit = {},
    incomingNotificationThreadTarget: NotificationThreadTarget? = null,
    onNotificationThreadTargetConsumed: () -> Unit = {},
    /**
     * Notified after the language has been stored and pushed to the OS, for hosts
     * that must react — Android recreates its Activity to reload resources.
     * Persistence is not the host's job: doing it there would bypass the
     * write-through store and never reach the `settings` table.
     */
    onLanguageApplied: (String) -> Unit = {},
) {
    val scope = rememberCoroutineScope()
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    // The core `settings` table is authoritative for these; the platform stores
    // stay in front of it as a write-through cache so the first frame can paint
    // before the keyed DB is open. See MobileSettings.kt.
    val coreLoadedNow = rememberUpdatedState(coreLoaded)
    val mirror = remember(core, prefs) { SettingsMirror(core, journal = prefs) { coreLoadedNow.value } }
    val backedPrefs =
        remember(prefs, mirror) { CoreBackedPreferences(prefs, PrefStore.App, scope, mirror) }
    val backedKanbanPrefs =
        remember(kanbanPrefs, mirror) {
            CoreBackedPreferences(kanbanPrefs, PrefStore.Kanban, scope, mirror)
        }
    val state =
        remember(core, backedPrefs, backedKanbanPrefs, services, locale, mobileHost) {
            MeronMobileState(
                scope = scope,
                core = core,
                coreLoaded = coreLoaded,
                prefs = backedPrefs,
                kanbanPrefs = backedKanbanPrefs,
                services = services,
                locale = locale,
                mobileHost = mobileHost,
                settingsMirror = mirror,
            )
        }
    // Appearance and language live here rather than in the hosts, so their writes
    // go through `backedPrefs` like every other setting. `putString` stages
    // synchronously, which is what makes them survive the Activity recreation the
    // language change triggers.
    var customThemes by remember(backedPrefs) { mutableStateOf(loadCustomThemes(backedPrefs)) }
    var themeChoice by remember(backedPrefs) { mutableStateOf(loadThemeChoice(backedPrefs, customThemes)) }
    val systemDark = isSystemInDarkTheme()
    // Android 13+ lets the language be changed from system settings while the app
    // is not running; that choice outranks the stored tag.
    val systemLanguageTag = remember(locale) { locale.systemLanguageTag() }
    var appLanguageTag by
        remember(backedPrefs, systemLanguageTag) {
            mutableStateOf(resolveAppLanguageTag(systemLanguageTag, loadAppLanguageTag(backedPrefs)))
        }
    LaunchedEffect(systemLanguageTag) {
        // Persisted as an effect rather than during composition, and through the
        // write-through store, so an externally chosen language reaches the table
        // instead of being read back as stale on the next hydrate. A reset to
        // "system default" persists too — it stores blank, clearing the old tag.
        if (appLanguageNeedsPersisting(systemLanguageTag, loadAppLanguageTag(backedPrefs))) {
            saveAppLanguageTag(backedPrefs, systemLanguageTag.orEmpty())
        }
    }
    val deviceLanguageTag = remember(locale) { locale.deviceLanguageTag() }
    val onThemeChoiceChange: (ThemeChoice) -> Unit = { choice ->
        themeChoice = choice
        saveThemeChoice(backedPrefs, choice)
    }
    // An imported theme is applied straight away, the way picking a swatch is.
    val onImportCustomTheme: (CustomTheme) -> Unit = { theme ->
        customThemes = customThemes + theme
        saveCustomThemes(backedPrefs, customThemes)
        onThemeChoiceChange(themeChoice.select(theme))
    }
    val onDeleteCustomTheme: (CustomTheme) -> Unit = { theme ->
        customThemes = customThemes.filterNot { it.id == theme.id }
        saveCustomThemes(backedPrefs, customThemes)
        onThemeChoiceChange(themeChoice.without(theme))
    }
    val onAppLanguageChange: (String) -> Unit = { tag ->
        appLanguageTag = tag
        saveAppLanguageTag(backedPrefs, tag)
        locale.applySystem(tag)
        onLanguageApplied(tag)
    }

    // Reconcile once the core is up: the table wins, which is what lets a
    // restored backup reach these settings without a separate channel.
    // The first commands wait while the core upgrades its store, with nothing to
    // say why. Ask, until they get through; a poll because the upgrade may have
    // begun before this composition could subscribe to any event.
    LaunchedEffect(coreLoaded) {
        if (!coreLoaded) return@LaunchedEffect
        while (!state.initialAccountsLoaded) {
            delay(CORE_STATUS_POLL_MS)
            val status = runCatching { withContext(ioDispatcher) { core.invoke(MobileCommand.CoreStatus) } }
            state.coreMigrating = status.getOrNull().orEmpty().contains("\"migrating\":true")
        }
        state.coreMigrating = false
    }
    LaunchedEffect(coreLoaded) {
        if (!coreLoaded) return@LaunchedEffect
        val changed = hydrateSettingsFromCore(prefs, kanbanPrefs, mirror)
        if (changed.isNotEmpty()) state.applyHydratedSettings(changed)
    }
    // Appearance and language are persisted by the host (and, for language, by
    // LocaleController, which normalizes the tag first), so the write-through
    // decorator never sees those writes — yet both are registry settings. Mirror
    // the cache after the host has stored it, or the table keeps the old value
    // and the next hydrate pulls it back over the user's choice.
    androidx.compose.runtime.CompositionLocalProvider(
        // Blank means "follow the system", so fall back to the device's language
        // rather than to English.
        LocalAppLocale provides appLanguageTag.ifBlank { resolveDeviceLanguageTag(deviceLanguageTag) },
        LocalMailTranslationLocale provides appLanguageTag.ifBlank { deviceLanguageTag },
        LocalPlatformServices provides services,
        LocalAvatarCore provides core,
        LocalShowThreadAvatars provides state.showThreadAvatars,
    ) {
        MeronTheme(
            appearanceMode = themeChoice.resolve(systemDark),
            messageFontScale = state.messageFontScale,
            darkMailBodies = state.darkMailBodies,
            autoFitMessages = state.autoFitMessages,
            scrollLongHtmlMessages = state.scrollLongHtmlMessages,
            chatFullMessages = state.chatFullMessages,
            pullUpToClose = state.pullUpToClose,
        ) {
            MeronMobileScreenContent(
                state = state,
                drawerState = drawerState,
                coreInitJson = coreInitJson,
                incomingMailtoDraft = incomingMailtoDraft,
                onMailtoDraftConsumed = onMailtoDraftConsumed,
                incomingOAuthCallbackUrl = incomingOAuthCallbackUrl,
                onOAuthCallbackConsumed = onOAuthCallbackConsumed,
                incomingNotificationThreadTarget = incomingNotificationThreadTarget,
                onNotificationThreadTargetConsumed = onNotificationThreadTargetConsumed,
                themeChoice = themeChoice,
                systemDark = systemDark,
                onThemeChoiceChange = onThemeChoiceChange,
                customThemes = customThemes,
                onImportCustomTheme = onImportCustomTheme,
                onDeleteCustomTheme = onDeleteCustomTheme,
                appLanguageTag = appLanguageTag,
                onAppLanguageChange = onAppLanguageChange,
                packageName = mobileHost.packageName,
                appVersion = mobileHost.appVersionName,
                coreProtocolVersion = mobileHost.coreProtocolVersion,
            )
        }
    }
}

// Mutating the back stack while a NavHost transition is still running can make
// AnimatedContent recompose an already-destroyed NavBackStackEntry, crashing in
// its ViewModelStore. Only pop/navigate once the current entry has settled.
internal fun canMutateBackStack(entryState: Lifecycle.State?): Boolean = entryState == null || entryState.isAtLeast(Lifecycle.State.RESUMED)

// Waits for the entry to leave its transition; returns false if it was
// destroyed instead (the back stack changed, so the caller's navigation
// request is stale and must be dropped).
private suspend fun NavBackStackEntry.awaitSettled(): Boolean {
    val state =
        lifecycle.currentStateFlow.first {
            it == Lifecycle.State.DESTROYED || it.isAtLeast(Lifecycle.State.RESUMED)
        }
    return state != Lifecycle.State.DESTROYED
}

private const val THREAD_STATE_KEY = "thread"

@OptIn(ExperimentalMaterial3Api::class, ExperimentalEncodingApi::class, ExperimentalComposeUiApi::class)
@Composable
private fun MeronMobileScreenContent(
    state: MeronMobileState,
    drawerState: androidx.compose.material3.DrawerState,
    coreInitJson: String,
    incomingMailtoDraft: ComposeDraft?,
    onMailtoDraftConsumed: () -> Unit,
    incomingOAuthCallbackUrl: String?,
    onOAuthCallbackConsumed: () -> Unit,
    incomingNotificationThreadTarget: NotificationThreadTarget?,
    onNotificationThreadTargetConsumed: () -> Unit,
    themeChoice: ThemeChoice,
    systemDark: Boolean,
    onThemeChoiceChange: (ThemeChoice) -> Unit,
    customThemes: List<CustomTheme>,
    onImportCustomTheme: (CustomTheme) -> Unit,
    onDeleteCustomTheme: (CustomTheme) -> Unit,
    appLanguageTag: String,
    onAppLanguageChange: (String) -> Unit,
    packageName: String,
    appVersion: String,
    coreProtocolVersion: Int,
) {
    with(state) {
        var contactToOpen by remember { mutableStateOf<ContactSuggestion?>(null) }
        val contactsState = rememberContactsScreenState(core)
        androidx.compose.runtime.CompositionLocalProvider(
            LocalCryptoState provides state,
            LocalOpenContact provides { contact: ContactSuggestion ->
                contactToOpen = contact
                flushQuickReplyAutosave()
                if (screen != Screen.Contacts && screen != Screen.Thread) previousTopScreen = screen
                screen = Screen.Contacts
            },
        ) {
            MailTaskListPicker(state)
            CryptoKeyChangeQuestion(state)
            val navController = rememberNavController()
            val currentBackStackEntry by navController.currentBackStackEntryAsState()
            val startRoute = remember { screen.route() }
            var pendingRoute by remember { mutableStateOf<String?>(null) }
            // A conversation is drawn over the screen it was opened from, which
            // stays the current route underneath it, live, for as long as it is open.
            var threadHostRoute by remember { mutableStateOf(AppRoutes.Mail) }
            LaunchedEffect(screen) {
                val currentEntry = currentBackStackEntry
                val currentRoute = currentEntry?.destination?.route
                if (screen == Screen.Thread && currentRoute != null) {
                    // Opened over whatever is showing; back from the editor, over
                    // what the editor was opened on.
                    threadHostRoute =
                        if (currentRoute == AppRoutes.Compose) {
                            navController.previousBackStackEntry?.destination?.route ?: threadHostRoute
                        } else {
                            currentRoute
                        }
                }
                val targetRoute = if (screen == Screen.Thread) threadHostRoute else screen.route()
                if (currentRoute == null) {
                    pendingRoute = targetRoute
                } else if (currentRoute != targetRoute) {
                    if (!currentEntry.awaitSettled()) return@LaunchedEffect
                    if (!navController.popBackStack(targetRoute, inclusive = false)) {
                        navController.navigate(targetRoute) {
                            launchSingleTop = true
                        }
                    }
                }
            }
            LaunchedEffect(currentBackStackEntry?.destination?.route) {
                val currentEntry = currentBackStackEntry
                val currentRoute = currentEntry?.destination?.route
                val queuedRoute = pendingRoute
                if (queuedRoute != null && currentRoute != null && currentRoute != queuedRoute) {
                    pendingRoute = null
                    if (!currentEntry.awaitSettled()) return@LaunchedEffect
                    navController.navigate(queuedRoute) {
                        launchSingleTop = true
                    }
                    return@LaunchedEffect
                }
                pendingRoute = null
                val destinationScreen = appRouteToScreen(currentRoute)
                // A conversation open over its route is where the app is, too.
                val overHost = screen == Screen.Thread && currentRoute == threadHostRoute
                if (destinationScreen != null && screen != destinationScreen && !overHost) {
                    screen = destinationScreen
                }
            }
            val popAppBack: () -> Unit = {
                if (screen == Screen.Thread) {
                    flushQuickReplyAutosave()
                    // The screen underneath never left; closing the conversation
                    // only uncovers it.
                    screen = appRouteToScreen(navController.currentBackStackEntry?.destination?.route) ?: previousTopScreen
                } else if (canMutateBackStack(navController.currentBackStackEntry?.lifecycle?.currentState)) {
                    if (screen == Screen.Contacts && navController.previousBackStackEntry == null) {
                        val destination = rootBackDestination(screen, previousTopScreen)
                        navController.navigate(destination.route()) {
                            popUpTo(AppRoutes.Contacts) { inclusive = true }
                            launchSingleTop = true
                        }
                        screen = destination
                    } else if (!navController.popBackStack()) {
                        screen = rootBackDestination(screen, previousTopScreen)
                    }
                }
            }
            CoreEventSubscription(state)
            CoreSyncEffects(state)
            val importOpml: (PickedFile?) -> Unit = { importOpmlFile(it) }
            installFileSaveLaunchers()
            val pickBackupFile: () -> Unit = { pickBackupFileToRestore() }
            val pickAttachmentInto: ((DraftAttachment) -> Unit) -> Unit = { pickDraftAttachment(it) }
            val pickAccountMedia: () -> Unit = { pickAccountMediaFile() }
            val pickKanbanBoardMedia: () -> Unit = { pickKanbanBoardMediaFile() }

            AppStartupEffects(
                state = state,
                incomingMailtoDraft = incomingMailtoDraft,
                onMailtoDraftConsumed = onMailtoDraftConsumed,
                incomingOAuthCallbackUrl = incomingOAuthCallbackUrl,
                onOAuthCallbackConsumed = onOAuthCallbackConsumed,
                incomingNotificationThreadTarget = incomingNotificationThreadTarget,
                onNotificationThreadTargetConsumed = onNotificationThreadTargetConsumed,
            )
            val selectedAccount = coreAccounts.firstOrNull { it.id == selectedCoreAccountId }
            val selectedAccountIsRss = selectedAccount != null && accountSummaryIsRss(selectedAccount)
            val selectedThreadAccount = selectedCoreThread?.accountId?.let { accountId -> coreAccounts.firstOrNull { it.id == accountId } }
            val selectedThreadAccountId = selectedThreadAccount?.id.orEmpty()
            val drawerFolders = foldersByAccount.values.flatten().ifEmpty { coreFolders }
            val selectedThreadPreferHtml =
                selectedThreadAccount?.let { account ->
                    conversationHtmlOverrides[account.id] ?: account.conversationHtml
                } ?: true
            val activeKanbanBoard = kanbanBoards.firstOrNull { it.id == activeKanbanBoardId } ?: kanbanBoards.firstOrNull()
            val appBarTitle = tr(if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID) "kanban.columns.unifiedInbox" else "folders.roles.inbox")
            val appBarSubtitle =
                if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID) {
                    tr("kanban.allAccounts")
                } else {
                    selectedAccount?.email?.ifBlank { selectedAccount.displayName }.orEmpty()
                }
            val selectedMailThreads =
                (coreThreads + kanbanColumns.values.flatMap { it.threads })
                    .distinctBy { it.id }
                    .filter { it.id in selectedMailThreadIds }
            val mailSelectionActive = selectedMailThreadIds.isNotEmpty()

            LaunchedEffect(screen, coreThreads, kanbanColumns) {
                val visibleThreadIds =
                    if (screen == Screen.Kanban) {
                        kanbanColumns.values
                            .flatMap { it.threads }
                            .map { it.id }
                            .toSet()
                    } else {
                        coreThreads.map { it.id }.toSet()
                    }
                val retainedThreadIds = selectedMailThreadIds.intersect(visibleThreadIds)
                if (retainedThreadIds.size != selectedMailThreadIds.size) {
                    selectedMailThreadIds = retainedThreadIds
                }
            }

            // MailRouteContent leaves composition while another route is open.
            // Keep its per-mailbox list states above the NavHost so returning to
            // the thread list restores the position that was visible before.
            val mailListStates = remember { mutableMapOf<MailboxCacheKey, MailListPosition>() }

            val threadOpen = screen == Screen.Thread
            val threadStateHolder = rememberSaveableStateHolder()
            LaunchedEffect(screen) {
                if (screen != Screen.Thread && screen != Screen.Compose) threadStateHolder.removeState(THREAD_STATE_KEY)
            }
            val threadVisibility = remember { MutableTransitionState(threadOpen) }
            threadVisibility.targetState = threadOpen
            val pull = rememberPullToDismissState()
            val focusManager = LocalFocusManager.current
            LaunchedEffect(threadOpen) {
                if (threadOpen) {
                    // Each opening starts the conversation in place, after a pull
                    // took the last one off screen.
                    pull.reset()
                    // The screen underneath stays composed, a search field in it
                    // focused, keyboard and all.
                    focusManager.clearFocus(force = true)
                } else {
                    // Closed some other way while a pull was settling.
                    pull.stop()
                }
            }
            // The screen underneath, fully hidden by a settled conversation: it
            // keeps its state but is neither drawn nor read out.
            val pulledAway by remember { derivedStateOf { pull.distancePx != 0f } }
            val threadSettled by remember { derivedStateOf { threadVisibility.isIdle && threadVisibility.currentState } }
            val hostCovered by remember {
                derivedStateOf { threadVisibility.isIdle && threadVisibility.currentState && pull.distancePx == 0f }
            }

            Box(Modifier.fillMaxSize()) {
                NavHost(
                    navController = navController,
                    startDestination = startRoute,
                    modifier =
                        Modifier
                            .drawWithContent { if (!hostCovered) drawContent() }
                            // Nor can focus move into it under the conversation.
                            .focusProperties { onEnter = { if (threadOpen) cancelFocusChange() } }
                            .then(if (hostCovered) Modifier.clearAndSetSemantics {} else Modifier),
                ) {
                    composable(AppRoutes.Compose) {
                        ComposeRouteContent(
                            state = state,
                            pickAttachmentInto = pickAttachmentInto,
                        )
                    }

                    composable(AppRoutes.AddAccount) {
                        AddAccountRouteContent(
                            state = state,
                            popAppBack = popAppBack,
                        )
                    }

                    composable(AppRoutes.Settings) {
                        SettingsRouteContent(
                            state = state,
                            themeChoice = themeChoice,
                            systemDark = systemDark,
                            onThemeChoiceChange = onThemeChoiceChange,
                            customThemes = customThemes,
                            onImportCustomTheme = onImportCustomTheme,
                            onDeleteCustomTheme = onDeleteCustomTheme,
                            appLanguageTag = appLanguageTag,
                            onAppLanguageChange = onAppLanguageChange,
                            popAppBack = popAppBack,
                            pickBackupFile = pickBackupFile,
                            pickAccountMedia = pickAccountMedia,
                            pickKanbanBoardMedia = pickKanbanBoardMedia,
                        )
                    }

                    composable(AppRoutes.Kanban) {
                        KanbanRouteContent(
                            state = state,
                            drawerState = drawerState,
                            drawerFolders = drawerFolders,
                            mailSelectionActive = mailSelectionActive,
                            selectedMailThreads = selectedMailThreads,
                            activeKanbanBoard = activeKanbanBoard,
                        )
                    }

                    composable(AppRoutes.Tasks) {
                        TasksRouteContent(
                            state = state,
                            drawerState = drawerState,
                            drawerFolders = drawerFolders,
                        )
                    }

                    composable(AppRoutes.Contacts) {
                        ContactsScreen(
                            core = core,
                            state = contactsState,
                            accounts = coreAccounts,
                            foldersByAccount = foldersByAccount,
                            showSenderImages = showSenderImages,
                            initialContact = contactToOpen,
                            onInitialContactConsumed = { contactToOpen = null },
                            active = screen == Screen.Contacts,
                            canCompose = composeIdentityCandidates().isNotEmpty(),
                            onBack = {
                                contactToOpen = null
                                popAppBack()
                            },
                            onCompose = { address -> openComposeTo(address, defaultSendAccountId(), returnScreen = Screen.Contacts) },
                            onOpenThread = { thread -> readCoreThread(thread) },
                            onCopy = { label, value -> services.copyText(label, value) },
                            mobile = this@with,
                            drawerState = drawerState,
                            drawerContent = { ViewsDrawer(state, drawerState, drawerFolders) },
                        )
                    }

                    composable(AppRoutes.Calendar) {
                        CalendarRouteContent(
                            state = state,
                            drawerState = drawerState,
                            drawerFolders = drawerFolders,
                        )
                    }

                    composable(AppRoutes.Mail) {
                        MailRouteContent(
                            state = state,
                            drawerState = drawerState,
                            drawerFolders = drawerFolders,
                            selectedAccount = selectedAccount,
                            selectedAccountIsRss = selectedAccountIsRss,
                            mailSelectionActive = mailSelectionActive,
                            selectedMailThreads = selectedMailThreads,
                            importOpml = importOpml,
                            mailListStates = mailListStates,
                        )
                    }
                }

                // An open drawer closes before back leaves the screen under it;
                // after the routes, so it comes before their own back handling.
                BackHandler(enabled = drawerState.isOpen) { scope.launch { drawerState.close() } }

                // The open conversation, over the route it was opened from.
                AnimatedVisibility(
                    visibleState = threadVisibility,
                    enter = fadeIn(tween(700)),
                    // Pulled away, it is off screen already, and must not keep the
                    // screen it uncovered from its touches while it fades.
                    exit = if (pulledAway) ExitTransition.None else fadeOut(tween(700)),
                ) {
                    // Its web views draw only once it has settled: hwui crashes drawing
                    // one into the offscreen layer the fade composites through.
                    CompositionLocalProvider(LocalMailWebViewDrawable provides threadSettled) {
                        // Kept while the editor is open over it, so a reply written in
                        // full comes back to the conversation as it was left.
                        threadStateHolder.SaveableStateProvider(THREAD_STATE_KEY) {
                            OpenThreadPager(
                                state = state,
                                threadOpen = threadOpen,
                                popAppBack = popAppBack,
                                pickAttachmentInto = pickAttachmentInto,
                                selectedThreadAccountId = selectedThreadAccountId,
                                pull = pull,
                            )
                        }
                    }
                }
            }

            MeronAppDialogs(
                state = state,
                appVersion = appVersion,
                activeKanbanBoard = activeKanbanBoard,
            )
        }
    }
}
