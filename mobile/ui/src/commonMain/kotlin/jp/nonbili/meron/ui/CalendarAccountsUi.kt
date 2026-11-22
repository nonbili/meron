package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Checklist
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Event
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.Key
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.accountSummaryIsRss
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/*
 * Settings › Calendars & tasks: whether the calendar and Tasks are on, the
 * alert defaults, and the accounts they sync with — CalDAV servers and Google
 * (Calendar, plus Google Tasks on its card). Mirrors desktop's
 * CalendarsTasksSection and CalendarAccountsPanel.
 */

/** A question to confirm before going ahead; [onConfirm] runs on yes. */
private data class Confirm(
    val title: String,
    val message: String,
    val confirmLabel: String,
    val onConfirm: () -> Unit,
)

/** What a dialog on the page is open for. */
private sealed interface CalendarDialog {
    /** Add a CalDAV account, with the server and login discovery found. */
    data class AddCalDav(
        val server: String = "",
        val username: String = "",
        val discovered: Boolean? = null,
    ) : CalendarDialog

    data object GoogleEmail : CalendarDialog

    data object FindForMailbox : CalendarDialog

    data class Password(
        val account: DavAccount,
    ) : CalendarDialog

    data class Rename(
        val account: DavAccount,
    ) : CalendarDialog

    data class EditCalendar(
        val calendar: DavCalendar,
    ) : CalendarDialog

    data class NewCalendar(
        val account: DavAccount,
    ) : CalendarDialog

    data class Color(
        val calendar: DavCalendar,
    ) : CalendarDialog
}

/** Mail accounts that could have calendars (or address books) on their
 *  server: not feeds, and Google ones only when Google sign-in is set up. */
internal fun MeronMobileState.davMailboxes(): List<AccountSummary> =
    coreAccounts.filter { account ->
        account.email.isNotBlank() && !accountSummaryIsRss(account) && account.authType != "local" &&
            (!isGoogleMailbox(account) || googleSignInConfigured)
    }

/** A password mail account on [login], whose password (kept in the core)
 *  usually opens the same server's calendars and address books too. */
internal fun MeronMobileState.mailAccountForLogin(login: String): AccountSummary? {
    val wanted = login.trim().lowercase()
    if (wanted.isEmpty()) return null
    return coreAccounts.firstOrNull { account ->
        account.authType == "password" && !account.needsReconnect &&
            account.username
                .ifBlank { account.email }
                .trim()
                .lowercase() == wanted
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun CalendarsTasksPage(
    state: MeronMobileState,
    modifier: Modifier = Modifier,
) {
    with(state) {
        val scope = rememberCoroutineScope()
        var dialog by remember { mutableStateOf<CalendarDialog?>(null) }
        var confirm by remember { mutableStateOf<Confirm?>(null) }
        var error by remember { mutableStateOf("") }
        var discovering by remember { mutableStateOf(false) }

        LaunchedEffect(Unit) {
            loadCalendarAccounts()
            loadGoogleTasksAccounts()
        }

        /** Run a change, showing why it failed on the page. */
        fun act(work: suspend () -> Unit) {
            error = ""
            scope.launch {
                try {
                    work()
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    error = e.message ?: trs("settings.calendars.saveFailed")
                    // A switch shown changed at once goes back to the truth.
                    loadCalendarAccounts()
                }
            }
        }

        /** Run a change that drops items from this device. When some of them
         *  were changed here and cannot reach the server yet, ask before
         *  throwing those changes away. */
        fun dropping(
            method: String,
            params: JsonObject,
        ) = act {
            val answer = coreCall(method, params)
            val unsent = unsentCount(answer)
            if (unsent > 0) {
                confirm =
                    Confirm(
                        title = trs("settings.calendars.unsentTitle"),
                        message = trs("settings.calendars.unsentConfirm", mapOf("count" to unsent)),
                        confirmLabel = trs("settings.calendars.discardUnsent"),
                    ) {
                        act {
                            coreCall(method, JsonObject(params + ("discard_unsent" to kotlinx.serialization.json.JsonPrimitive(true))))
                            afterCalendarChange()
                        }
                    }
                loadCalendarAccounts()
                loadGoogleTasksAccounts()
                return@act
            }
            afterCalendarChange()
        }

        fun setCalendarEnabled(
            calendar: DavCalendar,
            enabled: Boolean,
        ) {
            // Show the switch's new state at once; the answer puts the truth back.
            calendarAccounts =
                calendarAccounts.map { account ->
                    account.copy(calendars = account.calendars.map { if (it.id == calendar.id) it.copy(enabled = enabled) else it })
                }
            if (!enabled) {
                dropping(
                    "caldav.setCalendarEnabled",
                    buildJsonObject {
                        put("calendar_id", calendar.id)
                        put("enabled", false)
                    },
                )
                return
            }
            act {
                coreCall(
                    "caldav.setCalendarEnabled",
                    buildJsonObject {
                        put("calendar_id", calendar.id)
                        put("enabled", true)
                    },
                )
                afterCalendarChange()
                // A calendar just turned on has no events here yet: fetch them
                // now rather than at the next scheduled sync.
                calendarAccounts.firstOrNull { account -> account.calendars.any { it.id == calendar.id } }?.let {
                    syncCalendarAccount(it.id)
                    afterCalendarChange()
                }
            }
        }

        fun findForMailbox(mailbox: AccountSummary) {
            val google = calendarAccounts.firstOrNull { it.google && it.username.equals(mailbox.email.trim(), ignoreCase = true) }
            if (google != null) {
                newCalendarAccountId = google.id
                return
            }
            if (isGoogleMailbox(mailbox)) {
                scope.launch { connectGoogleService(GoogleService.Calendar, mailbox.email) }
                return
            }
            discovering = true
            scope.launch {
                val server =
                    runCatching { coreCall("caldav.discoverServer", buildJsonObject { put("email", mailbox.email) }).text("server_url") }
                        .getOrElse { if (it is CancellationException) throw it else "" }
                discovering = false
                dialog = CalendarDialog.AddCalDav(server = server, username = mailbox.email, discovered = server.isNotBlank())
            }
        }

        val mailboxes = davMailboxes()
        val busy = discovering || googleSignIn != null
        val listState = rememberLazyListState()
        SettingsList(modifier.appScrollbar(listState), state = listState) {
            item {
                SettingsToggleRow(
                    icon = Icons.Filled.Event,
                    title = tr("settings.calendar.enable"),
                    hint = tr("settings.calendar.enableHint"),
                    checked = calendarEnabled,
                    onToggle = { setCalendarEnabled(!calendarEnabled) },
                )
            }
            item {
                SettingsToggleRow(
                    icon = Icons.Filled.Checklist,
                    title = tr("settings.tasks.enable"),
                    hint = tr("settings.tasks.enableHint"),
                    checked = tasksEnabled,
                    onToggle = { setTasksEnabled(!tasksEnabled) },
                )
            }
            if (calendarEnabled) {
                item {
                    AlertDefaultRow(
                        title = tr("settings.calendar.reminderLead"),
                        hint = tr("settings.calendar.reminderLeadHint"),
                        allDay = false,
                        lead = reminderLeadMinutes,
                        onChange = { saveAlertDefault(false, it) },
                    )
                }
                item {
                    AlertDefaultRow(
                        title = tr("settings.calendar.allDayAlert"),
                        hint = tr("settings.calendar.allDayAlertHint"),
                        allDay = true,
                        lead = reminderAllDayDays,
                        onChange = { saveAlertDefault(true, it) },
                    )
                }
            }
            if (!calendarEnabled && !tasksEnabled) return@SettingsList

            section(tr("settings.calendars.accounts"))
            plain {
                Text(
                    tr("settings.calendars.intro"),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 20.dp, end = 20.dp, bottom = 8.dp),
                )
            }
            googleSignIn?.let { signIn ->
                plain {
                    SignInWaiting(onCancel = ::cancelGoogleServiceSignIn)
                }
            }
            val failure = error.ifBlank { googleSignInError }
            if (failure.isNotBlank()) {
                plain {
                    Text(
                        failure,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(horizontal = 20.dp, vertical = 4.dp),
                    )
                }
            }
            if (mailboxes.isNotEmpty()) {
                item {
                    SettingsRow(
                        icon = Icons.Filled.Search,
                        title = tr("settings.calendars.findForMailbox"),
                        trailing = { if (discovering) CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp) },
                        onClick = {
                            if (busy) return@SettingsRow
                            if (mailboxes.size == 1) findForMailbox(mailboxes.single()) else dialog = CalendarDialog.FindForMailbox
                        },
                    )
                }
            }
            if (googleSignInConfigured) {
                item {
                    SettingsRow(
                        icon = Icons.Filled.Key,
                        title = tr("accounts.oauth.signInWith", mapOf("provider" to "Google")),
                        onClick = { if (!busy) dialog = CalendarDialog.GoogleEmail },
                    )
                }
            }
            item {
                SettingsRow(
                    icon = Icons.Filled.Add,
                    title = tr("settings.calendars.add"),
                    onClick = { if (!busy) dialog = CalendarDialog.AddCalDav() },
                )
            }
            if (calendarAccounts.isEmpty()) {
                plain { SettingsEmptyLabel(tr("settings.calendars.none")) }
            }
            calendarAccounts.forEach { account ->
                plain { Spacer(Modifier.size(8.dp)) }
                item(key = "caldav:${account.id}") {
                    CalendarAccountCard(
                        state = state,
                        account = account,
                        busy = busy,
                        onDialog = { dialog = it },
                        onConfirm = { confirm = it },
                        onToggleCalendar = ::setCalendarEnabled,
                        onToggleTasks = { calendar, enabled ->
                            val params =
                                buildJsonObject {
                                    put("calendar_id", calendar.id)
                                    put("enabled", enabled)
                                }
                            if (!enabled) {
                                dropping("caldav.setTasksEnabled", params)
                            } else {
                                // A list just turned on has no to-dos here yet:
                                // fetch them now, not at the next scheduled sync.
                                act {
                                    coreCall("caldav.setTasksEnabled", params)
                                    afterCalendarChange()
                                    syncCalendarAccount(account.id)
                                    afterCalendarChange()
                                }
                            }
                        },
                        onRemove = {
                            confirm =
                                Confirm(
                                    title = trs("settings.calendars.remove"),
                                    message = trs("settings.calendars.removeConfirm", mapOf("name" to account.name)),
                                    confirmLabel = trs("settings.calendars.remove"),
                                ) { dropping("caldav.remove", buildJsonObject { put("account_id", account.id) }) }
                        },
                        onAct = ::act,
                    )
                }
            }
            // Google Tasks accounts whose Google calendar account was removed:
            // still shown so they can be synced or removed.
            val orphans =
                if (tasksEnabled) {
                    googleTasksAccounts.filter { tasks -> calendarAccounts.none { it.google && it.username.equals(tasks.email, ignoreCase = true) } }
                } else {
                    emptyList()
                }
            if (orphans.isNotEmpty()) {
                section("Google Tasks")
                orphans.forEach { tasks ->
                    item(key = "googleTasks:${tasks.email}") {
                        GoogleTasksOrphanRow(state, tasks, busy, onConfirm = { confirm = it }, onAct = ::act)
                    }
                }
            }
        }

        when (val open = dialog) {
            is CalendarDialog.AddCalDav -> {
                CalDavAccountDialog(
                    state = state,
                    initial = open,
                    onDismiss = { dialog = null },
                    onAdded = { id ->
                        dialog = null
                        newCalendarAccountId = id
                    },
                )
            }

            CalendarDialog.GoogleEmail -> {
                EmailDialog(
                    title = tr("accounts.oauth.signInWith", mapOf("provider" to "Google")),
                    onDismiss = { dialog = null },
                    onSubmit = { email ->
                        dialog = null
                        scope.launch { connectGoogleService(GoogleService.Calendar, email) }
                    },
                )
            }

            CalendarDialog.FindForMailbox -> {
                MailboxPickerDialog(
                    title = tr("settings.calendars.findForMailbox"),
                    mailboxes = mailboxes,
                    onDismiss = { dialog = null },
                    onPick = {
                        dialog = null
                        findForMailbox(it)
                    },
                )
            }

            is CalendarDialog.Password -> {
                TextValueDialog(
                    title = tr("settings.calendars.updatePassword"),
                    label = tr("settings.calendars.password"),
                    initial = "",
                    password = true,
                    onDismiss = { dialog = null },
                    onSave = { password ->
                        coreCall(
                            "caldav.setPassword",
                            buildJsonObject {
                                put("account_id", open.account.id)
                                put("password", password)
                            },
                        )
                        dialog = null
                        // Not in the dialog's scope, which closing it cancels.
                        this@with.scope.launch {
                            loadCalendarAccounts()
                            syncCalendarAccount(open.account.id)
                            afterCalendarChange()
                        }
                    },
                )
            }

            is CalendarDialog.Rename -> {
                TextValueDialog(
                    title = tr("settings.calendars.accountName"),
                    label = tr("settings.calendars.accountName"),
                    hint = tr("settings.calendars.accountNameHint"),
                    initial = open.account.name,
                    allowBlank = true,
                    onDismiss = { dialog = null },
                    onSave = { name ->
                        coreCall(
                            "caldav.renameAccount",
                            buildJsonObject {
                                put("account_id", open.account.id)
                                put("display_name", name.trim())
                            },
                        )
                        dialog = null
                        afterCalendarChange()
                    },
                )
            }

            is CalendarDialog.EditCalendar -> {
                EditCalendarDialog(
                    calendar = open.calendar,
                    onDismiss = { dialog = null },
                    onRename = { name ->
                        coreCall(
                            "caldav.updateCalendar",
                            buildJsonObject {
                                put("calendar_id", open.calendar.id)
                                put("name", name)
                            },
                        )
                        dialog = null
                        afterCalendarChange()
                    },
                    onDelete = {
                        dialog = null
                        confirm =
                            Confirm(
                                title = trs("settings.calendars.deleteCalendar"),
                                message = trs("settings.calendars.deleteCalendarConfirm", mapOf("name" to open.calendar.name)),
                                confirmLabel = trs("settings.calendars.deleteCalendar"),
                            ) {
                                act {
                                    coreCall("caldav.deleteCalendar", buildJsonObject { put("calendar_id", open.calendar.id) })
                                    afterCalendarChange()
                                }
                            }
                    },
                )
            }

            is CalendarDialog.NewCalendar -> {
                NewCalendarDialog(
                    onDismiss = { dialog = null },
                    onCreate = { name, color ->
                        coreCall(
                            "caldav.createCalendar",
                            buildJsonObject {
                                put("account_id", open.account.id)
                                put("name", name)
                                put("color", color)
                            },
                        )
                        dialog = null
                        afterCalendarChange()
                    },
                )
            }

            is CalendarDialog.Color -> {
                CalendarColorDialog(
                    color = calendarColorHex(open.calendar.id, open.calendar.color),
                    onDismiss = { dialog = null },
                    onPick = { color ->
                        dialog = null
                        act {
                            val answer =
                                coreCall(
                                    "caldav.updateCalendar",
                                    buildJsonObject {
                                        put("calendar_id", open.calendar.id)
                                        put("color", color)
                                    },
                                )
                            afterCalendarChange()
                            // A colour the server will not take is kept on this device.
                            if (answer.flag("local")) snackbarHost.showSnackbar(trs("settings.calendars.colorKeptHere"))
                        }
                    },
                )
            }

            null -> {}
        }

        confirm?.let { question ->
            AlertDialog(
                onDismissRequest = { confirm = null },
                title = { Text(question.title) },
                text = { Text(question.message) },
                confirmButton = {
                    TextButton(onClick = {
                        confirm = null
                        question.onConfirm()
                    }) { Text(question.confirmLabel, color = MaterialTheme.colorScheme.error) }
                },
                dismissButton = { TextButton(onClick = { confirm = null }) { Text(tr("buttons.cancel")) } },
            )
        }
    }
}

/** After a change to calendar accounts: everything that shows them reads again. */
internal suspend fun MeronMobileState.afterCalendarChange() {
    loadCalendarAccounts()
    loadSyncedCalendars()
    loadGoogleTasksAccounts()
    if (screen == Screen.Calendar) loadCalendar()
}

@Composable
private fun SignInWaiting(onCancel: () -> Unit) {
    Surface(
        shape = RoundedCornerShape(16.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
    ) {
        Row(Modifier.padding(start = 16.dp, end = 8.dp, top = 6.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
            Text(
                tr("accounts.oauth.waitingForSignIn", mapOf("provider" to "Google")),
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.weight(1f).padding(horizontal = 12.dp),
            )
            TextButton(onClick = onCancel) { Text(tr("buttons.cancel")) }
        }
    }
}

/** One account: its calendars to show, its task lists, and its settings,
 *  folded away until opened. Opens by itself on a failure or once added. */
@Composable
private fun CalendarAccountCard(
    state: MeronMobileState,
    account: DavAccount,
    busy: Boolean,
    onDialog: (CalendarDialog) -> Unit,
    onConfirm: (Confirm) -> Unit,
    onToggleCalendar: (DavCalendar, Boolean) -> Unit,
    onToggleTasks: (DavCalendar, Boolean) -> Unit,
    onRemove: () -> Unit,
    onAct: (suspend () -> Unit) -> Unit,
) = with(state) {
    val tasksAccount =
        if (account.google && tasksEnabled) googleTasksAccounts.firstOrNull { it.email.equals(account.username, ignoreCase = true) } else null
    val tasksError = tasksAccount?.lastError.orEmpty()
    val newlyAdded = newCalendarAccountId == account.id
    var expanded by remember { mutableStateOf(account.lastError.isNotBlank() || tasksError.isNotBlank() || newlyAdded) }
    LaunchedEffect(account.lastError, tasksError, newlyAdded) {
        if (account.lastError.isNotBlank() || tasksError.isNotBlank() || newlyAdded) expanded = true
    }
    var syncing by remember { mutableStateOf(false) }
    var intervalMenu by remember { mutableStateOf(false) }
    val colors = MaterialTheme.colorScheme
    val status =
        when {
            account.lastError.isNotBlank() -> tr("settings.calendars.syncFailed", mapOf("error" to account.lastError))
            account.lastSyncAt > 0 -> tr("settings.calendars.lastSynced", mapOf("time" to formatInboxTimestamp(account.lastSyncAt)))
            else -> tr("settings.calendars.notSynced")
        }
    // Collapsed, a Google Tasks failure shows here too; open, its own row has it.
    val tasksFailure = account.lastError.isBlank() && !expanded && tasksError.isNotBlank()
    val summary = if (tasksFailure) "Google Tasks · ${tr("settings.calendars.syncFailed", mapOf("error" to tasksError))}" else status
    val eventCalendars = account.calendars.filter { it.events }
    val taskCalendars = if (tasksEnabled) account.calendars.filter { it.tasks || it.tasksEnabled } else emptyList()

    Column {
        Row(
            Modifier.fillMaxWidth().clickable { expanded = !expanded }.padding(horizontal = 16.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (account.google) {
                AccountBadgeAvatar(label = account.name, avatarUrl = account.avatarUrl, size = 36.dp)
                Spacer(Modifier.width(12.dp))
            } else {
                Icon(Icons.Filled.Event, contentDescription = null, tint = colors.onSurfaceVariant, modifier = Modifier.padding(end = 16.dp))
            }
            Column(Modifier.weight(1f)) {
                Text(account.name, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                if (account.username.isNotBlank() && account.username != account.name) {
                    Text(account.username, style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                if (!(expanded && account.lastError.isNotBlank())) {
                    Text(
                        summary,
                        style = MaterialTheme.typography.bodySmall,
                        color = if (account.lastError.isNotBlank() || tasksFailure) colors.error else colors.onSurfaceVariant,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            Icon(if (expanded) Icons.Filled.ExpandLess else Icons.Filled.ExpandMore, contentDescription = null, tint = colors.onSurfaceVariant)
        }
        if (!expanded) return@Column
        HorizontalDivider(color = colors.outlineVariant.copy(alpha = 0.5f))
        if (account.lastError.isNotBlank()) {
            Text(status, color = colors.error, style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
        }
        if (account.discoveryWarning.isNotBlank()) {
            Text(account.discoveryWarning, color = colors.onSurfaceVariant, style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
        }
        CardHeading(tr("calendar.calendars"))
        if (eventCalendars.isEmpty()) {
            CardNote(tr("settings.calendars.noEventCalendars"))
        }
        eventCalendars.forEach { calendar ->
            Row(
                Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(start = 8.dp, end = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // The dot opens the calendar's colours.
                Box(
                    Modifier.size(40.dp).clip(CircleShape).clickable { onDialog(CalendarDialog.Color(calendar)) },
                    contentAlignment = Alignment.Center,
                ) {
                    Box(
                        Modifier
                            .size(14.dp)
                            .alpha(if (calendar.enabled) 1f else 0.4f)
                            .clip(CircleShape)
                            .background(calendarColor(calendar.id, calendar.color)),
                    )
                }
                Text(
                    calendar.name,
                    color = if (calendar.enabled) colors.onSurface else colors.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                if (calendar.readOnly) ReadOnlyChip()
                if (!account.google) {
                    IconButton(onClick = { onDialog(CalendarDialog.EditCalendar(calendar)) }) {
                        Icon(Icons.Filled.Edit, contentDescription = tr("settings.calendars.edit"), tint = colors.onSurfaceVariant, modifier = Modifier.size(20.dp))
                    }
                }
                Switch(checked = calendar.enabled, onCheckedChange = { onToggleCalendar(calendar, it) }, colors = settingsSwitchColors())
            }
        }
        if (!account.google) {
            TextButton(onClick = { onDialog(CalendarDialog.NewCalendar(account)) }, modifier = Modifier.padding(start = 8.dp)) {
                Icon(Icons.Filled.Add, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text(tr("settings.calendars.newCalendar"))
            }
        }
        if (taskCalendars.isNotEmpty()) {
            HorizontalDivider(color = colors.outlineVariant.copy(alpha = 0.5f))
            CardHeading(tr("tasks.title"))
            taskCalendars.forEach { calendar ->
                CardSwitchRow(calendar.name, calendar.tasksEnabled) { onToggleTasks(calendar, !calendar.tasksEnabled) }
            }
        }
        if (account.google && tasksEnabled) {
            HorizontalDivider(color = colors.outlineVariant.copy(alpha = 0.5f))
            GoogleTasksSwitch(state, account.username, tasksAccount, busy, onConfirm, onAct)
        }
        HorizontalDivider(color = colors.outlineVariant.copy(alpha = 0.5f))
        CardValueRow(tr("settings.calendars.accountName"), account.name) { onDialog(CalendarDialog.Rename(account)) }
        Box {
            CardValueRow(
                tr("settings.calendars.syncEvery"),
                if (account.syncInterval == 0) tr("settings.calendars.syncManually") else tr("settings.calendars.minutes", mapOf("count" to account.syncInterval)),
            ) { intervalMenu = true }
            Box(Modifier.matchParentSize(), contentAlignment = Alignment.TopEnd) {
                Box(Modifier.fillMaxHeight().width(0.dp)) {
                    DropdownMenu(expanded = intervalMenu, onDismissRequest = { intervalMenu = false }) {
                        CALDAV_SYNC_INTERVALS.forEach { minutes ->
                            DropdownMenuItem(
                                text = { Text(if (minutes == 0) tr("settings.calendars.syncManually") else tr("settings.calendars.minutes", mapOf("count" to minutes))) },
                                onClick = {
                                    intervalMenu = false
                                    onAct {
                                        coreCall(
                                            "caldav.setSyncInterval",
                                            buildJsonObject {
                                                put("account_id", account.id)
                                                put("minutes", minutes)
                                            },
                                        )
                                        loadCalendarAccounts()
                                    }
                                },
                            )
                        }
                    }
                }
            }
        }
        HorizontalDivider(color = colors.outlineVariant.copy(alpha = 0.5f))
        FlowRow(
            Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.Center,
        ) {
            OutlinedButton(
                enabled = !syncing,
                onClick = {
                    syncing = true
                    onAct {
                        try {
                            syncCalendarAccount(account.id)
                            tasksAccount?.let { coreCall("googleTasks.sync", buildJsonObject { put("email", it.email) }) }
                            afterCalendarChange()
                        } finally {
                            syncing = false
                        }
                    }
                },
            ) {
                Icon(Icons.Filled.Refresh, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text(tr("settings.calendars.syncNow"))
            }
            if (account.google) {
                if (googleSignInConfigured) {
                    TextButton(enabled = !busy, onClick = { onAct { connectGoogleService(GoogleService.Calendar, account.username, account.id) } }) {
                        Text(tr("accounts.actions.reconnectAccount"))
                    }
                }
            } else {
                TextButton(onClick = { onDialog(CalendarDialog.Password(account)) }) { Text(tr("settings.calendars.updatePassword")) }
            }
            TextButton(onClick = onRemove) { Text(tr("settings.calendars.remove"), color = colors.error) }
        }
    }
}

@Composable
private fun CardHeading(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 4.dp),
    )
}

@Composable
private fun CardNote(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

@Composable
private fun CardSwitchRow(
    title: String,
    checked: Boolean,
    subtitle: String = "",
    subtitleError: Boolean = false,
    onToggle: () -> Unit,
) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .clickable(onClick = onToggle)
            .padding(start = 16.dp, end = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (subtitle.isNotBlank()) {
                Text(
                    subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = if (subtitleError) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        Switch(checked = checked, onCheckedChange = { onToggle() }, colors = settingsSwitchColors())
    }
}

@Composable
private fun CardValueRow(
    title: String,
    value: String,
    onClick: () -> Unit,
) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(title, modifier = Modifier.weight(1f))
        Text(
            value,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(start = 12.dp),
        )
    }
}

@Composable
internal fun ReadOnlyChip() {
    Text(
        tr("calendar.readOnly"),
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier =
            Modifier
                .padding(horizontal = 6.dp)
                .clip(RoundedCornerShape(6.dp))
                .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                .padding(horizontal = 6.dp, vertical = 2.dp),
    )
}

/** The Tasks part of a Google calendar account's card: Google Tasks syncs
 *  over its own API but shares the account's sign-in, so it is switched on
 *  here. It speaks up only when it fails. */
@Composable
private fun GoogleTasksSwitch(
    state: MeronMobileState,
    email: String,
    account: GoogleTasksAccount?,
    busy: Boolean,
    onConfirm: (Confirm) -> Unit,
    onAct: (suspend () -> Unit) -> Unit,
) = with(state) {
    if (account == null && !googleSignInConfigured) return@with
    CardHeading(tr("tasks.title"))
    val failure = account?.lastError.orEmpty()
    CardSwitchRow(
        title = "Google Tasks",
        checked = account != null,
        subtitle = if (failure.isNotBlank()) tr("settings.calendars.syncFailed", mapOf("error" to failure)) else "",
        subtitleError = true,
    ) {
        if (busy) return@CardSwitchRow
        if (account == null) {
            onAct { connectGoogleService(GoogleService.Tasks, email) }
            return@CardSwitchRow
        }
        onConfirm(
            Confirm(
                title = "Google Tasks",
                message = trs("settings.tasks.removeGoogleConfirm", mapOf("email" to account.email)),
                confirmLabel = trs("settings.calendars.remove"),
            ) { onAct { removeGoogleTasks(account.email, onConfirm, onAct) } },
        )
    }
    // The card's Reconnect covers the calendars: a grant that still serves
    // them but not Tasks needs Tasks to ask for itself.
    if (account != null && failure.isNotBlank() && googleSignInConfigured) {
        TextButton(enabled = !busy, onClick = { onAct { connectGoogleService(GoogleService.Tasks, account.email) } }, modifier = Modifier.padding(start = 8.dp)) {
            Text(tr("settings.account.reconnectButton"))
        }
    }
}

/** Remove a Google Tasks account, asking first when edits made here have not
 *  reached Google yet. */
private suspend fun MeronMobileState.removeGoogleTasks(
    email: String,
    onConfirm: (Confirm) -> Unit,
    onAct: (suspend () -> Unit) -> Unit,
) {
    val answer = coreCall("googleTasks.remove", buildJsonObject { put("email", email) })
    val unsent = unsentCount(answer)
    if (unsent > 0) {
        onConfirm(
            Confirm(
                title = trs("settings.calendars.unsentTitle"),
                message = trs("settings.calendars.unsentConfirm", mapOf("count" to unsent)),
                confirmLabel = trs("settings.calendars.discardUnsent"),
            ) {
                onAct {
                    coreCall(
                        "googleTasks.remove",
                        buildJsonObject {
                            put("email", email)
                            put("discard_unsent", true)
                        },
                    )
                    loadGoogleTasksAccounts()
                }
            },
        )
        return
    }
    googleTasksAccounts = parseGoogleTasksAccounts(answer)
}

@Composable
private fun GoogleTasksOrphanRow(
    state: MeronMobileState,
    account: GoogleTasksAccount,
    busy: Boolean,
    onConfirm: (Confirm) -> Unit,
    onAct: (suspend () -> Unit) -> Unit,
) = with(state) {
    val status =
        when {
            account.lastError.isNotBlank() -> tr("settings.calendars.syncFailed", mapOf("error" to account.lastError))
            account.lastSyncAt > 0 -> tr("settings.calendars.lastSynced", mapOf("time" to formatInboxTimestamp(account.lastSyncAt)))
            else -> tr("settings.calendars.notSynced")
        }
    Column(Modifier.padding(vertical = 8.dp)) {
        Text(account.email, fontWeight = FontWeight.Medium, modifier = Modifier.padding(horizontal = 16.dp))
        Text(
            status,
            style = MaterialTheme.typography.bodySmall,
            color = if (account.lastError.isNotBlank()) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 16.dp),
        )
        Row(Modifier.padding(horizontal = 8.dp)) {
            if (googleSignInConfigured && account.lastError.isNotBlank()) {
                TextButton(enabled = !busy, onClick = { onAct { connectGoogleService(GoogleService.Tasks, account.email) } }) {
                    Text(tr("settings.account.reconnectButton"))
                }
            }
            TextButton(onClick = {
                onAct {
                    googleTasksAccounts = parseGoogleTasksAccounts(coreCall("googleTasks.sync", buildJsonObject { put("email", account.email) }))
                }
            }) { Text(tr("settings.calendars.syncNow")) }
            TextButton(onClick = {
                onConfirm(
                    Confirm(
                        title = trs("settings.account.removeAccountTitle"),
                        message = trs("settings.tasks.removeGoogleConfirm", mapOf("email" to account.email)),
                        confirmLabel = trs("settings.calendars.remove"),
                    ) { onAct { removeGoogleTasks(account.email, onConfirm, onAct) } },
                )
            }) { Text(tr("settings.calendars.remove"), color = MaterialTheme.colorScheme.error) }
        }
    }
}

/** Connect a CalDAV server: discovery doubles as the password check. A blank
 *  password, with a mail account on the same login, uses that account's. */
@Composable
private fun CalDavAccountDialog(
    state: MeronMobileState,
    initial: CalendarDialog.AddCalDav,
    onDismiss: () -> Unit,
    onAdded: (String) -> Unit,
) = with(state) {
    val scope = rememberCoroutineScope()
    var server by remember { mutableStateOf(initial.server) }
    var username by remember { mutableStateOf(initial.username) }
    var password by remember { mutableStateOf("") }
    var name by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf("") }
    // Bumped when a connect is cancelled: a server that does not answer must
    // not hold the dialog, and its late answer is ignored.
    var attempt by remember { mutableStateOf(0) }
    val mailAccount = mailAccountForLogin(username)
    val canSubmit = server.isNotBlank() && username.isNotBlank() && (password.isNotEmpty() || mailAccount != null) && !busy
    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        title = { Text(tr("settings.calendars.add")) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                if (initial.discovered == false) {
                    Text(tr("settings.calendars.discoveryNoServer"), style = MaterialTheme.typography.bodySmall)
                }
                OutlinedTextField(
                    value = server,
                    onValueChange = { server = it },
                    label = { Text(tr("settings.calendars.server")) },
                    placeholder = { Text(tr("settings.calendars.serverPlaceholder")) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false),
                )
                OutlinedTextField(
                    value = username,
                    onValueChange = { username = it },
                    label = { Text(tr("settings.calendars.username")) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email, autoCorrectEnabled = false),
                )
                OutlinedTextField(
                    value = password,
                    onValueChange = { password = it },
                    label = { Text(tr(if (mailAccount != null) "settings.calendars.passwordFromMail" else "settings.calendars.password")) },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                )
                PlainHttpWarning(server = server, mailPassword = password.isEmpty() && mailAccount != null)
                Text(tr("settings.calendars.passwordHint"), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text(tr("settings.calendars.accountNameOptional")) },
                    singleLine = true,
                )
                if (error.isNotBlank()) Text(error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
            }
        },
        confirmButton = {
            Button(
                enabled = canSubmit,
                onClick = {
                    val current = ++attempt
                    busy = true
                    error = ""
                    val before = calendarAccounts.map { it.id }.toSet()
                    scope.launch {
                        runCatching {
                            coreCall(
                                "caldav.add",
                                buildJsonObject {
                                    put("server_url", server.trim())
                                    put("username", username.trim())
                                    put("password", password)
                                    if (password.isEmpty() && mailAccount != null) put("mail_account_id", mailAccount.id)
                                    put("display_name", name.trim())
                                },
                            )
                        }.onSuccess { answer ->
                            if (current != attempt) return@onSuccess
                            busy = false
                            calendarAccounts = parseDavAccounts(answer)
                            // The account connected, new or reconnected.
                            val id = answer.text("account_id").ifBlank { calendarAccounts.firstOrNull { it.id !in before }?.id.orEmpty() }
                            onAdded(id)
                            // Not in the dialog's scope, which closing it cancels.
                            this@with.scope.launch {
                                if (id.isNotBlank()) syncCalendarAccount(id)
                                afterCalendarChange()
                            }
                        }.onFailure {
                            if (it is CancellationException) throw it
                            if (current != attempt) return@onFailure
                            busy = false
                            error = it.message ?: trs("settings.calendars.addFailed")
                        }
                    }
                },
            ) { Text(tr(if (busy) "settings.calendars.connecting" else "settings.calendars.connect")) }
        },
        dismissButton = {
            // While connecting, stop and keep the form to fix it; then close.
            TextButton(onClick = {
                if (busy) {
                    attempt++
                    busy = false
                } else {
                    onDismiss()
                }
            }) { Text(tr("buttons.cancel")) }
        },
    )
}

/** Whether a typed server address is plain `http://` to another machine: a
 *  password sent there crosses the network unencrypted. This device
 *  (localhost, 127.0.0.0/8, ::1) is not a network hop. Desktop's
 *  `sendsPlainText`. */
internal fun sendsPlainText(server: String): Boolean {
    val text = server.trim()
    if (!text.lowercase().startsWith("http://")) return false
    val authority =
        text
            .substring("http://".length)
            .substringBefore('/')
            .substringBefore('?')
            .substringBefore('#')
            .substringAfterLast('@')
    val host =
        if (authority.startsWith("[")) {
            authority.substringBefore(']') + "]"
        } else {
            authority.substringBefore(':')
        }.lowercase()
    if (host.isEmpty()) return true
    return !(host == "localhost" || host.endsWith(".localhost") || host == "[::1]" || Regex("^127\\.\\d+\\.\\d+\\.\\d+$").matches(host))
}

/** Under a DAV server form: a plain `http://` address to another machine
 *  sends the password unencrypted. Shown while typing; nothing is blocked. */
@Composable
internal fun PlainHttpWarning(
    server: String,
    mailPassword: Boolean,
) {
    if (!sendsPlainText(server)) return
    Text(
        tr(if (mailPassword) "accounts.plainHttpMailWarning" else "accounts.plainHttpWarning"),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.error,
    )
}

/** Ask for an address, for a sign-in that needs to know whose grant it keeps. */
@Composable
internal fun EmailDialog(
    title: String,
    initial: String = "",
    onDismiss: () -> Unit,
    onSubmit: (String) -> Unit,
) {
    var email by remember { mutableStateOf(initial) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            OutlinedTextField(
                value = email,
                onValueChange = { email = it },
                label = { Text(tr("accounts.fields.emailAddress")) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email, autoCorrectEnabled = false),
            )
        },
        confirmButton = { Button(enabled = '@' in email, onClick = { onSubmit(email.trim()) }) { Text(title) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) } },
    )
}

@Composable
internal fun MailboxPickerDialog(
    title: String,
    mailboxes: List<AccountSummary>,
    onDismiss: () -> Unit,
    onPick: (AccountSummary) -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                mailboxes.forEach { mailbox ->
                    Text(
                        mailbox.email,
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(8.dp))
                                .clickable { onPick(mailbox) }
                                .padding(vertical = 12.dp, horizontal = 8.dp),
                    )
                }
            }
        },
        confirmButton = {},
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) } },
    )
}

/** One value to edit, saved on the server: the dialog stays, with the
 *  server's reason, when it refuses. */
@Composable
private fun TextValueDialog(
    title: String,
    label: String,
    initial: String,
    onDismiss: () -> Unit,
    onSave: suspend (String) -> Unit,
    hint: String = "",
    password: Boolean = false,
    allowBlank: Boolean = false,
) {
    val scope = rememberCoroutineScope()
    var value by remember { mutableStateOf(initial) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        title = { Text(title) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = value,
                    onValueChange = { value = it },
                    label = { Text(label) },
                    singleLine = true,
                    visualTransformation = if (password) PasswordVisualTransformation() else androidx.compose.ui.text.input.VisualTransformation.None,
                    keyboardOptions = if (password) KeyboardOptions(keyboardType = KeyboardType.Password) else KeyboardOptions.Default,
                )
                if (hint.isNotBlank()) Text(hint, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                if (error.isNotBlank()) Text(error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
            }
        },
        confirmButton = {
            Button(
                enabled = !busy && (allowBlank || value.isNotBlank()),
                onClick = {
                    busy = true
                    error = ""
                    scope.launch {
                        runCatching { onSave(value) }.onFailure {
                            if (it is CancellationException) throw it
                            error = it.message ?: ""
                        }
                        busy = false
                    }
                },
            ) { Text(tr("buttons.save")) }
        },
        dismissButton = { TextButton(onClick = onDismiss, enabled = !busy) { Text(tr("buttons.cancel")) } },
    )
}

/** A CalDAV calendar's name, and deleting it, both on its server. */
@Composable
private fun EditCalendarDialog(
    calendar: DavCalendar,
    onDismiss: () -> Unit,
    onRename: suspend (String) -> Unit,
    onDelete: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    var name by remember { mutableStateOf(calendar.name) }
    var error by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("settings.calendars.edit")) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                if (!calendar.readOnly) {
                    OutlinedTextField(
                        value = name,
                        onValueChange = { name = it },
                        label = { Text(tr("settings.calendars.calendarName")) },
                        singleLine = true,
                    )
                }
                if (error.isNotBlank()) Text(error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                TextButton(onClick = onDelete) {
                    Icon(Icons.Filled.Delete, contentDescription = null, tint = MaterialTheme.colorScheme.error, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                    Text(tr("settings.calendars.deleteCalendar"), color = MaterialTheme.colorScheme.error)
                }
            }
        },
        confirmButton = {
            if (!calendar.readOnly) {
                Button(enabled = name.isNotBlank(), onClick = {
                    val trimmed = name.trim()
                    if (trimmed == calendar.name) return@Button onDismiss()
                    scope.launch {
                        runCatching { onRename(trimmed) }.onFailure {
                            if (it is CancellationException) throw it
                            error = it.message ?: ""
                        }
                    }
                }) { Text(tr("buttons.save")) }
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) } },
    )
}

/** Make a calendar on the account's server. */
@Composable
private fun NewCalendarDialog(
    onDismiss: () -> Unit,
    onCreate: suspend (String, String) -> Unit,
) {
    val scope = rememberCoroutineScope()
    var name by remember { mutableStateOf("") }
    var color by remember { mutableStateOf(ACCOUNT_COLORS[5].second) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf("") }
    val failed = tr("settings.calendars.createFailed")
    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        title = { Text(tr("settings.calendars.newCalendar")) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text(tr("settings.calendars.calendarName")) },
                    singleLine = true,
                )
                ColorSwatches(selected = color, onPick = { color = it })
                if (error.isNotBlank()) Text(error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
            }
        },
        confirmButton = {
            Button(enabled = name.isNotBlank() && !busy, onClick = {
                busy = true
                error = ""
                scope.launch {
                    runCatching { onCreate(name.trim(), color) }.onFailure {
                        if (it is CancellationException) throw it
                        error = it.message ?: failed
                    }
                    busy = false
                }
            }) { Text(tr("settings.calendars.create")) }
        },
        dismissButton = { TextButton(onClick = onDismiss, enabled = !busy) { Text(tr("buttons.cancel")) } },
    )
}

@Composable
private fun CalendarColorDialog(
    color: String,
    onDismiss: () -> Unit,
    onPick: (String) -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("settings.calendars.color")) },
        text = { ColorSwatches(selected = color, onPick = { if (it != color) onPick(it) else onDismiss() }) },
        confirmButton = {},
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) } },
    )
}

/** The preset colours, the current one ringed; one outside them shows too. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ColorSwatches(
    selected: String,
    onPick: (String) -> Unit,
) {
    val current = selected.trim().lowercase()
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        ACCOUNT_COLORS.forEach { (id, hex) ->
            ColorSwatch(label = tr("colors.$id"), fill = accountColorOrNull(hex), selected = current == hex, onClick = { onPick(hex) })
        }
        val custom = accountColorOrNull(current).takeIf { ACCOUNT_COLORS.none { it.second == current } }
        if (custom != null) ColorSwatch(label = tr("colors.custom"), fill = custom, selected = true, onClick = {})
    }
}

/** Switch the calendar on or off. Off leaves the Calendar screen if it is
 *  showing; on reads the synced calendars again. */
internal fun MeronMobileState.setCalendarEnabled(enabled: Boolean) {
    calendarEnabled = enabled
    saveAppBoolean(prefs, CALENDAR_ENABLED_PREF, enabled)
    if (!enabled && screen == Screen.Calendar) screen = Screen.Mail
}

/** Switch Tasks on or off, in the core too: task-list sync rests while it is
 *  off, and catches up when it is back on. */
internal fun MeronMobileState.setTasksEnabled(enabled: Boolean) {
    tasksEnabled = enabled
    saveAppBoolean(prefs, TASKS_ENABLED_PREF, enabled)
    if (!enabled && screen == Screen.Tasks) screen = Screen.Mail
    scope.launch {
        // The app mirrors the switch to the core as it changes; catch up
        // once the core has it.
        mirrorTasksEnabled()
        if (enabled) {
            runCatching { coreCall("googleTasks.sync", buildJsonObject { put("automatic", true) }) }
            runCatching { coreCall("caldav.sync", buildJsonObject { put("automatic", true) }) }
        }
    }
}
