package jp.nonbili.meron.ui

import androidx.compose.animation.core.animate
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.gestures.rememberDraggableState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Sort
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Contacts
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.PushPin
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.outlined.Email
import androidx.compose.material.icons.outlined.PushPin
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.backhandler.BackHandler
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.ContactSuggestion
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.formatContactSuggestion
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlin.math.roundToInt

@Composable
internal fun rememberContactsScreenState(core: MeronCore): ContactsScreenState = remember(core) { ContactsScreenState(core) }

/** Local directory; it never changes the mailbox's current query or folder. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalComposeUiApi::class)
@Composable
internal fun ContactsScreen(
    core: MeronCore,
    onBack: () -> Unit,
    onCompose: (String) -> Unit,
    onOpenThread: (ThreadSummary) -> Unit,
    onCopy: (String, String) -> Unit,
    initialContact: ContactSuggestion? = null,
    canCompose: Boolean = true,
    state: ContactsScreenState = rememberContactsScreenState(core),
    onInitialContactConsumed: () -> Unit = {},
    active: Boolean = true,
    accounts: List<AccountSummary> = emptyList(),
    foldersByAccount: Map<String, List<FolderSummary>> = emptyMap(),
    showSenderImages: Boolean = true,
) {
    val scope = rememberCoroutineScope()
    val query = state.query
    val sort = state.sort
    val limit = state.limit
    var retry by remember { mutableStateOf(0) }
    var sortOpen by remember { mutableStateOf(false) }
    val selected = state.selected
    val goBack = { if (state.selected != null) state.select(null) else onBack() }
    val colors = MaterialTheme.colorScheme
    BackHandler(enabled = active && selected != null) { state.select(null) }
    LaunchedEffect(core) { state.loadPins() }
    LaunchedEffect(initialContact) {
        if (initialContact != null) {
            if (contactAddressKey(initialContact.addr) != state.selected?.addr) state.select(initialContact)
            onInitialContactConsumed()
        }
    }
    LaunchedEffect(query, sort, limit, retry) {
        delay(150)
        state.loadDirectory(query, sort, limit)
    }
    LaunchedEffect(selected?.addr) {
        if (selected != null) {
            launch { state.loadHistory() }
            launch { state.loadNote(selected.addr) }
        }
    }
    Scaffold(
        containerColor = colors.surface,
        topBar = {
            Column(Modifier.background(colors.surface)) {
                TopAppBar(
                    title = {
                        Text(
                            tr("contacts.title"),
                            fontSize = 18.sp,
                            fontWeight = FontWeight.SemiBold,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    },
                    colors = TopAppBarDefaults.topAppBarColors(containerColor = colors.surface),
                    navigationIcon = {
                        IconButton(onClick = goBack) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = tr("buttons.back"))
                        }
                    },
                    actions = {
                        if (selected != null) {
                            val pinned = state.isPinned(selected)
                            val pinLabel = tr(if (pinned) "contacts.unpin" else "contacts.pin")
                            IconButton(
                                onClick = { scope.launch { state.togglePin(selected) } },
                                enabled = state.pinsLoaded && !state.pinsSaving,
                            ) {
                                Icon(
                                    if (pinned) Icons.Filled.PushPin else Icons.Outlined.PushPin,
                                    contentDescription = pinLabel,
                                    tint = if (pinned) colors.primary else colors.onSurfaceVariant,
                                )
                            }
                        }
                    },
                )
                if (selected == null) {
                    ContactsSearchBar(
                        query = query,
                        onQueryChange = {
                            state.query = it
                            state.limit = 100
                        },
                        sort = sort,
                        sortOpen = sortOpen,
                        onSortOpenChange = { sortOpen = it },
                        sortEnabled = state.pinsLoaded && !state.sortSaving,
                        onSelectSort = { option ->
                            scope.launch { state.setSort(option) }
                            sortOpen = false
                        },
                    )
                }
                HorizontalDivider(color = colors.outlineVariant.copy(alpha = 0.5f))
            }
        },
    ) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(padding)
                .consumeWindowInsets(padding)
                .imePadding(),
        ) {
            if (state.pinsError) {
                ContactFailure(
                    onRetry = {
                        scope.launch {
                            if (!state.pinsLoaded) {
                                state.loadPins()
                            } else {
                                state.retryPin()
                            }
                        }
                    },
                )
            }
            if (state.sortError) ContactFailure { scope.launch { state.retrySort() } }
            if (selected == null) {
                val matchingPins =
                    state.pins
                        .filter {
                            "${it.name} ${it.addr}".contains(query.trim(), ignoreCase = true)
                        }.sortedBy { it.name.ifBlank { it.addr }.lowercase() }
                val others = state.contacts.filterNot(state::isPinned)
                val directoryListState = rememberLazyListState()
                LazyColumn(
                    Modifier.fillMaxSize().appScrollbar(directoryListState),
                    state = directoryListState,
                    contentPadding = PaddingValues(vertical = 6.dp),
                ) {
                    if (matchingPins.isNotEmpty()) {
                        item(key = "section:pinned") { ContactSectionTitle(tr("contacts.pinned")) }
                        items(matchingPins, key = { "pin:${contactAddressKey(it.addr)}" }) { contact ->
                            ContactDirectoryRow(
                                state = state,
                                contact = contact,
                                showSenderImages = showSenderImages,
                                onSelect = { state.select(contact) },
                                onPin = { scope.launch { state.togglePin(contact) } },
                            )
                        }
                        if (others.isNotEmpty()) {
                            item(key = "section:other") { ContactSectionTitle(tr("contacts.other")) }
                        }
                    }
                    items(others, key = { contactAddressKey(it.addr) }) { contact ->
                        ContactDirectoryRow(
                            state = state,
                            contact = contact,
                            showSenderImages = showSenderImages,
                            onSelect = { state.select(contact) },
                            onPin = { scope.launch { state.togglePin(contact) } },
                        )
                    }
                    if (state.directoryLoading) item(key = "loading") { ContactLoading() }
                    if (state.directoryError) item(key = "error") { ContactFailure { retry++ } }
                    if (!state.directoryLoading && !state.directoryError && matchingPins.isEmpty() && others.isEmpty()) {
                        item(key = "empty") {
                            Column(
                                Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 48.dp),
                                horizontalAlignment = Alignment.CenterHorizontally,
                                verticalArrangement = Arrangement.spacedBy(10.dp),
                            ) {
                                Icon(
                                    Icons.Filled.Contacts,
                                    contentDescription = null,
                                    modifier = Modifier.size(36.dp),
                                    tint = colors.onSurfaceVariant.copy(alpha = 0.55f),
                                )
                                Text(
                                    tr("contacts.empty"),
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = colors.onSurfaceVariant,
                                )
                            }
                        }
                    }
                    if (!state.directoryError && state.contacts.size >= limit) {
                        item(key = "more") {
                            Box(Modifier.fillMaxWidth().padding(vertical = 12.dp), contentAlignment = Alignment.Center) {
                                OutlinedButton(
                                    onClick = { state.limit += 100 },
                                    enabled = !state.directoryLoading,
                                ) {
                                    Text(tr("threads.actions.loadMore"))
                                }
                            }
                        }
                    }
                }
            } else {
                ContactDetails(
                    state = state,
                    contact = selected,
                    onCompose = onCompose,
                    onOpenThread = onOpenThread,
                    onCopy = onCopy,
                    canCompose = canCompose,
                    accounts = accounts,
                    foldersByAccount = foldersByAccount,
                    showSenderImages = showSenderImages,
                )
            }
        }
    }
}

@Composable
private fun ContactsSearchBar(
    query: String,
    onQueryChange: (String) -> Unit,
    sort: String,
    sortOpen: Boolean,
    onSortOpenChange: (Boolean) -> Unit,
    sortEnabled: Boolean,
    onSelectSort: (String) -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val focusRequester = remember { FocusRequester() }
    Row(
        Modifier
            .fillMaxWidth()
            .padding(start = 16.dp, end = 16.dp, bottom = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Surface(
            modifier = Modifier.weight(1f).height(42.dp),
            shape = RoundedCornerShape(12.dp),
            color = colors.surfaceContainerHigh,
        ) {
            Row(
                Modifier
                    .fillMaxSize()
                    .padding(start = 12.dp, end = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(
                    Icons.Filled.Search,
                    contentDescription = null,
                    tint = colors.onSurfaceVariant,
                    modifier = Modifier.size(18.dp),
                )
                BasicTextField(
                    value = query,
                    onValueChange = onQueryChange,
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    cursorBrush = SolidColor(colors.primary),
                    textStyle = MaterialTheme.typography.bodyMedium.copy(color = colors.onSurface),
                    modifier =
                        Modifier
                            .weight(1f)
                            .padding(start = 10.dp, end = 4.dp)
                            .focusRequester(focusRequester),
                    decorationBox = { innerTextField ->
                        Box(
                            Modifier.fillMaxWidth(),
                            contentAlignment = Alignment.CenterStart,
                        ) {
                            if (query.isEmpty()) {
                                Text(
                                    tr("contacts.search"),
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = colors.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                            innerTextField()
                        }
                    },
                )
                if (query.isNotEmpty()) {
                    IconButton(
                        onClick = { onQueryChange("") },
                        modifier = Modifier.size(34.dp),
                    ) {
                        Icon(
                            Icons.Filled.Close,
                            contentDescription = tr("common.clearSearch"),
                            tint = colors.onSurfaceVariant,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
            }
        }
        Box {
            Surface(
                modifier = Modifier.size(42.dp),
                shape = RoundedCornerShape(12.dp),
                color = if (sortOpen) colors.secondaryContainer else colors.surfaceContainerHigh,
            ) {
                IconButton(
                    onClick = { onSortOpenChange(true) },
                    enabled = sortEnabled,
                    modifier = Modifier.fillMaxSize(),
                ) {
                    Icon(
                        Icons.AutoMirrored.Filled.Sort,
                        contentDescription = tr("contacts.sort") + ": " + tr(contactSortLabel(sort)),
                        tint = if (sortOpen) colors.onSecondaryContainer else colors.onSurfaceVariant,
                        modifier = Modifier.size(20.dp),
                    )
                }
            }
            DropdownMenu(expanded = sortOpen, onDismissRequest = { onSortOpenChange(false) }) {
                listOf("frequent", "recent", "name").forEach { option ->
                    val activeSort = sort == option
                    DropdownMenuItem(
                        text = {
                            Text(
                                tr(contactSortLabel(option)),
                                fontWeight = if (activeSort) FontWeight.SemiBold else FontWeight.Normal,
                            )
                        },
                        onClick = { onSelectSort(option) },
                        trailingIcon = {
                            if (activeSort) {
                                Icon(Icons.Filled.Check, contentDescription = null, tint = colors.primary)
                            }
                        },
                    )
                }
            }
        }
    }
}

private fun contactSortLabel(sort: String): String =
    when (sort) {
        "recent" -> "contacts.sortRecent"
        "name" -> "contacts.sortName"
        else -> "contacts.sortFrequent"
    }

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ContactDirectoryRow(
    state: ContactsScreenState,
    contact: ContactSuggestion,
    showSenderImages: Boolean,
    onSelect: () -> Unit,
    onPin: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val pinned = state.isPinned(contact)
    val pinLabel = tr(if (pinned) "contacts.unpin" else "contacts.pin")
    val enabled = state.pinsLoaded && !state.pinsSaving
    var menuOpen by remember(contact.addr) { mutableStateOf(false) }
    var reveal by remember(contact.addr) { mutableFloatStateOf(0f) }
    val revealWidth = with(LocalDensity.current) { 88.dp.toPx() }
    val direction = if (LocalLayoutDirection.current == LayoutDirection.Rtl) -1f else 1f
    val displayName = contact.name.ifBlank { contact.addr }
    val hasDistinctName = contact.name.isNotBlank() && !contact.name.equals(contact.addr, ignoreCase = true)
    Box(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 1.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(if (reveal < -0.5f) colors.secondaryContainer else colors.surface),
    ) {
        if (reveal < -0.5f) {
            Box(
                modifier =
                    Modifier
                        .align(Alignment.CenterEnd)
                        .width(88.dp)
                        .heightIn(min = 58.dp)
                        .clickable(enabled = enabled) {
                            reveal = 0f
                            onPin()
                        }.padding(horizontal = 6.dp),
                contentAlignment = Alignment.Center,
            ) {
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    Icon(
                        if (pinned) Icons.Outlined.PushPin else Icons.Filled.PushPin,
                        contentDescription = null,
                        tint = colors.onSecondaryContainer,
                        modifier = Modifier.size(18.dp),
                    )
                    Text(
                        pinLabel,
                        style = MaterialTheme.typography.labelSmall,
                        color = colors.onSecondaryContainer,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
        Row(
            Modifier
                .offset { IntOffset((reveal * direction).roundToInt(), 0) }
                .fillMaxWidth()
                .heightIn(min = 58.dp)
                .background(colors.surface)
                .semantics {
                    if (enabled) {
                        customActions =
                            listOf(
                                CustomAccessibilityAction(pinLabel) {
                                    reveal = 0f
                                    onPin()
                                    true
                                },
                            )
                    }
                }
                // A bounded reveal, never a dismiss: releasing exposes a button, not an action.
                .draggable(
                    state = rememberDraggableState { delta -> reveal = (reveal + delta * direction).coerceIn(-revealWidth, 0f) },
                    orientation = Orientation.Horizontal,
                    enabled = enabled,
                    onDragStopped = { velocity ->
                        val target = if (velocity * direction < -600f || (velocity * direction <= 600f && reveal < -revealWidth / 2)) -revealWidth else 0f
                        animate(reveal, target) { value, _ -> reveal = value }
                    },
                ).combinedClickable(
                    onClick = {
                        if (reveal < -0.5f) {
                            reveal = 0f
                        } else {
                            onSelect()
                        }
                    },
                    onLongClickLabel = pinLabel,
                    onLongClick = { menuOpen = true },
                ).padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            SenderAvatar(
                formatContactSuggestion(contact),
                enabled = showSenderImages,
                size = 40.dp,
                fallbackName = displayName,
            )
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(
                    displayName,
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = FontWeight.Medium,
                    color = colors.onSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (hasDistinctName) {
                    Text(
                        contact.addr,
                        style = MaterialTheme.typography.bodySmall,
                        color = colors.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            if (pinned) {
                Icon(
                    Icons.Filled.PushPin,
                    contentDescription = null,
                    tint = colors.primary.copy(alpha = 0.7f),
                    modifier = Modifier.size(15.dp),
                )
            }
        }
        DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
            ContactPinMenuItem(state, contact) {
                menuOpen = false
                reveal = 0f
                onPin()
            }
        }
    }
}

@Composable
private fun ContactPinMenuItem(
    state: ContactsScreenState,
    contact: ContactSuggestion,
    onClick: () -> Unit,
) {
    val pinned = state.isPinned(contact)
    DropdownMenuItem(
        text = { Text(tr(if (pinned) "contacts.unpin" else "contacts.pin")) },
        leadingIcon = { Icon(if (pinned) Icons.Outlined.PushPin else Icons.Filled.PushPin, null) },
        onClick = onClick,
        enabled = state.pinsLoaded && !state.pinsSaving,
    )
}

@Composable
private fun ContactDetails(
    state: ContactsScreenState,
    contact: ContactSuggestion,
    onCompose: (String) -> Unit,
    onOpenThread: (ThreadSummary) -> Unit,
    onCopy: (String, String) -> Unit,
    canCompose: Boolean,
    accounts: List<AccountSummary>,
    foldersByAccount: Map<String, List<FolderSummary>>,
    showSenderImages: Boolean,
) {
    val scope = rememberCoroutineScope()
    val colors = MaterialTheme.colorScheme
    val copyLabel = tr("chat.copyEmailAddress")
    var copied by remember(contact.addr) { mutableStateOf(false) }
    val listState = rememberLazyListState()
    val accountsById = remember(accounts) { accounts.associateBy { it.id } }
    val inboxLabel = tr("folders.roles.inbox")
    val displayName = contact.name.ifBlank { contact.addr }
    val hasDistinctName = contact.name.isNotBlank() && !contact.name.equals(contact.addr, ignoreCase = true)
    LaunchedEffect(copied) {
        if (copied) {
            delay(1500)
            copied = false
        }
    }
    LazyColumn(
        Modifier.fillMaxSize().appScrollbar(listState),
        state = listState,
        contentPadding = PaddingValues(bottom = 24.dp),
    ) {
        item(key = "header:${contact.addr}") {
            Row(
                Modifier
                    .fillMaxWidth()
                    .padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                SenderAvatar(
                    formatContactSuggestion(contact),
                    enabled = showSenderImages,
                    size = 48.dp,
                    fallbackName = displayName,
                )
                Column(
                    Modifier.weight(1f),
                    verticalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(4.dp),
                    ) {
                        Text(
                            displayName,
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.SemiBold,
                            color = colors.onSurface,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f, fill = false),
                        )
                        if (!hasDistinctName) {
                            ContactCopyAddressButton(
                                copied = copied,
                                onClick = {
                                    onCopy(copyLabel, contact.addr)
                                    copied = true
                                },
                            )
                        }
                    }
                    if (hasDistinctName) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(4.dp),
                        ) {
                            SelectionContainer(Modifier.weight(1f, fill = false)) {
                                Text(
                                    contact.addr,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = colors.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                            ContactCopyAddressButton(
                                copied = copied,
                                onClick = {
                                    onCopy(copyLabel, contact.addr)
                                    copied = true
                                },
                            )
                        }
                    }
                }
                if (canCompose) {
                    FilledTonalIconButton(
                        onClick = { onCompose(formatContactSuggestion(contact)) },
                        colors =
                            IconButtonDefaults.filledTonalIconButtonColors(
                                containerColor = colors.primaryContainer,
                                contentColor = colors.onPrimaryContainer,
                            ),
                        modifier = Modifier.size(40.dp),
                    ) {
                        Icon(
                            Icons.Outlined.Email,
                            contentDescription = tr("composer.actions.newMessage"),
                            modifier = Modifier.size(19.dp),
                        )
                    }
                }
            }
        }
        item(key = "note:${contact.addr}") { ContactNotes(state, contact.addr) }
        item(key = "section:conversations") {
            HorizontalDivider(color = colors.outlineVariant.copy(alpha = 0.5f))
            ContactSectionTitle(tr("contacts.recentConversations"))
        }
        items(state.threads, key = { it.id }) { thread ->
            ContactThreadRow(
                thread = thread,
                account = accountsById[thread.accountId],
                // History spans folders, so each row names where it lives.
                folderLabel =
                    thread.folder
                        .takeIf { it.isNotBlank() }
                        ?.let { folder ->
                            threadFolderLabel(folder, foldersByAccount[thread.accountId].orEmpty(), inboxLabel)
                        }.orEmpty(),
                showSenderImages = showSenderImages,
                onOpen = { onOpenThread(thread) },
            )
            HorizontalDivider(
                Modifier.padding(start = 66.dp, end = 16.dp),
                color = colors.outlineVariant.copy(alpha = 0.35f),
            )
        }
        if (state.historyLoading) item(key = "history:loading") { ContactLoading() }
        if (state.historyError) {
            item(key = "history:error") {
                ContactFailure { scope.launch { state.loadHistory(more = state.historyLoaded) } }
            }
        }
        if (state.historyLoaded && !state.historyLoading && !state.historyError) {
            if (state.threads.isEmpty()) {
                item(key = "history:empty") {
                    Text(
                        tr("contacts.noConversations"),
                        Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                        style = MaterialTheme.typography.bodyMedium,
                        color = colors.onSurfaceVariant,
                    )
                }
            }
            if (state.cursor.isNotEmpty()) {
                item(key = "history:more") {
                    Box(Modifier.fillMaxWidth().padding(vertical = 12.dp), contentAlignment = Alignment.Center) {
                        OutlinedButton(onClick = { scope.launch { state.loadHistory(more = true) } }) {
                            Text(tr("threads.actions.loadMore"))
                        }
                    }
                }
            }
            item(key = "history:footnote") {
                Text(
                    tr("contacts.cachedHistory"),
                    Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
                    style = MaterialTheme.typography.labelSmall,
                    color = colors.onSurfaceVariant.copy(alpha = 0.75f),
                )
            }
        }
    }
}

@Composable
private fun ContactCopyAddressButton(
    copied: Boolean,
    onClick: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    IconButton(
        onClick = onClick,
        modifier = Modifier.size(26.dp),
    ) {
        Icon(
            if (copied) Icons.Filled.Check else Icons.Filled.ContentCopy,
            contentDescription = tr(if (copied) "common.copied" else "chat.copyEmailAddress"),
            tint = if (copied) colors.primary else colors.onSurfaceVariant,
            modifier = Modifier.size(14.dp),
        )
    }
}

@Composable
private fun ContactThreadRow(
    thread: ThreadSummary,
    account: AccountSummary?,
    folderLabel: String,
    showSenderImages: Boolean,
    onOpen: () -> Unit,
) {
    val unread = thread.unread
    val colors = MaterialTheme.colorScheme
    val chat = LocalChatColors.current
    val senderLabel = thread.sender.ifBlank { thread.accountId }
    val hasDraft = thread.hasDraft || thread.folderRole == "drafts" || (thread.folderRole == "folder" && folderIsDrafts(thread.folder))
    val draftBadge = tr("chat.draft")
    val rowBackground =
        if (unread) {
            colors.primary.copy(alpha = 0.06f).compositeOver(colors.surface)
        } else {
            colors.surface
        }
    Row(
        Modifier
            .fillMaxWidth()
            .background(rowBackground)
            .clickable(onClick = onOpen)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        ThreadAvatarWithAccountBadge(
            senderLabel = senderLabel,
            showSenderImages = showSenderImages,
            account = account,
        )
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Row(
                    Modifier.weight(1f),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    val me = tr("threads.me")
                    Text(
                        if (thread.senders.size > 1) {
                            val names = thread.senders.map { it.ifBlank { me } }
                            if (thread.sendersTruncated) {
                                names.first() + " .. " + names.drop(1).joinToString(", ")
                            } else {
                                names.joinToString(", ")
                            }
                        } else {
                            senderLabel.substringBefore('@')
                        },
                        fontSize = 13.sp,
                        fontWeight = if (unread) FontWeight.SemiBold else FontWeight.Medium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f, fill = false),
                        color = colors.onSurface,
                    )
                    if (thread.messageCount > 1) {
                        Text(
                            thread.messageCount.toString(),
                            fontSize = 12.sp,
                            fontWeight = FontWeight.Normal,
                            color = colors.onSurfaceVariant,
                            modifier = Modifier.padding(start = 4.dp),
                        )
                    }
                }
                if (folderLabel.isNotBlank()) {
                    Surface(
                        modifier = Modifier.widthIn(max = 96.dp),
                        shape = RoundedCornerShape(4.dp),
                        color = colors.surfaceContainerHigh,
                    ) {
                        Text(
                            folderLabel,
                            modifier = Modifier.padding(horizontal = 5.dp, vertical = 1.dp),
                            fontSize = 10.sp,
                            fontWeight = FontWeight.Medium,
                            color = colors.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
                if (thread.dateEpochSeconds > 0) {
                    Text(
                        formatInboxTimestamp(thread.dateEpochSeconds),
                        fontSize = 11.sp,
                        fontWeight = if (unread) FontWeight.Medium else FontWeight.Normal,
                        color = if (unread) colors.primary else colors.onSurfaceVariant,
                        maxLines = 1,
                    )
                }
                if (unread) {
                    val unreadContentDescription = tr("filters.unread")
                    Box(
                        Modifier
                            .size(8.dp)
                            .clip(CircleShape)
                            .background(colors.primary)
                            .semantics { contentDescription = unreadContentDescription },
                    )
                }
            }
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(5.dp),
            ) {
                if (thread.starred) {
                    Icon(
                        Icons.Filled.Star,
                        contentDescription = null,
                        tint = chat.star,
                        modifier = Modifier.size(14.dp),
                    )
                }
                Text(
                    buildAnnotatedString {
                        if (hasDraft) {
                            withStyle(
                                SpanStyle(
                                    color = colors.error,
                                    fontWeight = FontWeight.Normal,
                                ),
                            ) {
                                append(draftBadge)
                                append(" ")
                            }
                        }
                        withStyle(
                            SpanStyle(
                                color = colors.onSurface,
                                fontWeight = if (unread) FontWeight.SemiBold else FontWeight.Medium,
                            ),
                        ) {
                            append(thread.subject.ifBlank { "(no subject)" })
                        }
                        if (thread.preview.isNotBlank()) {
                            withStyle(SpanStyle(color = colors.onSurfaceVariant, fontWeight = FontWeight.Normal)) {
                                append(" — ${thread.preview}")
                            }
                        }
                    },
                    fontSize = 12.sp,
                    lineHeight = 17.sp,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
            }
            MailRowTagChips(thread.tags, Modifier.padding(top = 4.dp))
            if (thread.files.isNotEmpty()) {
                MailRowAttachmentChips(thread.files, Modifier.padding(top = 4.dp))
            }
        }
    }
}

@Composable
private fun ContactNotes(
    state: ContactsScreenState,
    addr: String,
) {
    val note = state.note(addr)
    val scope = rememberCoroutineScope()
    val colors = MaterialTheme.colorScheme
    var preview by remember(addr) { mutableStateOf(false) }
    val noteFocusRequester = remember { FocusRequester() }
    val editorOpen = note.editing || note.dirty
    val showPreview = preview && note.text.isNotBlank()
    LaunchedEffect(note.editing, showPreview) {
        if (note.editing && !showPreview) {
            runCatching { noteFocusRequester.requestFocus() }
        }
    }
    Column(
        Modifier
            .fillMaxWidth()
            .padding(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = 28.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                tr("contacts.notes"),
                style = MaterialTheme.typography.labelMedium,
                fontWeight = FontWeight.SemiBold,
                color = colors.onSurfaceVariant,
                modifier = Modifier.weight(1f),
            )
            if (note.loaded && !editorOpen && note.text.isNotEmpty()) {
                Row(
                    Modifier
                        .clip(RoundedCornerShape(6.dp))
                        .clickable {
                            note.editing = true
                            preview = false
                        }.padding(horizontal = 8.dp, vertical = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    Icon(
                        Icons.Filled.Edit,
                        contentDescription = null,
                        tint = colors.onSurfaceVariant,
                        modifier = Modifier.size(13.dp),
                    )
                    Text(
                        tr("contacts.notesEdit"),
                        style = MaterialTheme.typography.labelSmall,
                        fontWeight = FontWeight.SemiBold,
                        color = colors.onSurfaceVariant,
                    )
                }
            }
        }
        when {
            !note.loaded && note.error -> {
                ContactFailure { scope.launch { state.loadNote(addr) } }
            }

            !note.loaded -> {
                ContactLoading()
            }

            editorOpen -> {
                // No WebView/HTML, image fetching or automatic links in private notes.
                Surface(
                    shape = RoundedCornerShape(12.dp),
                    color = colors.surface,
                    border = BorderStroke(1.dp, colors.outlineVariant),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Column(Modifier.fillMaxWidth()) {
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 8.dp, vertical = 6.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(4.dp),
                        ) {
                            ContactNoteTabButton(
                                label = tr("contacts.notesWrite"),
                                active = !showPreview,
                                enabled = true,
                                onClick = { preview = false },
                            )
                            ContactNoteTabButton(
                                label = tr("contacts.notesPreview"),
                                active = showPreview,
                                enabled = note.text.isNotBlank(),
                                onClick = { preview = true },
                            )
                            Spacer(Modifier.weight(1f))
                            Icon(
                                MarkdownMarkIcon,
                                contentDescription = tr("contacts.markdownHint"),
                                tint = colors.onSurfaceVariant.copy(alpha = 0.7f),
                                modifier = Modifier.padding(end = 4.dp).size(18.dp),
                            )
                        }
                        HorizontalDivider(color = colors.outlineVariant.copy(alpha = 0.5f))
                        if (showPreview) {
                            Box(
                                Modifier
                                    .fillMaxWidth()
                                    .heightIn(min = 104.dp)
                                    .padding(12.dp),
                            ) {
                                ContactMarkdown(note.text)
                            }
                        } else {
                            val notesLabel = tr("contacts.notes")
                            BasicTextField(
                                value = note.text,
                                onValueChange = { note.text = it },
                                cursorBrush = SolidColor(colors.primary),
                                textStyle =
                                    MaterialTheme.typography.bodyMedium.copy(
                                        color = colors.onSurface,
                                        lineHeight = 20.sp,
                                    ),
                                modifier =
                                    Modifier
                                        .fillMaxWidth()
                                        .heightIn(min = 104.dp, max = 240.dp)
                                        .padding(12.dp)
                                        .focusRequester(noteFocusRequester)
                                        .semantics { contentDescription = notesLabel },
                                decorationBox = { innerTextField ->
                                    Box(Modifier.fillMaxWidth()) {
                                        if (note.text.isEmpty()) {
                                            Text(
                                                tr("contacts.notesPlaceholder"),
                                                style = MaterialTheme.typography.bodyMedium,
                                                color = colors.onSurfaceVariant.copy(alpha = 0.7f),
                                            )
                                        }
                                        innerTextField()
                                    }
                                },
                            )
                        }
                        HorizontalDivider(color = colors.outlineVariant.copy(alpha = 0.5f))
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .background(colors.surfaceContainer.copy(alpha = 0.5f))
                                .padding(horizontal = 8.dp, vertical = 6.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.End),
                        ) {
                            TextButton(
                                onClick = {
                                    note.text = note.saved
                                    note.editing = false
                                    note.error = false
                                    preview = false
                                },
                                enabled = !note.saving,
                                contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp),
                                modifier = Modifier.height(32.dp),
                            ) {
                                Text(tr("buttons.cancel"), style = MaterialTheme.typography.labelMedium)
                            }
                            Button(
                                onClick = { scope.launch { state.saveNote(addr) } },
                                enabled = note.dirty && !note.saving,
                                shape = RoundedCornerShape(8.dp),
                                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 4.dp),
                                colors = ButtonDefaults.buttonColors(containerColor = colors.primary, contentColor = colors.onPrimary),
                                modifier = Modifier.height(32.dp),
                            ) {
                                Text(
                                    tr(if (note.saving) "contacts.notesSaving" else "contacts.notesSave"),
                                    style = MaterialTheme.typography.labelMedium,
                                )
                            }
                        }
                    }
                }
            }

            note.text.isNotEmpty() -> {
                Surface(
                    shape = RoundedCornerShape(10.dp),
                    color = colors.surfaceContainer.copy(alpha = 0.55f),
                    border = BorderStroke(1.dp, colors.outlineVariant.copy(alpha = 0.5f)),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Box(Modifier.fillMaxWidth().padding(12.dp)) {
                        ContactMarkdown(note.text)
                    }
                }
            }

            else -> {
                Surface(
                    onClick = {
                        note.editing = true
                        preview = false
                    },
                    shape = RoundedCornerShape(10.dp),
                    color = colors.surfaceContainer.copy(alpha = 0.4f),
                    border = BorderStroke(1.dp, colors.outlineVariant.copy(alpha = 0.7f)),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 12.dp, vertical = 10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Icon(
                            Icons.Filled.Add,
                            contentDescription = null,
                            tint = colors.onSurfaceVariant,
                            modifier = Modifier.size(16.dp),
                        )
                        Text(
                            tr("contacts.notesPlaceholder"),
                            style = MaterialTheme.typography.bodyMedium,
                            color = colors.onSurfaceVariant,
                        )
                    }
                }
            }
        }
        if (note.loaded && note.error) {
            Text(
                tr("contacts.notesSaveError"),
                style = MaterialTheme.typography.bodySmall,
                color = colors.error,
            )
        }
    }
}

@Composable
private fun ContactNoteTabButton(
    label: String,
    active: Boolean,
    enabled: Boolean,
    onClick: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    Box(
        Modifier
            .clip(RoundedCornerShape(6.dp))
            .background(if (active) colors.surfaceContainerHigh else colors.surface)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 10.dp, vertical = 5.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            label,
            style = MaterialTheme.typography.labelMedium,
            fontWeight = if (active) FontWeight.SemiBold else FontWeight.Medium,
            color =
                when {
                    !enabled -> colors.onSurfaceVariant.copy(alpha = 0.4f)
                    active -> colors.onSurface
                    else -> colors.onSurfaceVariant
                },
        )
    }
}

@Composable
private fun ContactSectionTitle(text: String) {
    Text(
        text,
        Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 4.dp),
        style = MaterialTheme.typography.labelMedium,
        fontWeight = FontWeight.SemiBold,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

/** The Markdown mark (CC0, dcurtis/markdown-mark), matching desktop's MarkdownIcon. */
private val MarkdownMarkIcon: ImageVector by lazy {
    ImageVector
        .Builder(
            name = "MarkdownMark",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            val stroke = SolidColor(androidx.compose.ui.graphics.Color.Black)
            path(
                stroke = stroke,
                strokeLineWidth = 2f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                // Rounded rect x=2, y=5, width=20, height=14, rx=2
                moveTo(4f, 5f)
                lineTo(20f, 5f)
                quadTo(22f, 5f, 22f, 7f)
                lineTo(22f, 17f)
                quadTo(22f, 19f, 20f, 19f)
                lineTo(4f, 19f)
                quadTo(2f, 19f, 2f, 17f)
                lineTo(2f, 7f)
                quadTo(2f, 5f, 4f, 5f)
                close()
                // "M": M6 15V9l3 3 3-3v6
                moveTo(6f, 15f)
                lineTo(6f, 9f)
                lineTo(9f, 12f)
                lineTo(12f, 9f)
                lineTo(12f, 15f)
                // Down arrow: M17 9v6m-2-2 2 2 2-2
                moveTo(17f, 9f)
                lineTo(17f, 15f)
                moveTo(15f, 13f)
                lineTo(17f, 15f)
                lineTo(19f, 13f)
            }
        }.build()
}

@Composable
private fun ContactLoading() {
    Box(Modifier.fillMaxWidth().padding(20.dp), contentAlignment = Alignment.Center) {
        CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.5.dp)
    }
}

@Composable
private fun ContactFailure(onRetry: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp)
            .clip(RoundedCornerShape(10.dp))
            .background(colors.errorContainer.copy(alpha = 0.5f))
            .clickable(onClick = onRetry)
            .padding(horizontal = 12.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(
            Icons.Filled.ErrorOutline,
            contentDescription = null,
            tint = colors.error,
            modifier = Modifier.size(18.dp),
        )
        Text(
            tr("contacts.retry"),
            style = MaterialTheme.typography.bodySmall,
            fontWeight = FontWeight.Medium,
            color = colors.error,
        )
    }
}
