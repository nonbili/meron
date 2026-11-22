package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Download
import androidx.compose.material.icons.filled.FilterList
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Sync
import androidx.compose.material.icons.filled.Upload
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/*
 * The calendar's own actions, after desktop's CalendarSyncButton,
 * CalendarFilter and CalendarFileMenu: sync now, which calendars the views
 * show, and moving events in and out as iCalendar (.ics) files.
 */

/** An import picked and previewed, waiting for the user to go ahead. */
private data class IcsImport(
    val name: String,
    val ics: String,
    val imported: Int,
    val duplicates: Int,
    val skipped: Int,
)

private fun JsonObject.count(key: String): Int = text(key).toIntOrNull() ?: 0

/** The calendar bar's overflow: sync, show calendars, import, export. */
@Composable
internal fun CalendarActionsMenu(state: MeronMobileState) =
    with(state) {
        val scope = rememberCoroutineScope()
        var menu by remember { mutableStateOf(false) }
        var filtering by remember { mutableStateOf(false) }
        var exporting by remember { mutableStateOf(false) }
        var importing by remember { mutableStateOf<IcsImport?>(null) }
        var busy by remember { mutableStateOf(false) }
        var error by remember { mutableStateOf("") }
        val failed = tr("calendar.file.failed")

        fun pickImport() {
            services.pickFile(listOf("text/calendar", "*/*")) { picked ->
                if (picked == null) return@pickFile
                scope.launch {
                    runCatching {
                        // The core's own limit, before decoding a file far past it.
                        if (picked.bytes.size > 8 * 1024 * 1024) error("Calendar file exceeds 8 MiB")
                        val ics = picked.bytes.decodeToString(throwOnInvalidSequence = true)
                        val preview = coreCall("calendar.previewImport", buildJsonObject { put("ics", ics) })
                        IcsImport(picked.name, ics, preview.count("imported"), preview.count("duplicates"), preview.count("skipped"))
                    }.onSuccess { importing = it }
                        .onFailure {
                            if (it is CancellationException) throw it
                            snackbarHost.showSnackbar(withReason(failed, it.message) { key, args -> trs(key, args) })
                        }
                }
            }
        }

        Box {
            IconButton(onClick = { menu = true }) {
                Icon(Icons.Filled.MoreVert, contentDescription = tr("calendar.file.actions"))
            }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                if (syncedCalendars.isNotEmpty() || calendarAccounts.isNotEmpty()) {
                    // A failure shows on the account in Calendars & tasks, as on desktop.
                    val failure = calendarAccounts.firstOrNull { it.lastError.isNotBlank() }?.lastError
                    DropdownMenuItem(
                        text = {
                            Column {
                                Text(tr("settings.calendars.syncNow"))
                                if (failure != null) {
                                    Text(
                                        tr("settings.calendars.syncFailed", mapOf("error" to failure)),
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.error,
                                        maxLines = 2,
                                        overflow = TextOverflow.Ellipsis,
                                    )
                                }
                            }
                        },
                        leadingIcon = { Icon(Icons.Filled.Sync, contentDescription = null) },
                        enabled = !calendarSyncing,
                        onClick = {
                            menu = false
                            scope.launch { syncAllCalendars() }
                        },
                    )
                }
                // Also while this device's events are hidden, the only way back.
                if (syncedCalendars.isNotEmpty() || hiddenCalendars.isNotEmpty()) {
                    DropdownMenuItem(
                        text = { Text(tr("calendar.chooseCalendars")) },
                        leadingIcon = { Icon(Icons.Filled.FilterList, contentDescription = null) },
                        onClick = {
                            menu = false
                            filtering = true
                        },
                    )
                }
                DropdownMenuItem(
                    text = { Text(tr("calendar.file.import")) },
                    leadingIcon = { Icon(Icons.Filled.Upload, contentDescription = null) },
                    onClick = {
                        menu = false
                        pickImport()
                    },
                )
                DropdownMenuItem(
                    text = { Text(tr("calendar.file.export")) },
                    leadingIcon = { Icon(Icons.Filled.Download, contentDescription = null) },
                    onClick = {
                        menu = false
                        exporting = true
                    },
                )
            }
        }

        if (filtering) {
            CalendarFilterDialog(state, onDismiss = { filtering = false })
        }

        importing?.let { file ->
            AlertDialog(
                onDismissRequest = { if (!busy) importing = null },
                title = { Text(tr("calendar.file.importTitle")) },
                text = {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(file.name, style = MaterialTheme.typography.titleSmall)
                        Text(
                            tr(
                                "calendar.file.preview",
                                mapOf(
                                    "imported" to file.imported,
                                    "duplicates" to file.duplicates,
                                    "skipped" to file.skipped,
                                    "calendar" to tr("calendar.onThisDevice"),
                                ),
                            ),
                        )
                        if (error.isNotBlank()) Text(error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                    }
                },
                confirmButton = {
                    Button(enabled = !busy && file.imported > 0, onClick = {
                        busy = true
                        error = ""
                        scope.launch {
                            runCatching { coreCall("calendar.importIcs", buildJsonObject { put("ics", file.ics) }) }
                                .onSuccess { summary ->
                                    importing = null
                                    loadCalendar()
                                    snackbarHost.showSnackbar(
                                        trs(
                                            "calendar.file.imported",
                                            mapOf(
                                                "imported" to summary.count("imported"),
                                                "duplicates" to summary.count("duplicates"),
                                                "skipped" to summary.count("skipped"),
                                            ),
                                        ),
                                    )
                                }.onFailure {
                                    if (it is CancellationException) throw it
                                    error = withReason(failed, it.message) { key, args -> trs(key, args) }
                                }
                            busy = false
                        }
                    }) { Text(tr("common.import")) }
                },
                dismissButton = { TextButton(enabled = !busy, onClick = { importing = null }) { Text(tr("buttons.cancel")) } },
            )
        }

        if (exporting) {
            ExportCalendarDialog(
                calendars = syncedCalendars,
                busy = busy,
                error = error,
                onDismiss = {
                    exporting = false
                    error = ""
                },
                onExport = { calendar ->
                    busy = true
                    error = ""
                    scope.launch {
                        runCatching { coreCall("calendar.exportIcs", buildJsonObject { put("calendar", calendar) }).text("ics") }
                            .onSuccess { ics ->
                                busy = false
                                if (ics.isBlank()) {
                                    error = trs("calendar.file.failed")
                                    return@onSuccess
                                }
                                saveIcs(ics, "meron-calendar.ics") { exporting = false }
                            }.onFailure {
                                if (it is CancellationException) throw it
                                busy = false
                                error = withReason(failed, it.message) { key, args -> trs(key, args) }
                            }
                    }
                },
            )
        }
    }

/** Hand an export to the platform's save dialog; [onSaved] runs once it is written. */
internal fun MeronMobileState.saveIcs(
    ics: String,
    fileName: String,
    onSaved: () -> Unit = {},
) {
    services.saveFile(ics.encodeToByteArray(), fileName, "text/calendar") { result ->
        scope.launch {
            result
                .onSuccess { saved ->
                    if (saved) {
                        onSaved()
                        snackbarHost.showSnackbar(trs("calendar.file.exported"))
                    }
                }.onFailure { snackbarHost.showSnackbar(trsError("calendar.file.failed", it)) }
        }
    }
}

/** Export one event (or its whole series) as an .ics file. */
internal fun MeronMobileState.exportCalendarEvent(eventId: String) {
    scope.launch {
        runCatching { coreCall("calendar.exportIcs", buildJsonObject { put("event_id", eventId) }).text("ics") }
            .onSuccess { if (it.isNotBlank()) saveIcs(it, "meron-event.ics") }
            .onFailure {
                if (it is CancellationException) throw it
                snackbarHost.showSnackbar(trsError("calendar.file.failed", it))
            }
    }
}

/** Export the events on this device, or one synced calendar's. */
@Composable
private fun ExportCalendarDialog(
    calendars: List<SyncedCalendar>,
    busy: Boolean,
    error: String,
    onDismiss: () -> Unit,
    onExport: (String) -> Unit,
) {
    var calendar by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        title = { Text(tr("calendar.file.exportTitle")) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                CalendarChoice(tr("calendar.onThisDevice"), null, calendar == "") { calendar = "" }
                calendars.forEach {
                    CalendarChoice(bookLabel(it.name, it.accountName), calendarColor(it.id, it.color), calendar == it.id) { calendar = it.id }
                }
                if (error.isNotBlank()) Text(error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
            }
        },
        confirmButton = { Button(enabled = !busy, onClick = { onExport(calendar) }) { Text(tr("common.export")) } },
        dismissButton = { TextButton(enabled = !busy, onClick = onDismiss) { Text(tr("buttons.cancel")) } },
    )
}

@Composable
private fun CalendarChoice(
    label: String,
    color: androidx.compose.ui.graphics.Color?,
    selected: Boolean,
    onClick: () -> Unit,
) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 44.dp)
            .clip(MaterialTheme.shapes.small)
            .clickable(onClick = onClick),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RadioButton(selected = selected, onClick = null, modifier = Modifier.padding(horizontal = 8.dp))
        if (color != null) {
            Box(
                Modifier
                    .padding(end = 8.dp)
                    .size(10.dp)
                    .clip(CircleShape)
                    .background(color),
            )
        }
        Text(label, maxLines = 2, overflow = TextOverflow.Ellipsis)
    }
}

/** Which calendars the views show. Only what is drawn: hidden events stay
 *  synced and still alert. */
@Composable
private fun CalendarFilterDialog(
    state: MeronMobileState,
    onDismiss: () -> Unit,
) = with(state) {
    fun toggle(key: String) {
        hiddenCalendars = if (key in hiddenCalendars) hiddenCalendars - key else hiddenCalendars + key
        saveAppStringSet(prefs, HIDDEN_CALENDARS_PREF, hiddenCalendars)
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("calendar.chooseCalendars")) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                FilterRow(tr("calendar.onThisDevice"), null, LOCAL_CALENDAR !in hiddenCalendars) { toggle(LOCAL_CALENDAR) }
                syncedCalendars.groupBy { it.accountId }.values.forEach { group ->
                    Text(
                        group.first().accountName,
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(start = 8.dp, top = 12.dp, bottom = 4.dp),
                    )
                    group.forEach { calendar ->
                        FilterRow(calendar.name, calendarColor(calendar.id, calendar.color), calendar.id !in hiddenCalendars) { toggle(calendar.id) }
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.done")) } },
    )
}

@Composable
private fun FilterRow(
    label: String,
    color: androidx.compose.ui.graphics.Color?,
    checked: Boolean,
    onToggle: () -> Unit,
) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 44.dp)
            .clip(MaterialTheme.shapes.small)
            .clickable(onClick = onToggle),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Checkbox(checked = checked, onCheckedChange = { onToggle() })
        if (color != null) {
            Box(
                Modifier
                    .padding(end = 8.dp)
                    .size(10.dp)
                    .clip(CircleShape)
                    .background(color),
            )
        }
        Text(label, maxLines = 2, overflow = TextOverflow.Ellipsis)
    }
}
