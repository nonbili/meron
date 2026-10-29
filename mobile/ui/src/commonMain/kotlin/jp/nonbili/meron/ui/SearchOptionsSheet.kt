package jp.nonbili.meron.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.BookmarkAdd
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Event
import androidx.compose.material.icons.filled.ViewKanban
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDefaults
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuAnchorType
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.ContactSuggestParams
import jp.nonbili.meron.shared.ContactSuggestion
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.SearchFields
import jp.nonbili.meron.shared.parseContactSuggestResponse
import jp.nonbili.meron.shared.parseSearchBuildResponse
import jp.nonbili.meron.shared.parseSearchFieldsResponse
import jp.nonbili.meron.shared.requireCoreOk
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Gmail-style search options: a form for the operators the core understands
 * (`from:`, `to:`, `subject:`, `is:unread`, …). The query text stays the source
 * of truth — the sheet opens with [query] read into the form, and searching
 * hands the form back to [onSearch] as query text.
 *
 * On a board, [saveColumns] are the columns the query can be saved into as a
 * column of its own: one is saved into directly, several offer a pick.
 * From and To suggest [contactsAccountId]'s correspondents, or every
 * account's when it's blank.
 *
 * With [scopeColumns], a "Search in" picker narrows the search to one of them,
 * as on desktop: it starts at [scope] and hands the pick to [onScopeChange] on
 * search. The pick then also decides which [saveColumns] are offered and
 * whose correspondents are suggested.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SearchOptionsSheet(
    state: MeronMobileState,
    query: String,
    onDismiss: () -> Unit,
    onSearch: (String) -> Unit,
    saveColumns: List<KanbanColumnSpec> = emptyList(),
    /** A column's folder name and, below it, whose folder it is. */
    columnLabel: @Composable (KanbanColumnSpec) -> Pair<String, String> = { "" to "" },
    onSave: (String, KanbanColumnSpec) -> Unit = { _, _ -> },
    contactsAccountId: String = "",
    /** The submit button's text; "Search" by default. */
    submitLabel: String? = null,
    scopeColumns: List<KanbanColumnSpec> = emptyList(),
    scope: String = "all",
    onScopeChange: (String) -> Unit = {},
) {
    var fields by remember { mutableStateOf<SearchFields?>(null) }
    var pickingColumn by remember { mutableStateOf(false) }
    var pickedScope by remember(scope) { mutableStateOf(scope.ifBlank { "all" }) }
    val scopedColumn = scopeColumns.firstOrNull { kanbanColumnKey(it) == pickedScope }
    val offeredSaveColumns =
        if (scopeColumns.isEmpty() || scopedColumn == null) saveColumns else saveColumns.filter { it == scopedColumn }
    val contactsAccount =
        if (scopeColumns.isEmpty()) contactsAccountId else scopedColumn?.accountId?.takeUnless { it == UNIFIED_ACCOUNT_ID }.orEmpty()
    var pickingDate by remember { mutableStateOf<DateField?>(null) }
    val scope = rememberCoroutineScope()
    LaunchedEffect(query) {
        fields = state.parseSearchQuery(query) ?: SearchFields(words = query)
    }
    val save: (KanbanColumnSpec) -> Unit = { column ->
        fields?.let { current ->
            scope.launch {
                val built = state.buildSearchQuery(current)
                if (built.isNullOrBlank()) return@launch
                onSave(built, column)
                onDismiss()
            }
        }
    }
    val submit: () -> Unit = {
        fields?.let { current ->
            scope.launch {
                val built = state.buildSearchQuery(current) ?: return@launch
                if (scopeColumns.isNotEmpty()) onScopeChange(pickedScope)
                onSearch(built)
                onDismiss()
            }
        }
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        val current = fields ?: return@ModalBottomSheet
        Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp)
                .navigationBarsPadding(),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(tr("search.options"), style = MaterialTheme.typography.titleMedium)
            if (scopeColumns.isNotEmpty()) {
                SearchOptionsScopeField(state, scopeColumns, scopedColumn, columnLabel) { column ->
                    pickedScope = column?.let(::kanbanColumnKey) ?: "all"
                    pickingColumn = false
                }
            }
            SearchOptionsAddressField(state, contactsAccount, tr("composer.fields.from"), current.from, submit) {
                fields = current.copy(from = it)
            }
            SearchOptionsAddressField(state, contactsAccount, tr("composer.fields.to"), current.to, submit) {
                fields = current.copy(to = it)
            }
            SearchOptionsTextField(tr("composer.fields.subject"), current.subject, submit) { fields = current.copy(subject = it) }
            SearchOptionsTextField(tr("search.hasWords"), current.words, submit) { fields = current.copy(words = it) }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                SearchOptionsDateChip(tr("search.after"), current.after, { pickingDate = DateField.After }) {
                    fields = current.copy(after = "")
                }
                SearchOptionsDateChip(tr("search.before"), current.before, { pickingDate = DateField.Before }) {
                    fields = current.copy(before = "")
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                FilterChip(
                    selected = current.unread,
                    onClick = { fields = current.copy(unread = !current.unread) },
                    label = { Text(tr("filters.unread")) },
                )
                FilterChip(
                    selected = current.starred,
                    onClick = { fields = current.copy(starred = !current.starred) },
                    label = { Text(tr("filters.starred")) },
                )
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (offeredSaveColumns.isNotEmpty()) {
                    TextButton(
                        onClick = {
                            if (offeredSaveColumns.size == 1) save(offeredSaveColumns.first()) else pickingColumn = !pickingColumn
                        },
                        enabled = current != SearchFields(),
                    ) {
                        Icon(Icons.Filled.BookmarkAdd, contentDescription = null, Modifier.size(18.dp))
                        Spacer(Modifier.size(8.dp))
                        Text(tr("kanban.actions.saveSearchAsColumn"))
                    }
                }
                Spacer(Modifier.weight(1f))
                Button(onClick = submit) { Text(submitLabel ?: tr("kanban.actions.search")) }
            }
            if (pickingColumn) {
                HorizontalDivider()
                offeredSaveColumns.forEach { column ->
                    val (folder, account) = columnLabel(column)
                    ListItem(
                        // Whose folder it is, since a board can hold the same folder of several accounts.
                        leadingContent = {
                            Avatar(columnAvatarLabel(column, state.coreAccounts), 32.dp, columnAvatarFallbackIcon(column, state.coreAccounts))
                        },
                        headlineContent = { Text(folder) },
                        supportingContent = if (account.isBlank()) null else ({ Text(account) }),
                        modifier = Modifier.clickable { save(column) },
                    )
                }
            }
            Spacer(Modifier.size(8.dp))
        }
    }

    pickingDate?.let { field ->
        val current = fields ?: return@let
        val initial = if (field == DateField.After) current.after else current.before
        val pickerState = rememberDatePickerState(initialSelectedDateMillis = isoDateToPickerMillis(initial))
        DatePickerDialog(
            onDismissRequest = { pickingDate = null },
            confirmButton = {
                TextButton(onClick = {
                    pickerState.selectedDateMillis?.let { millis ->
                        val date = pickerMillisToIsoDate(millis)
                        fields = if (field == DateField.After) current.copy(after = date) else current.copy(before = date)
                    }
                    pickingDate = null
                }) { Text(tr("buttons.save")) }
            },
            dismissButton = { TextButton(onClick = { pickingDate = null }) { Text(tr("buttons.cancel")) } },
        ) {
            DatePicker(state = pickerState)
        }
    }
}

private enum class DateField { After, Before }

/** "Search in": all of a board's columns, or one of them, picked from a menu. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SearchOptionsScopeField(
    state: MeronMobileState,
    columns: List<KanbanColumnSpec>,
    selected: KanbanColumnSpec?,
    columnLabel: @Composable (KanbanColumnSpec) -> Pair<String, String>,
    onPick: (KanbanColumnSpec?) -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    val accounts = state.coreAccounts
    ExposedDropdownMenuBox(expanded = expanded, onExpandedChange = { expanded = it }) {
        OutlinedTextField(
            value = selected?.let { columnLabel(it).first } ?: tr("kanban.searchScope.allColumns"),
            onValueChange = {},
            readOnly = true,
            singleLine = true,
            label = { Text(tr("search.searchIn")) },
            leadingIcon = {
                if (selected == null) {
                    Icon(Icons.Filled.ViewKanban, contentDescription = null)
                } else {
                    Avatar(columnAvatarLabel(selected, accounts), 24.dp, columnAvatarFallbackIcon(selected, accounts))
                }
            },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = expanded) },
            modifier = Modifier.fillMaxWidth().menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable),
        )
        ExposedDropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            DropdownMenuItem(
                text = { Text(tr("kanban.searchScope.allColumns")) },
                leadingIcon = { Icon(Icons.Filled.ViewKanban, contentDescription = null) },
                onClick = {
                    expanded = false
                    onPick(null)
                },
            )
            columns.forEach { column ->
                val (folder, account) = columnLabel(column)
                DropdownMenuItem(
                    text = {
                        Column {
                            Text(folder, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            if (account.isNotBlank()) {
                                Text(
                                    account,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                        }
                    },
                    leadingIcon = { Avatar(columnAvatarLabel(column, accounts), 24.dp, columnAvatarFallbackIcon(column, accounts)) },
                    onClick = {
                        expanded = false
                        onPick(column)
                    },
                )
            }
        }
    }
}

@Composable
private fun SearchOptionsTextField(
    label: String,
    value: String,
    onSubmit: () -> Unit,
    onValueChange: (String) -> Unit,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        label = { Text(label) },
        singleLine = true,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        keyboardActions = KeyboardActions(onSearch = { onSubmit() }),
        modifier = Modifier.fillMaxWidth(),
    )
}

/** The comma ending the list's last complete entry, or -1; one inside quotes (`"Lee, Ann"`) is part of a name. */
private fun lastListComma(value: String): Int {
    var last = -1
    var quoted = false
    value.forEachIndexed { i, c ->
        if (c == '"') {
            quoted = !quoted
        } else if (c == ',' && !quoted) {
            last = i
        }
    }
    return last
}

/**
 * A text field suggesting addresses from cached mail as it's typed. It holds a
 * comma-separated list, any of which matches, as in Gmail; suggestions
 * complete the entry after the last comma, and a pick puts in its bare
 * address.
 */
@Composable
private fun SearchOptionsAddressField(
    state: MeronMobileState,
    accountId: String,
    label: String,
    value: String,
    onSubmit: () -> Unit,
    onValueChange: (String) -> Unit,
) {
    var focused by remember { mutableStateOf(false) }
    // Suggest only after an edit, so opening the sheet on a filled field or
    // picking a suggestion doesn't pop the list up.
    var typed by remember { mutableStateOf(false) }
    var suggestions by remember { mutableStateOf(emptyList<ContactSuggestion>()) }
    LaunchedEffect(value, typed, accountId) {
        val query = value.substring(lastListComma(value) + 1).trim()
        if (!typed || query.isEmpty()) {
            suggestions = emptyList()
            return@LaunchedEffect
        }
        delay(120)
        suggestions = state.suggestSearchContacts(accountId, query)
    }
    Column {
        OutlinedTextField(
            value = value,
            onValueChange = {
                typed = true
                onValueChange(it)
            },
            label = { Text(label) },
            singleLine = true,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            keyboardActions = KeyboardActions(onSearch = { onSubmit() }),
            modifier = Modifier.fillMaxWidth().onFocusChanged { focused = it.isFocused },
        )
        if (focused && suggestions.isNotEmpty()) {
            suggestions.forEach { contact ->
                val name = contact.name.trim()
                ListItem(
                    leadingContent = { Avatar(name.ifBlank { contact.addr }, 32.dp) },
                    headlineContent = { Text(name.ifBlank { contact.addr }, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    supportingContent =
                        if (name.isBlank() || name.equals(contact.addr, ignoreCase = true)) {
                            null
                        } else {
                            { Text(contact.addr, maxLines = 1, overflow = TextOverflow.Ellipsis) }
                        },
                    modifier =
                        Modifier.clickable {
                            typed = false
                            val head = value.substring(0, lastListComma(value).coerceAtLeast(0))
                            onValueChange(if (head.isEmpty()) "${contact.addr}, " else "$head, ${contact.addr}, ")
                        },
                )
            }
        }
    }
}

@Composable
private fun SearchOptionsDateChip(
    label: String,
    date: String,
    onPick: () -> Unit,
    onClear: () -> Unit,
) {
    AssistChip(
        onClick = onPick,
        label = { Text(if (date.isBlank()) label else "$label $date") },
        leadingIcon = { Icon(Icons.Filled.Event, contentDescription = null, Modifier.size(18.dp)) },
        trailingIcon =
            if (date.isBlank()) {
                null
            } else {
                {
                    IconButton(onClick = onClear, modifier = Modifier.size(24.dp)) {
                        Icon(Icons.Filled.Close, contentDescription = null, Modifier.size(16.dp))
                    }
                }
            },
    )
}

private suspend fun MeronMobileState.parseSearchQuery(query: String): SearchFields? =
    runCatching {
        withContext(ioDispatcher) {
            parseSearchFieldsResponse(requireCoreOk(MobileMailCommandClient(core).parseSearch(query)))
        }
    }.getOrNull()

private suspend fun MeronMobileState.suggestSearchContacts(
    accountId: String,
    query: String,
): List<ContactSuggestion> =
    runCatching {
        withContext(ioDispatcher) {
            parseContactSuggestResponse(
                MobileMailCommandClient(core).suggestContacts(ContactSuggestParams(accountId = accountId, query = query, limit = 6)),
            )
        }
    }.getOrDefault(emptyList())

private suspend fun MeronMobileState.buildSearchQuery(fields: SearchFields): String? =
    runCatching {
        withContext(ioDispatcher) {
            parseSearchBuildResponse(requireCoreOk(MobileMailCommandClient(core).buildSearch(fields)))
        }
    }.getOrNull()

/**
 * A `yyyy-mm-dd` date as the UTC-midnight millis the date picker works in, or
 * null outside the picker's years: it throws on an initial date it can't show,
 * and a typed `before:1850/01/01` is a valid query.
 */
internal fun isoDateToPickerMillis(date: String): Long? {
    val parts = date.split("-").mapNotNull { it.toIntOrNull() }
    if (parts.size != 3 || parts[0] !in DatePickerDefaults.YearRange) return null
    return daysFromCivil(parts[0], parts[1], parts[2]) * 86_400_000L
}

/** The inverse of [isoDateToPickerMillis]. */
internal fun pickerMillisToIsoDate(utcMillis: Long): String {
    val (year, month, day) = civilFromDays(utcMillis.floorDiv(86_400_000L))
    return "${year.toString().padStart(4, '0')}-${month.toString().padStart(2, '0')}-${day.toString().padStart(2, '0')}"
}
