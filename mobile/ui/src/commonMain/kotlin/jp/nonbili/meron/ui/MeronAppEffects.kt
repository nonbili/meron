package jp.nonbili.meron.ui

import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.SnackbarDuration
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.repeatOnLifecycle
import jp.nonbili.meron.shared.ComposeDraft
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlin.io.encoding.ExperimentalEncodingApi

/** Core events: refresh whatever on screen they touch, and route core logs to the platform logger. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalEncodingApi::class, ExperimentalComposeUiApi::class)
@Composable
internal fun CoreEventSubscription(
    state: MeronMobileState,
) {
    with(state) {
        DisposableEffect(Unit) {
            val handle =
                if (coreLoaded) {
                    core.events().subscribe { event ->
                        when (event.name) {
                            "mail.newMessages" -> {
                                if (!liveMailPushEnabled) {
                                    mobileHost.notifyNewMail(event.detailJson)
                                }
                                val eventAccount = event.detailJson.jsonStringValue("account")
                                val eventFolder = event.detailJson.jsonStringValue("folder")
                                scope.launch {
                                    reloadVisibleMailboxFor(eventAccount, eventFolder)
                                    refreshKanbanColumnsForMailEvent(eventAccount, eventFolder)
                                    refreshOpenThreadFor(eventAccount)
                                }
                            }

                            "mail.synced" -> {
                                val eventAccount = event.detailJson.jsonStringValue("account")
                                val eventFolder = event.detailJson.jsonStringValue("folder")
                                scope.launch {
                                    reloadVisibleMailboxFor(eventAccount, eventFolder)
                                    refreshKanbanColumnsForMailEvent(eventAccount, eventFolder)
                                    refreshOpenThreadFor(eventAccount)
                                }
                            }

                            // A mailbox changed outside the UI — a notification's
                            // Archive or Mark as read. The mail is already gone
                            // from the server, so the open list is stale in
                            // exactly the way a sync would have corrected.
                            "mail.mailboxChanged" -> {
                                val eventAccount = event.detailJson.jsonStringValue("account")
                                val eventFolder = event.detailJson.jsonStringValue("folder")
                                scope.launch {
                                    reloadVisibleMailboxFor(eventAccount, eventFolder)
                                    refreshKanbanColumnsForMailEvent(eventAccount, eventFolder)
                                    refreshOpenThreadFor(eventAccount)
                                }
                            }

                            // A Sent copy the provider filed seconds after SMTP
                            // returned, too late for the reload the send itself
                            // ran. It adds a message to its conversation, so the
                            // row's count (which spans folders) is one short. It
                            // names no folder: the row sits in whichever mailbox
                            // holds the conversation, so every column of the
                            // account re-reads.
                            "mail.sentCopyCached" -> {
                                val eventAccount = event.detailJson.jsonStringValue("account")
                                scope.launch {
                                    reloadVisibleMailboxFor(eventAccount, "")
                                    refreshKanbanColumnsForAccount(eventAccount)
                                    refreshOpenThreadFor(eventAccount)
                                }
                            }

                            // The deferred sync tail (body prefetch, Sent/Drafts
                            // headers) never changes the open folder's thread
                            // list, so only the open thread needs a refresh —
                            // reloading the list here would reset pagination.
                            "mail.tailSynced" -> {
                                val eventAccount = event.detailJson.jsonStringValue("account")
                                scope.launch {
                                    refreshOpenThreadFor(eventAccount)
                                }
                            }

                            // Synced calendars, task lists and address books
                            // changed: whatever shows them reads again.
                            "calendar.changed", "caldav.changed" -> {
                                scope.launch {
                                    if (event.name == "caldav.changed") loadCalendarAccounts()
                                    loadSyncedCalendars()
                                    if (screen == Screen.Calendar) loadCalendar()
                                }
                            }

                            "tasks.changed" -> {
                                val method = event.detailJson.jsonStringValue("method")
                                scope.launch {
                                    if (screen == Screen.Tasks) refreshTasksAfterChange(method)
                                    // The calendar shows tasks due, too.
                                    if (screen == Screen.Calendar && tasksEnabled) loadCalendar()
                                }
                            }

                            "googleTasks.changed" -> {
                                scope.launch { loadGoogleTasksAccounts() }
                            }

                            "carddav.changed" -> {
                                addressBooksVersion++
                            }

                            "account.needsReconnect" -> {
                                refreshAccountReconnectState(event.detailJson.jsonStringValue("account"))
                            }

                            "error" -> {
                                // Watcher/runtime failures are regular core events,
                                // not `log` events. Keep them visible in Logcat and
                                // the redacted on-device diagnostic log.
                                Log.e("core/event", event.detailJson.jsonStringValue("message"))
                            }

                            "log" -> {
                                // Surface Rust core logs through the platform logger
                                // (os_log / Logcat); they'd otherwise be invisible
                                // on device.
                                val tag = "core/" + event.detailJson.jsonStringValue("tag")
                                val message = event.detailJson.jsonStringValue("message")
                                when (event.detailJson.jsonStringValue("level")) {
                                    "DEBUG" -> Log.d(tag, message)
                                    "INFO" -> Log.i(tag, message)
                                    "WARN" -> Log.w(tag, message)
                                    else -> Log.e(tag, message)
                                }
                                // A core panic aborts the process right after
                                // this line, so mark it here; the JVM crash
                                // handler never runs for a native abort.
                                if (tag == "core/panic") {
                                    mobileHost.noteCoreCrash(message)
                                }
                            }
                        }
                    }
                } else {
                    null
                }
            onDispose { handle?.close() }
        }
    }
}

/** Background and foreground sync, kept in step with the app's switches. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalEncodingApi::class, ExperimentalComposeUiApi::class)
@Composable
internal fun CoreSyncEffects(
    state: MeronMobileState,
) {
    with(state) {
        // Before anything syncs: Google grants refresh with the app's
        // own client, and task-list sync follows the Tasks switch.
        LaunchedEffect(coreLoaded) {
            if (!coreLoaded) return@LaunchedEffect
            configureCoreOAuth()
            loadSyncedCalendars()
        }
        // The core's Tasks switch pauses task-list sync: kept in step with
        // the app's however it changes, a restored backup included.
        LaunchedEffect(coreLoaded, tasksEnabled) {
            if (coreLoaded) mirrorTasksEnabled()
        }
        // Calendars, task lists and Google Tasks at their own intervals while
        // the app is open, as desktop's sidecar polls them. The first round
        // waits for the screen's own reads.
        // Only while the app is on screen: in the background the platform's
        // refresh worker syncs, if the user allows it.
        val pollLifecycle = androidx.lifecycle.compose.LocalLifecycleOwner.current.lifecycle
        LaunchedEffect(coreLoaded, calendarEnabled, tasksEnabled, pollLifecycle) {
            if (!coreLoaded || (!calendarEnabled && !tasksEnabled)) return@LaunchedEffect
            val lastRun = mutableMapOf<String, Long>()
            pollLifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
                delay(5_000)
                while (true) {
                    pollSyncServices(lastRun)
                    delay(60_000)
                }
            }
        }
        LaunchedEffect(liveMailPushEnabled) {
            mobileHost.syncLiveMailPush(liveMailPushEnabled)
        }
        LaunchedEffect(backgroundSyncEnabled) {
            mobileHost.syncBackgroundRefresh(backgroundSyncEnabled)
        }
        // Foreground poll for platforms without a real background channel (iOS):
        // re-sync the visible mailbox on the chosen interval while the app is
        // open. Honors "Off" (interval 0). The timer suspends with the app and
        // resumes on return; the foreground signal below covers the gap after a
        // long suspension.
        if (!mobileHost.supportsBackgroundPush) {
            LaunchedEffect(pollIntervalMinutes, coreLoaded) {
                if (!coreLoaded || pollIntervalMinutes <= 0) return@LaunchedEffect
                while (true) {
                    delay(pollIntervalMinutes * 60_000L)
                    syncCoreThreads(
                        accountOverride = selectedCoreAccountId,
                        folderOverride = selectedCoreFolder,
                        syncFirst = true,
                    )
                }
            }
        }
        // Refresh the visible mailbox whenever the app comes back to the
        // foreground. Something else kept mail arriving while it was away —
        // Android's background worker, iOS's poll — and on Android the activity
        // may have been stopped for all of it, leaving the list showing what was
        // on screen when the user left even though the store has moved on. A
        // store re-read is enough to close that gap, so only platforms that have
        // to fetch for themselves (iOS, and only when its poll is on) go to the
        // network here.
        LaunchedEffect(coreLoaded, pollIntervalMinutes) {
            if (!coreLoaded) return@LaunchedEffect
            AppForegroundSignal.events.collect {
                syncCoreThreads(
                    accountOverride = selectedCoreAccountId,
                    folderOverride = selectedCoreFolder,
                    syncFirst = !mobileHost.supportsBackgroundPush && pollIntervalMinutes > 0,
                )
            }
        }
        LaunchedEffect(Unit) {
            NotificationPermissionSignal.events.collect {
                notificationPermissionGranted = mobileHost.notificationsEnabled()
            }
        }
    }
}

/** Startup reads, incoming links and notifications, and the status snackbar. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalEncodingApi::class, ExperimentalComposeUiApi::class)
@Composable
internal fun AppStartupEffects(
    state: MeronMobileState,
    incomingMailtoDraft: ComposeDraft?,
    onMailtoDraftConsumed: () -> Unit,
    incomingOAuthCallbackUrl: String?,
    onOAuthCallbackConsumed: () -> Unit,
    incomingNotificationThreadTarget: NotificationThreadTarget?,
    onNotificationThreadTargetConsumed: () -> Unit,
) {
    with(state) {
        LaunchedEffect(storageClearConfirming) {
            if (storageClearConfirming) {
                delay(4_000)
                storageClearConfirming = false
            }
        }

        LaunchedEffect(status, statusIsError) {
            if (status.isNotBlank()) {
                snackbarHost.showSnackbar(
                    message = status,
                    withDismissAction = statusIsError,
                    duration = if (statusIsError) SnackbarDuration.Indefinite else SnackbarDuration.Short,
                )
                if (statusIsError) status = ""
            }
        }

        // A `mailto:` link can arrive before the core has answered with the
        // accounts and the app-wide signature — on a cold start it usually
        // does. Composing then would seed a draft from an empty account list
        // and record that it has no signature, so this waits for both.
        LaunchedEffect(incomingMailtoDraft, initialAccountsLoaded, appSignatureLoaded) {
            if (!initialAccountsLoaded || !appSignatureLoaded) return@LaunchedEffect
            incomingMailtoDraft?.let { draft ->
                openMailtoCompose(draft) {
                    onMailtoDraftConsumed()
                    status = trs("mobile.status.composeDraftLoaded")
                }
            }
        }

        LaunchedEffect(incomingOAuthCallbackUrl) {
            incomingOAuthCallbackUrl?.let { rawUrl ->
                handleOAuthCallback(rawUrl)
                onOAuthCallbackConsumed()
            }
        }

        LaunchedEffect(incomingNotificationThreadTarget) {
            incomingNotificationThreadTarget?.let { target ->
                openNotificationThread(target)
                onNotificationThreadTargetConsumed()
            }
        }

        // Load persisted accounts once on startup so they survive app restarts.
        LaunchedEffect(Unit) {
            if (coreLoaded && coreAccounts.isEmpty()) {
                listAccounts()
            } else if (!coreLoaded) {
                initialAccountsLoaded = true
            }
            // The proxy lives in the core store, so it needs a read of its own.
            loadAppProxy()
            loadAppSignature()
            loadRemoteImageSenders()
            loadAlertDefaults()
            loadDetectOneTimeCodes()
        }

        // Once accounts are known, surface whatever the local store already holds so
        // a cold start shows the cached inbox instead of an empty "Nothing here yet".
        // A server sync still happens on pull-to-refresh / "Sync now".
        LaunchedEffect(coreAccounts) {
            if (coreAccounts.isNotEmpty() && coreThreads.isEmpty() && activeMailboxLoadKey == null) {
                syncCoreThreads(syncFirst = false)
            }
        }

        LaunchedEffect(coreAccounts, activeKanbanBoardId) {
            if (coreAccounts.isNotEmpty() && screen == Screen.Kanban) {
                loadKanbanBoard(refresh = false)
            }
        }
    }
}
