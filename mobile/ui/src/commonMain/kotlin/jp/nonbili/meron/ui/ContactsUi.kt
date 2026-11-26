package jp.nonbili.meron.ui

import androidx.compose.animation.core.animate
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.gestures.rememberDraggableState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Sort
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Contacts
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.PersonAdd
import androidx.compose.material.icons.filled.PushPin
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.outlined.Email
import androidx.compose.material.icons.outlined.PushPin
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DrawerState
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
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
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.ContactSuggestion
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.formatContactSuggestion
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.put
import kotlin.math.roundToInt

@Composable
internal fun rememberContactsScreenState(core: MeronCore): ContactsScreenState = remember(core) { ContactsScreenState(core) }

/** Local directory; it never changes the mailbox's current query or folder. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalComposeUiApi::class, ExperimentalLayoutApi::class)
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
    /** The app, for managing address books (which signs in to Google and
     *  finds servers from mail accounts); without it they are not offered. */
    mobile: MeronMobileState? = null,
    /** The app's drawer, offered from the list; without it the list goes back. */
    drawerState: DrawerState? = null,
    drawerContent: (@Composable () -> Unit)? = null,
) {
    val scope = rememberCoroutineScope()
    val query = state.query
    // Address books: which one the list shows, the editor and the page that
    // manages them. Bumped to read the books, lists and cards again.
    var bookRefresh by remember { mutableStateOf(0) }
    var managingBooks by remember { mutableStateOf(false) }
    var bookMenu by remember { mutableStateOf(false) }
    var editor by remember { mutableStateOf<ContactEditorTarget?>(null) }
    val shownBook = state.shownBook
    val book = state.books.firstOrNull { it.id == shownBook }
    val otherOnly = shownBook == OTHER_CONTACTS
    val refreshWarning = tr("contacts.editorRefreshWarning")
    val snackbar = mobile?.snackbarHost
    val sort = state.sort
    val limit = state.limit
    var retry by remember { mutableStateOf(0) }
    var sortOpen by remember { mutableStateOf(false) }
    val selected = state.selected
    val goBack = {
        when {
            managingBooks -> managingBooks = false
            state.selected != null -> state.select(null)
            else -> onBack()
        }
    }
    val colors = MaterialTheme.colorScheme
    BackHandler(enabled = active && (selected != null || managingBooks)) { goBack() }
    LaunchedEffect(mobile?.addressBooksVersion, bookRefresh) { state.loadBooks() }
    LaunchedEffect(core) { state.loadPins() }
    LaunchedEffect(initialContact) {
        if (initialContact != null) {
            if (contactAddressKey(initialContact.addr) != state.selected?.addr) state.select(initialContact)
            onInitialContactConsumed()
        }
    }
    LaunchedEffect(query, sort, limit, retry, otherOnly, mobile?.addressBooksVersion, bookRefresh) {
        delay(150)
        state.loadDirectory(query, sort, limit, unsaved = otherOnly)
    }
    // An address book lists its own cards, by name.
    LaunchedEffect(book?.id, query, mobile?.addressBooksVersion, bookRefresh) {
        if (book == null) return@LaunchedEffect
        delay(150)
        state.loadBookCards(query)
    }
    LaunchedEffect(selected?.addr) {
        if (selected != null) {
            launch { state.loadHistory() }
            launch { state.loadNote(selected.addr) }
        }
    }
    LaunchedEffect(selected?.addr, mobile?.addressBooksVersion, bookRefresh) {
        if (selected != null) state.loadCards(selected.addr)
    }
    // The list is a top-level view like Tasks and Calendar, so it offers
    // the drawer; a contact and the address books go back to it.
    val listShown = selected == null && !managingBooks
    ContactsDrawer(drawerState, drawerContent, gesturesEnabled = listShown) {
        Scaffold(
            containerColor = colors.background,
            // Saving to an address book reports here.
            snackbarHost = { snackbar?.let { SnackbarHost(it) } },
            topBar = {
                Column(Modifier.background(colors.background)) {
                    TopAppBar(
                        title = {
                            when {
                                managingBooks -> {
                                    Text(tr("contacts.addressBooks"), fontSize = 18.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                }

                                selected != null || (state.books.isEmpty() && mobile == null) -> {
                                    Text(tr("contacts.title"), fontSize = 18.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                }

                                else -> {
                                    // The title switches address books, as desktop's does.
                                    Box {
                                        Row(
                                            Modifier.clickable { bookMenu = true },
                                            verticalAlignment = Alignment.CenterVertically,
                                        ) {
                                            Text(
                                                when {
                                                    book != null -> bookLabel(book.name, book.account)
                                                    otherOnly -> tr("contacts.otherContacts")
                                                    state.books.isNotEmpty() -> tr("contacts.allContacts")
                                                    else -> tr("contacts.title")
                                                },
                                                fontSize = 18.sp,
                                                maxLines = 1,
                                                overflow = TextOverflow.Ellipsis,
                                                modifier = Modifier.weight(1f, fill = false),
                                            )
                                            Icon(Icons.Filled.ArrowDropDown, contentDescription = tr("contacts.showBook"))
                                        }
                                        DropdownMenu(expanded = bookMenu, onDismissRequest = { bookMenu = false }) {
                                            val options =
                                                buildList {
                                                    add("" to tr("contacts.allContacts"))
                                                    state.books.forEach { add(it.id to bookLabel(it.name, it.account)) }
                                                    // With no address book every contact is an other one.
                                                    if (state.books.isNotEmpty()) add(OTHER_CONTACTS to tr("contacts.otherContacts"))
                                                }
                                            options.forEach { (id, label) ->
                                                DropdownMenuItem(
                                                    text = { Text(label) },
                                                    trailingIcon = { if (id == shownBook) Icon(Icons.Filled.Check, contentDescription = null) },
                                                    onClick = {
                                                        bookMenu = false
                                                        scope.launch { state.chooseBook(id) }
                                                    },
                                                )
                                            }
                                            if (mobile != null) {
                                                HorizontalDivider()
                                                DropdownMenuItem(
                                                    text = { Text(tr("contacts.manageAddressBooks")) },
                                                    onClick = {
                                                        bookMenu = false
                                                        managingBooks = true
                                                    },
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        colors = canvasAppBarColors(),
                        navigationIcon = {
                            if (listShown && drawerState != null) {
                                IconButton(onClick = { scope.launch { drawerState.open() } }) {
                                    Icon(Icons.Filled.Menu, contentDescription = tr("mobile.actions.openNavigation"))
                                }
                            } else {
                                IconButton(onClick = goBack) {
                                    Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = tr("buttons.back"))
                                }
                            }
                        },
                        actions = {
                            if (selected == null && !managingBooks && state.books.isNotEmpty()) {
                                IconButton(onClick = { editor = ContactEditorTarget(bookId = book?.id.orEmpty()) }) {
                                    Icon(Icons.Filled.PersonAdd, contentDescription = tr("contacts.newContact"))
                                }
                            }
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
                    if (selected == null && !managingBooks) {
                        ContactsSearchBar(
                            query = query,
                            onQueryChange = {
                                state.query = it
                                state.limit = 100
                            },
                            sort = sort,
                            sortOpen = sortOpen,
                            onSortOpenChange = { sortOpen = it },
                            // An address book lists by name.
                            sortEnabled = state.pinsLoaded && !state.sortSaving && book == null,
                            onSelectSort = { option ->
                                scope.launch { state.setSort(option) }
                                sortOpen = false
                            },
                        )
                    }
                }
            },
        ) { padding ->
            Column(
                Modifier
                    .fillMaxSize()
                    // Both lists scroll on under the navigation bar.
                    .padding(padding.withoutBottom())
                    .consumeWindowInsets(padding.withoutBottom())
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
                if (managingBooks && mobile != null) {
                    AddressBooksPage(mobile, onChange = { bookRefresh++ }, modifier = Modifier.fillMaxSize())
                } else if (selected == null && book != null) {
                    BookCardList(
                        state = state,
                        query = query,
                        onSelect = { card, addr -> state.select(ContactSuggestion(card.name, addr)) },
                        onCompose = if (canCompose) onCompose else null,
                        onRetry = { bookRefresh++ },
                        onMore = { scope.launch { state.loadBookCards(query, more = true) } },
                    )
                } else if (selected == null) {
                    val matchingPins =
                        (if (otherOnly) emptyList() else state.pins)
                            .filter {
                                "${it.name} ${it.addr}".contains(query.trim(), ignoreCase = true)
                            }.sortedBy { it.name.ifBlank { it.addr }.lowercase() }
                    val others = if (otherOnly) state.contacts else state.contacts.filterNot(state::isPinned)
                    val directoryListState = rememberLazyListState()
                    LazyColumn(
                        Modifier.fillMaxSize().appScrollbar(directoryListState),
                        state = directoryListState,
                        contentPadding = PaddingValues(top = 6.dp, bottom = 6.dp + navigationBarInset()),
                    ) {
                        if (matchingPins.isNotEmpty()) {
                            item(key = "section:pinned") { ContactSectionTitle(tr("contacts.pinned")) }
                            itemsIndexed(matchingPins, key = { _, it -> "pin:${contactAddressKey(it.addr)}" }) { index, contact ->
                                ContactDirectoryRow(
                                    state = state,
                                    contact = contact,
                                    shape = groupedRowShape(index, matchingPins.size),
                                    showSenderImages = showSenderImages,
                                    onSelect = { state.select(contact) },
                                    onPin = { scope.launch { state.togglePin(contact) } },
                                )
                            }
                            if (others.isNotEmpty()) {
                                item(key = "section:other") { ContactSectionTitle(tr("contacts.other")) }
                            }
                        }
                        itemsIndexed(others, key = { _, it -> contactAddressKey(it.addr) }) { index, contact ->
                            ContactDirectoryRow(
                                state = state,
                                contact = contact,
                                shape = groupedRowShape(index, others.size),
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
                        onEditCard = { editor = ContactEditorTarget(card = it) },
                        onSaveToBook = { note ->
                            editor =
                                ContactEditorTarget(
                                    name = selected.name,
                                    email = selected.addr,
                                    note = note.ifBlank { null },
                                    movesLocalNote = note.isNotBlank(),
                                    bookId = book?.id.orEmpty(),
                                )
                        },
                        onMoveNote = { card, note ->
                            scope.launch {
                                runCatching {
                                    // Always written, version-checked: the copy here may
                                    // be behind the server, and a repeated note is better
                                    // than a lost one.
                                    val answer =
                                        state.mutateCard(
                                            "carddav.updateContact",
                                            kotlinx.serialization.json.buildJsonObject {
                                                put("book_id", card.bookId)
                                                put("contact_id", card.id)
                                                put("version", card.version)
                                                put("name", card.name)
                                                put("emails", kotlinx.serialization.json.JsonArray(card.emails.map(::JsonPrimitive)))
                                                put("note", if (card.note.isBlank()) note else "${card.note.trim()}\n\n$note")
                                            },
                                        )
                                    if (answer.text("sync_error").isNotBlank()) scope.launch { snackbar?.showSnackbar(refreshWarning) }
                                    state.clearNoteIfUnchanged(selected.addr, note)
                                }.onFailure {
                                    if (it is kotlinx.coroutines.CancellationException) throw it
                                    val reason = it.message.orEmpty()
                                    scope.launch { snackbar?.showSnackbar(reason) }
                                }
                                bookRefresh++
                            }
                        },
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
            editor?.let { target ->
                ContactEditorSheet(
                    contacts = state,
                    target = target,
                    onDismiss = { editor = null },
                    onRefreshWarning = { scope.launch { snackbar?.showSnackbar(refreshWarning) } },
                    onDone = { result ->
                        val editing = target.card
                        // The device's copy goes only once the card holds it: a note
                        // taken out in the editor stays here.
                        val moved = target.note?.trim().orEmpty()
                        if (result != null && target.movesLocalNote && moved.isNotBlank() && moved in result.note) {
                            scope.launch { state.clearNoteIfUnchanged(state.selected?.addr ?: target.email, moved) }
                        }
                        editor = null
                        bookRefresh++
                        // Deleted: stay, as the address may still be in mail. Saved:
                        // show the contact under an address it still has.
                        if (result != null) {
                            val addr = state.selected?.addr?.takeIf { editing != null || it.isNotBlank() } ?: result.emails.first()
                            if (result.emails.any { contactAddressKey(it) == contactAddressKey(addr) }) {
                                // The same address keeps its history; only the name changes.
                                if (state.selected?.addr == contactAddressKey(addr)) {
                                    state.rename(result.name)
                                } else {
                                    state.select(ContactSuggestion(result.name, contactAddressKey(addr)))
                                }
                            } else if (editing != null) {
                                state.select(null)
                            }
                        }
                    },
                )
            }
        }
    }
}

@Composable
private fun ContactsDrawer(
    drawerState: DrawerState?,
    drawerContent: (@Composable () -> Unit)?,
    gesturesEnabled: Boolean,
    content: @Composable () -> Unit,
) {
    if (drawerState == null || drawerContent == null) {
        content()
    } else {
        ModalNavigationDrawer(
            drawerState = drawerState,
            drawerContent = drawerContent,
            gesturesEnabled = gesturesEnabled || drawerState.isOpen,
            content = content,
        )
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
            .padding(start = ViewCardMargin, end = ViewCardMargin, bottom = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Surface(
            modifier = Modifier.weight(1f).height(42.dp),
            shape = RoundedCornerShape(12.dp),
            color = colors.surface,
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
                color = if (sortOpen) colors.secondaryContainer else colors.surface,
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
    shape: Shape,
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
            .padding(horizontal = ViewCardMargin, vertical = GroupedRowGap / 2)
            .clip(shape)
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

/** An address book's own list: its cards by name, a page at a time. */
@Composable
private fun BookCardList(
    state: ContactsScreenState,
    query: String,
    onSelect: (ListedCard, String) -> Unit,
    onCompose: ((String) -> Unit)?,
    onRetry: () -> Unit,
    onMore: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val listState = rememberLazyListState()
    val loaded = state.bookCardsFor?.first == state.shownBook
    LazyColumn(
        Modifier.fillMaxSize().appScrollbar(listState),
        state = listState,
        contentPadding = PaddingValues(top = 6.dp, bottom = 6.dp + navigationBarInset()),
    ) {
        when {
            state.bookError -> {
                item(key = "error") { ContactFailure(onRetry) }
            }

            !loaded -> {
                item(key = "loading") { ContactLoading() }
            }

            else -> {
                itemsIndexed(state.bookCards, key = { _, it -> it.id }) { index, card ->
                    val addr = card.emails.firstOrNull().orEmpty()
                    val displayName = card.name.ifBlank { addr }
                    // The directory's rows, so the two lists read alike.
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .padding(horizontal = ViewCardMargin, vertical = GroupedRowGap / 2)
                            .clip(groupedRowShape(index, state.bookCards.size))
                            .background(colors.surface)
                            .clickable(enabled = addr.isNotBlank()) { onSelect(card, contactAddressKey(addr)) }
                            .heightIn(min = 58.dp)
                            .padding(horizontal = 12.dp, vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        // Initials only: sender images look up people who wrote
                        // to the user, and a card may be someone who never has.
                        SenderAvatar(
                            formatContactSuggestion(ContactSuggestion(card.name, addr)),
                            enabled = false,
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
                            if (card.emails.isNotEmpty()) {
                                Text(
                                    card.emails.joinToString(", "),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = colors.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                        }
                        if (onCompose != null && addr.isNotBlank()) {
                            IconButton(onClick = { onCompose(if (card.name.isBlank()) addr else "${card.name} <$addr>") }) {
                                Icon(Icons.Outlined.Email, contentDescription = tr("composer.actions.newMessage"), modifier = Modifier.size(20.dp))
                            }
                        }
                    }
                }
                if (state.bookCards.isEmpty()) {
                    item(key = "empty") {
                        Text(
                            tr(if (query.isBlank()) "contacts.bookEmpty" else "contacts.empty"),
                            color = colors.onSurfaceVariant,
                            modifier = Modifier.padding(24.dp),
                        )
                    }
                }
                // Only for the rows on screen: a search still loading has none yet.
                if (state.bookMore && state.bookCardsFor?.second == query) {
                    item(key = "more") {
                        Box(Modifier.fillMaxWidth().padding(vertical = 12.dp), contentAlignment = Alignment.Center) {
                            OutlinedButton(onClick = onMore, enabled = !state.bookLoading) { Text(tr("threads.actions.loadMore")) }
                        }
                    }
                }
            }
        }
    }
}

@Composable
internal fun ContactSectionTitle(text: String) {
    Text(
        text,
        Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 4.dp),
        style = MaterialTheme.typography.labelMedium,
        fontWeight = FontWeight.SemiBold,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
internal fun ContactLoading() {
    Box(Modifier.fillMaxWidth().padding(20.dp), contentAlignment = Alignment.Center) {
        CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.5.dp)
    }
}

@Composable
internal fun ContactFailure(onRetry: () -> Unit) {
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
