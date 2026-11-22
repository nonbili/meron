package jp.nonbili.meron.ui

import androidx.compose.ui.graphics.Color
import jp.nonbili.meron.shared.CalendarItem
import jp.nonbili.meron.shared.TaskListSummary
import jp.nonbili.meron.shared.requireCoreOk
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonArray
import kotlin.uuid.ExperimentalUuidApi
import kotlin.uuid.Uuid

/*
 * The services calendars, address books and task lists sync with — CalDAV,
 * CardDAV and Google (Calendar, Contacts, Tasks) — as desktop uses them
 * (states/caldav.ts, GoogleTasksAccounts.tsx, AddressBooks.tsx). The core does
 * the syncing; this asks it, keeps what it answers for the screens, and runs
 * the Google sign-ins and the foreground poll desktop's sidecar runs there.
 */

// ---- What the core reports ---------------------------------------------------

/** One calendar of a CalDAV account, as `caldav.accounts` lists it. */
internal data class DavCalendar(
    val id: String,
    val name: String,
    val color: String,
    val readOnly: Boolean,
    val enabled: Boolean,
    /** Holds events (VEVENT); only these are shown in the calendar. */
    val events: Boolean,
    /** Holds to-dos (VTODO). */
    val tasks: Boolean,
    val tasksEnabled: Boolean,
)

/** A CalDAV account (see meron-core/src/caldav/service.rs). */
internal data class DavAccount(
    val id: String,
    val serverUrl: String,
    val username: String,
    val google: Boolean,
    /** What the account is called everywhere. */
    val name: String,
    /** Its mailbox's picture, for a Google account. */
    val avatarUrl: String,
    /** Epoch seconds of the last sync that finished cleanly; 0 for never. */
    val lastSyncAt: Long,
    /** Why the last sync failed; empty when it did not. */
    val lastError: String,
    val discoveryWarning: String,
    /** Minutes between automatic syncs; 0 for only when asked. */
    val syncInterval: Int,
    val calendars: List<DavCalendar>,
)

/** A synced calendar events can come from, as `caldav.calendars` lists it. */
internal data class SyncedCalendar(
    val id: String,
    val name: String,
    val color: String,
    val writable: Boolean,
    val accountId: String,
    val accountName: String,
)

internal data class GoogleTasksAccount(
    val email: String,
    val lastError: String,
    val lastSyncAt: Long,
)

/** A CardDAV (or Google Contacts) account and its address books. */
internal data class AddressBookAccount(
    val id: String,
    val username: String,
    val server: String,
    val google: Boolean,
    val books: List<AddressBook>,
)

internal data class AddressBook(
    val id: String,
    val name: String,
    val enabled: Boolean,
    /** vCards the server holds that could not be read. */
    val skipped: Int = 0,
    /** The login of the account it is in. */
    val account: String = "",
)

/** A contact saved in a synced address book (see `carddav.cardsFor`). */
internal data class BookCard(
    val bookId: String,
    val bookName: String,
    val id: String,
    val version: String,
    val name: String,
    val emails: List<String>,
    /** The address book account's login. */
    val account: String,
    /** The vCard NOTE, shared with other clients. */
    val note: String,
)

/** A card as an address book's list gives it (see `carddav.contacts`). */
internal data class ListedCard(
    val id: String,
    val name: String,
    val emails: List<String>,
)

/** "Work · cora@example.test": which server account a book is on. */
internal fun bookLabel(
    name: String,
    account: String,
): String = if (account.isBlank()) name else "$name · $account"

private fun JsonObject.long(key: String): Long = (get(key) as? JsonPrimitive)?.longOrNull ?: 0

private fun JsonObject.objects(key: String): List<JsonObject> = (get(key) as? JsonArray).orEmpty().mapNotNull { it as? JsonObject }

private fun JsonObject.strings(key: String): List<String> = (get(key) as? JsonArray).orEmpty().mapNotNull { (it as? JsonPrimitive)?.contentOrNull }

internal fun parseDavAccounts(result: JsonObject): List<DavAccount> =
    result.objects("accounts").map { account ->
        DavAccount(
            id = account.text("id"),
            serverUrl = account.text("server_url"),
            username = account.text("username"),
            google = account.flag("google"),
            name = account.text("name").ifBlank { account.text("display_name") },
            avatarUrl = account.text("avatar_url"),
            lastSyncAt = account.long("last_sync_at"),
            lastError = account.text("last_error"),
            discoveryWarning = account.text("discovery_warning"),
            syncInterval = account.long("sync_interval").toInt(),
            calendars =
                account.objects("calendars").map { calendar ->
                    DavCalendar(
                        id = calendar.text("id"),
                        name = calendar.text("name"),
                        color = calendar.text("color"),
                        readOnly = calendar.flag("read_only"),
                        enabled = calendar.flag("enabled"),
                        events = calendar.flag("events"),
                        tasks = calendar.flag("tasks"),
                        tasksEnabled = calendar.flag("tasks_enabled"),
                    )
                },
        )
    }

internal fun parseSyncedCalendars(result: JsonObject): List<SyncedCalendar> =
    result.objects("calendars").map {
        SyncedCalendar(
            id = it.text("id"),
            name = it.text("name"),
            color = it.text("color"),
            writable = it.flag("writable"),
            accountId = it.text("account_id"),
            accountName = it.text("account_name"),
        )
    }

internal fun parseGoogleTasksAccounts(result: JsonObject): List<GoogleTasksAccount> = result.objects("accounts").map { GoogleTasksAccount(it.text("email"), it.text("last_error"), it.long("last_sync_at")) }

internal fun parseAddressBookAccounts(result: JsonObject): List<AddressBookAccount> =
    result.objects("accounts").map { account ->
        AddressBookAccount(
            id = account.text("id"),
            username = account.text("username"),
            server = account.text("server"),
            google = account.flag("google"),
            books =
                account.objects("books").map {
                    AddressBook(
                        id = it.text("id"),
                        name = it.text("name"),
                        enabled = it.flag("enabled"),
                        skipped = it.long("skipped").toInt(),
                        account = account.text("username"),
                    )
                },
        )
    }

internal fun parseBookCards(result: JsonObject): List<BookCard> =
    result.objects("cards").map {
        BookCard(
            bookId = it.text("book_id"),
            bookName = it.text("book_name"),
            id = it.text("id"),
            version = it.text("version"),
            name = it.text("name"),
            emails = it.strings("emails"),
            account = it.text("account"),
            note = it.text("note"),
        )
    }

internal fun parseListedCards(result: JsonObject): List<ListedCard> = result.objects("contacts").map { ListedCard(it.text("id"), it.text("name"), it.strings("emails")) }

// ---- Calendars as the views draw them ---------------------------------------

/** How [MeronMobileState.hiddenCalendars] names the events that live on this
 *  device: local ones and accepted invitations. */
internal const val LOCAL_CALENDAR = "local"

/** The choices the sync interval offers, in minutes; 0 is manual. */
internal val CALDAV_SYNC_INTERVALS = listOf(5, 15, 30, 60, 0)

/** The key an item is shown or hidden by. Tasks have none: they follow the
 *  Tasks setting, not a calendar. */
internal fun calendarKey(item: CalendarItem): String? = if (item.source == "task") null else item.calendar.ifBlank { LOCAL_CALENDAR }

/** [items] without those of hidden calendars. */
internal fun visibleCalendarItems(
    items: List<CalendarItem>,
    hidden: Set<String>,
): List<CalendarItem> =
    if (hidden.isEmpty()) {
        items
    } else {
        items.filter { item -> calendarKey(item)?.let { it !in hidden } ?: true }
    }

/** The colour a synced calendar's events are drawn in: its own, or for a
 *  calendar without one a preset picked by its id, so it stays the same from
 *  one session to the next and differs from its neighbours'. The same pick as
 *  desktop's `calendarColor`. */
internal fun calendarColorHex(
    id: String,
    color: String,
): String {
    val own = color.trim().lowercase()
    if (Regex("^#[0-9a-f]{6}$").matches(own)) return own
    var hash = 0
    for (ch in id) hash = hash * 31 + ch.code
    val index = (kotlin.math.abs(hash.toLong()) % ACCOUNT_COLORS.size).toInt()
    return ACCOUNT_COLORS[index].second
}

internal fun calendarColor(
    id: String,
    color: String,
): Color = accountColorOrNull(calendarColorHex(id, color)) ?: Color.Gray

// ---- Task lists -------------------------------------------------------------

internal data class TaskListGroup(
    val key: String,
    /** Empty for the lists kept on this device. */
    val label: String,
    val lists: List<TaskListSummary>,
)

private fun serverHost(url: String): String = url.substringAfter("://").substringBefore('/').ifBlank { url }

/** Lists kept on this device (unlabelled) first, then one group per synced
 *  account, labelled by its address and service, in the order lists come.
 *  Two CalDAV accounts with one username also name their servers. Mirrors
 *  desktop's `taskListGroups`. */
internal fun taskListGroups(lists: List<TaskListSummary>): List<TaskListGroup> {
    val local = lists.filter { it.provider.isBlank() }
    val synced = LinkedHashMap<String, MutableList<TaskListSummary>>()
    for (list in lists.filter { it.provider.isNotBlank() }) {
        synced.getOrPut("${list.provider}:${list.accountId.ifBlank { list.account }}") { mutableListOf() } += list
    }
    val labelled =
        synced.map { (key, members) ->
            val first = members.first()
            val service = if (first.provider == "google") "Google Tasks" else "CalDAV"
            Triple(key, if (first.account.isBlank()) service else "${first.account} · $service", members)
        }
    val labels = labelled.map { it.second }
    val groups =
        labelled.map { (key, label, members) ->
            val server = members.first().server
            val shared = labels.count { it == label } > 1
            TaskListGroup(key, if (shared && server.isNotBlank()) "$label (${serverHost(server)})" else label, members)
        }
    return (if (local.isEmpty()) emptyList() else listOf(TaskListGroup("", "", local))) + groups
}

// ---- Asking the core ----------------------------------------------------------

/** One core call off the main thread: its result object, or the core's
 *  reason as the exception. */
internal suspend fun MeronMobileState.coreCall(
    method: String,
    params: JsonObject = JsonObject(emptyMap()),
): JsonObject =
    withContext(ioDispatcher) {
        val response = Json.parseToJsonElement(requireCoreOk(core.invoke(method, params.toString()))).jsonObject
        (response["result"] as? JsonObject) ?: response
    }

/** Re-read the CalDAV accounts. Quiet on failure: the page keeps what it had. */
internal suspend fun MeronMobileState.loadCalendarAccounts() {
    if (!coreLoaded) return
    runCatching { coreCall("caldav.accounts") }
        .onSuccess { calendarAccounts = parseDavAccounts(it) }
        .onFailure { if (it is CancellationException) throw it }
}

/** Re-read which synced calendars there are, for colours, the filter and the
 *  editor's picker. Quiet on failure: the calendar shows without them. */
internal suspend fun MeronMobileState.loadSyncedCalendars() {
    if (!coreLoaded) return
    runCatching { coreCall("caldav.calendars") }
        .onSuccess { syncedCalendars = parseSyncedCalendars(it) }
        .onFailure { if (it is CancellationException) throw it }
}

internal suspend fun MeronMobileState.loadGoogleTasksAccounts() {
    if (!coreLoaded) return
    runCatching { coreCall("googleTasks.accounts") }
        .onSuccess { googleTasksAccounts = parseGoogleTasksAccounts(it) }
        .onFailure { if (it is CancellationException) throw it }
}

/** Sync one CalDAV account now, or every one. A failure is recorded on the
 *  account, where its card shows it, so it is not reported again here. */
internal suspend fun MeronMobileState.syncCalendarAccount(accountId: String? = null) {
    runCatching {
        coreCall(
            "caldav.sync",
            buildJsonObject { if (accountId != null) put("account_id", accountId) },
        )
    }.onFailure { if (it is CancellationException) throw it }
    loadCalendarAccounts()
}

/** Sync every calendar account and, with Tasks on, Google Tasks: the
 *  calendar's sync action. */
internal suspend fun MeronMobileState.syncAllCalendars() {
    if (calendarSyncing) return
    calendarSyncing = true
    try {
        syncCalendarAccount(null)
        // Not by the cached accounts, which may not be loaded yet: the core
        // does nothing without any.
        if (tasksEnabled) {
            runCatching { coreCall("googleTasks.sync") }.onFailure { if (it is CancellationException) throw it }
            loadGoogleTasksAccounts()
        }
    } finally {
        calendarSyncing = false
    }
    if (screen == Screen.Calendar) loadCalendar()
}

/** What a change that drops a calendar's (or list's) items from this device
 *  answers when some of them were changed here and have not reached the server:
 *  how many, so the user can be asked before they are thrown away. */
internal fun unsentCount(result: JsonObject): Int = result.long("unsent").toInt()

// ---- Google sign-in for Calendar, Contacts and Tasks --------------------------

/** A Google service that signs in with the account's shared grant. */
internal enum class GoogleService(
    /** The core's OAuth provider for its sign-in. */
    val provider: String,
    /** The core method that connects it, with or without fresh tokens. */
    val method: String,
) {
    Calendar("google_calendar", "caldav.addGoogle"),
    Contacts("google_contacts", "carddav.addGoogle"),
    Tasks("google_tasks", "googleTasks.add"),
    ;

    companion object {
        fun ofProvider(provider: String): GoogleService? = entries.firstOrNull { it.provider == provider }
    }
}

/** A Google service sign-in under way: which, and for which address. */
internal data class GoogleServiceSignIn(
    val service: GoogleService,
    val email: String,
)

/** Whether this build can sign in to Google at all. */
internal val MeronMobileState.googleSignInConfigured: Boolean
    get() = mobileHost.googleClientId.isNotBlank() && mobileHost.googleRedirectUri.isNotBlank()

/** Whether an account is a Google mailbox, whose services sign in with Google. */
internal fun isGoogleMailbox(account: jp.nonbili.meron.shared.AccountSummary): Boolean = account.provider == "gmail" || account.authType == "gmail_oauth" || account.imapHost.equals("imap.gmail.com", ignoreCase = true)

/** Tell the core the app's own OAuth clients, which Google grants refresh
 *  with. Called once the core is up, before anything syncs. */
internal suspend fun MeronMobileState.configureCoreOAuth() {
    if (!coreLoaded) return
    runCatching {
        coreCall(
            "oauth.configure",
            buildJsonObject {
                put("google_client_id", mobileHost.googleClientId)
                put("google_token_url", mobileHost.googleTokenUrl)
                put("outlook_client_id", mobileHost.outlookClientId)
            },
        )
    }.onFailure { if (it is CancellationException) throw it }
}

/**
 * Connect a Google service for [email], or reconnect the account [accountId].
 * A grant the account already holds may cover it: then it connects at once.
 * Otherwise the core names every scope the sign-in must cover — those of the
 * other services sharing the grant included — and the browser is opened; the
 * redirect lands in [finishGoogleServiceSignIn]. Mirrors desktop's
 * `connectGoogleService`.
 */
@OptIn(ExperimentalUuidApi::class)
internal suspend fun MeronMobileState.connectGoogleService(
    service: GoogleService,
    email: String,
    accountId: String = "",
): Boolean {
    val address = email.trim().lowercase()
    if (!googleSignInConfigured || '@' !in address) return false
    // A sign-in cancelled, or replaced by another, while a call is out must
    // not go on to open the browser.
    val attempt = ++googleSignInAttempt
    googleSignIn = GoogleServiceSignIn(service, address)
    googleSignInError = ""
    val answer =
        runCatching {
            coreCall(
                service.method,
                buildJsonObject {
                    put("email", address)
                    if (accountId.isNotBlank()) put("account_id", accountId)
                },
            )
        }.onFailure {
            // The screen that started it went away: so does its sign-in.
            if (it is CancellationException) {
                if (attempt == googleSignInAttempt) googleSignIn = null
                throw it
            }
        }.getOrElse {
            if (attempt != googleSignInAttempt) return false
            googleSignIn = null
            googleSignInError = it.message ?: trs("settings.calendars.addFailed")
            return false
        }
    if (!answer.flag("needs_sign_in")) {
        // Connected with the grant the account holds: show it, cancelled or not.
        if (attempt == googleSignInAttempt) googleSignIn = null
        googleServiceConnected(service, answer)
        return true
    }
    if (attempt != googleSignInAttempt) return false
    val redirectUri = mobileHost.googleRedirectUri.trim()
    val state = Uuid.random().toString()
    val verifier = Uuid.random().toString() + Uuid.random().toString()
    savePendingOAuthFlow(
        prefs,
        PendingOAuthFlow(
            provider = service.provider,
            state = state,
            verifier = verifier,
            redirectUri = redirectUri,
            email = address,
            serviceAccountId = accountId,
        ),
    )
    val url =
        runCatching {
            coreCall(
                "oauth.authorizeUrl",
                buildJsonObject {
                    put("provider", service.provider)
                    put("client_id", mobileHost.googleClientId)
                    put("redirect_uri", redirectUri)
                    put("state", state)
                    put("code_challenge", pkceChallenge(verifier))
                    put("login_hint", address)
                    putJsonArray("scopes") { answer.strings("scopes").forEach { add(JsonPrimitive(it)) } }
                },
            ).text("url")
        }.onFailure {
            // The screen that started it went away: so does its sign-in.
            if (it is CancellationException) {
                if (attempt == googleSignInAttempt) googleSignIn = null
                throw it
            }
        }.getOrElse {
            if (attempt != googleSignInAttempt) return false
            clearPendingOAuthFlow(prefs)
            googleSignIn = null
            googleSignInError = it.message ?: trs("settings.calendars.addFailed")
            return false
        }
    if (attempt != googleSignInAttempt) return false
    services.openOAuthUrl(
        url = url,
        callbackScheme = redirectUri.substringBefore(':', missingDelimiterValue = ""),
        onCallback = ::handleOAuthCallback,
        onFailure = { message ->
            googleSignIn = null
            googleSignInError = withReason(trs("mobile.android.oauthBrowserLaunchFailed"), message) { key, args -> trs(key, args) }
        },
    )
    return true
}

/** Stop waiting for a Google sign-in left in the browser; a late redirect for
 *  it is ignored. */
internal fun MeronMobileState.cancelGoogleServiceSignIn() {
    googleSignInAttempt++
    clearPendingOAuthFlow(prefs)
    googleSignIn = null
}

/** The redirect of a Google service sign-in: swap the code for tokens, then
 *  hand them to the service, which keeps the grant once it has connected. */
internal fun MeronMobileState.finishGoogleServiceSignIn(
    pending: PendingOAuthFlow,
    code: String,
) {
    val service = GoogleService.ofProvider(pending.provider) ?: return
    clearPendingOAuthFlow(prefs)
    // Cancel still applies while the code is being swapped.
    val attempt = ++googleSignInAttempt
    googleSignIn = GoogleServiceSignIn(service, pending.email)
    googleSignInError = ""
    scope.launch {
        // A callback can land before startup has told the core the app's
        // OAuth client (a cold start back from the browser): tell it first.
        configureCoreOAuth()
        runCatching {
            val tokens =
                coreCall(
                    "oauth.exchangeCode",
                    buildJsonObject {
                        put("provider", service.provider)
                        put("code", code)
                        put("code_verifier", pending.verifier)
                        put("redirect_uri", pending.redirectUri)
                    },
                )
            // The grant is the account's: another Google account signed in
            // in the browser would hand this one someone else's calendars.
            if (pending.email.isNotBlank() && !tokens.text("email").equals(pending.email, ignoreCase = true)) {
                error(trs("accounts.oauth.wrongGoogleAccount"))
            }
            if (attempt != googleSignInAttempt) throw CancellationException("Google sign-in cancelled")
            coreCall(
                service.method,
                buildJsonObject {
                    if (pending.serviceAccountId.isNotBlank()) put("account_id", pending.serviceAccountId)
                    put("email", tokens.text("email"))
                    put("access_token", tokens.text("access_token"))
                    put("refresh_token", tokens.text("refresh_token"))
                    put("expires_in", tokens.long("expires_in"))
                    put("scope", tokens.text("scope"))
                },
            )
        }.onSuccess { answer ->
            if (attempt == googleSignInAttempt) googleSignIn = null
            googleServiceConnected(service, answer)
        }.onFailure {
            if (attempt != googleSignInAttempt) return@onFailure
            if (it is CancellationException) throw it
            googleSignIn = null
            googleSignInError = it.message ?: trs("settings.calendars.addFailed")
        }
    }
}

/** After a Google service connected: show it, and fetch what it holds. */
private suspend fun MeronMobileState.googleServiceConnected(
    service: GoogleService,
    answer: JsonObject,
) {
    when (service) {
        GoogleService.Calendar -> {
            loadCalendarAccounts()
            val accountId = answer.text("account_id")
            // A reconnect renews the grant Google Tasks shares: retry its sync too.
            if (tasksEnabled) {
                googleTasksAccounts.firstOrNull { account -> calendarAccounts.any { it.id == accountId && it.username.equals(account.email, true) } }?.let {
                    runCatching { coreCall("googleTasks.sync", buildJsonObject { put("email", it.email) }) }
                }
            }
            if (accountId.isNotBlank()) {
                newCalendarAccountId = accountId
                syncCalendarAccount(accountId)
            }
            loadSyncedCalendars()
        }

        GoogleService.Contacts -> {
            addressBooksVersion++
            answer.text("sync_error").takeIf { it.isNotBlank() }?.let { googleSignInError = it }
        }

        GoogleService.Tasks -> {
            answer["accounts"]?.let { googleTasksAccounts = parseGoogleTasksAccounts(answer) }
        }
    }
}

// ---- Keeping in step while the app is open ----------------------------------

/**
 * One round of the foreground poll, what desktop's sidecar polls run every
 * minute: each CalDAV account at its own interval, and Google Tasks at its
 * calendar account's interval, at once when edits wait to go out. [lastRun]
 * remembers when this poll last synced each, so a failing account is not
 * retried every round.
 */
internal suspend fun MeronMobileState.pollSyncServices(
    lastRun: MutableMap<String, Long>,
    nowMillis: Long = currentTimeMillis(),
) {
    if (!coreLoaded) return
    val accounts = runCatching { parseDavAccounts(coreCall("caldav.accounts")) }.getOrElse { if (it is CancellationException) throw it else emptyList() }
    for (account in accounts) {
        if (account.syncInterval <= 0) continue
        val key = "caldav:${account.id}"
        val last = lastRun[key]
        if (last != null && nowMillis - last < account.syncInterval * 60_000L) continue
        lastRun[key] = nowMillis
        runCatching {
            coreCall(
                "caldav.sync",
                buildJsonObject {
                    put("account_id", account.id)
                    put("automatic", true)
                },
            )
        }.onFailure { if (it is CancellationException) throw it }
    }
    val tasks = runCatching { coreCall("googleTasks.pollState") }.getOrElse { if (it is CancellationException) throw it else return }
    if (!tasks.flag("enabled")) return
    val pending = tasks.strings("pending").toSet()
    for (account in tasks.objects("accounts")) {
        val email = account.text("email")
        val minutes = account.long("minutes")
        val key = "googleTasks:$email"
        val last = lastRun[key]
        val due = email in pending || (minutes > 0 && (last == null || nowMillis - last >= minutes * 60_000L))
        if (!due) continue
        lastRun[key] = nowMillis
        runCatching {
            coreCall(
                "googleTasks.sync",
                buildJsonObject {
                    put("email", email)
                    put("automatic", true)
                },
            )
        }.onFailure { if (it is CancellationException) throw it }
    }
}

/** Keep the core's Tasks switch, which pauses task-list sync, in step with
 *  the app's. Turning Tasks on catches up at once, as desktop does. */
internal suspend fun MeronMobileState.mirrorTasksEnabled() {
    if (!coreLoaded) return
    runCatching {
        coreCall(
            "app.prefsSet",
            buildJsonObject {
                put("key", "tasks_enabled")
                put("value", tasksEnabled)
            },
        )
    }.onFailure { if (it is CancellationException) throw it }
}

internal fun JsonObject.boolOrNull(key: String): Boolean? = (get(key) as? JsonPrimitive)?.booleanOrNull
