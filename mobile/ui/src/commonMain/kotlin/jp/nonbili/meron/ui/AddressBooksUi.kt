package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlin.uuid.ExperimentalUuidApi
import kotlin.uuid.Uuid

/*
 * Address books synced over CardDAV (or Google Contacts), and adding, editing
 * and deleting the contacts in them. Mirrors desktop's AddressBooks and
 * ContactEditor.
 */

/** The address book form, with what discovery found. */
private data class ServerForm(
    val server: String,
    val email: String,
    val discovered: Boolean,
)

/** Manage address books: connect a server (found from a mail account, or
 *  entered), connect Google Contacts, pick which books sync, sync, disconnect.
 *  [onChange] tells the contacts list to read again. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun AddressBooksPage(
    state: MeronMobileState,
    onChange: () -> Unit,
    modifier: Modifier = Modifier,
) = with(state) {
    val scope = rememberCoroutineScope()
    var accounts by remember { mutableStateOf(emptyList<AddressBookAccount>()) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf("") }
    var form by remember { mutableStateOf<ServerForm?>(null) }
    var picking by remember { mutableStateOf(false) }
    var disconnecting by remember { mutableStateOf<AddressBookAccount?>(null) }
    var googleEmailOpen by remember { mutableStateOf(false) }
    // Bumped to abandon a running step: its late answer still reloads the
    // list but no longer holds the page.
    var attempt by remember { mutableStateOf(0) }
    val failed = tr("contacts.addressBooksFailed")

    suspend fun reload() {
        runCatching { accounts = parseAddressBookAccounts(coreCall("carddav.accounts")) }
            .onFailure { if (it is CancellationException) throw it else error = it.message.orEmpty() }
    }
    LaunchedEffect(addressBooksVersion) { reload() }

    fun run(work: suspend () -> Unit) {
        if (busy) return
        val current = ++attempt
        busy = true
        error = ""
        scope.launch {
            try {
                work()
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                if (current == attempt) error = e.message ?: failed
            } finally {
                onChange()
                reload()
                if (current == attempt) busy = false
            }
        }
    }

    val mailboxes = davMailboxes().filter { mailbox -> accounts.none { it.username.equals(mailbox.email.trim(), ignoreCase = true) } }

    fun find(mailbox: jp.nonbili.meron.shared.AccountSummary) {
        if (isGoogleMailbox(mailbox)) {
            scope.launch { connectGoogleService(GoogleService.Contacts, mailbox.email) }
            return
        }
        run {
            val server =
                runCatching { coreCall("carddav.discoverServer", buildJsonObject { put("email", mailbox.email) }).text("server_url") }
                    .getOrElse { if (it is CancellationException) throw it else "" }
            form = ServerForm(server, mailbox.email, server.isNotBlank())
        }
    }

    LazyColumn(modifier, contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp + navigationBarInset())) {
        item {
            Text(
                tr("contacts.addressBooksDescription"),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(bottom = 12.dp),
            )
        }
        if (googleSignIn?.service == GoogleService.Contacts) {
            item {
                Row(Modifier.fillMaxWidth().padding(vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                    Text(
                        tr("accounts.oauth.waitingForSignIn", mapOf("provider" to "Google")),
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.weight(1f).padding(horizontal = 12.dp),
                    )
                    TextButton(onClick = ::cancelGoogleServiceSignIn) { Text(tr("buttons.cancel")) }
                }
            }
        }
        val failure = error.ifBlank { googleSignInError }
        if (failure.isNotBlank()) {
            item { Text(failure, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(vertical = 8.dp)) }
        }
        items(accounts, key = { it.id }) { account ->
            Surface(
                shape = RoundedCornerShape(16.dp),
                color = MaterialTheme.colorScheme.surfaceContainerLow,
                modifier = Modifier.fillMaxWidth().padding(bottom = 8.dp),
            ) {
                Column {
                    Column(Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
                        Text(account.username, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(
                            if (account.google) "Google Contacts" else account.server,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))
                    if (account.books.isEmpty()) {
                        Text(tr("contacts.addressBooksNoBooks"), style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(16.dp))
                    }
                    account.books.forEach { book ->
                        Row(
                            Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(start = 16.dp, end = 12.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(Modifier.weight(1f)) {
                                Text(
                                    book.name,
                                    color = if (book.enabled) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                                if (book.skipped > 0) {
                                    Text(
                                        tr("contacts.addressBooksSkipped", mapOf("count" to book.skipped)),
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            }
                            Switch(
                                checked = book.enabled,
                                enabled = !busy,
                                colors = settingsSwitchColors(),
                                onCheckedChange = { enabled ->
                                    run {
                                        coreCall(
                                            "carddav.setBookEnabled",
                                            buildJsonObject {
                                                put("book_id", book.id)
                                                put("enabled", enabled)
                                            },
                                        )
                                        if (enabled) coreCall("carddav.sync", buildJsonObject { put("account_id", account.id) })
                                    }
                                },
                            )
                        }
                    }
                    HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))
                    FlowRow(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        TextButton(enabled = !busy, onClick = { run { coreCall("carddav.sync", buildJsonObject { put("account_id", account.id) }) } }) {
                            Icon(Icons.Filled.Refresh, contentDescription = null, modifier = Modifier.size(18.dp))
                            Spacer(Modifier.width(6.dp))
                            Text(tr("settings.calendars.syncNow"))
                        }
                        if (account.google && googleSignInConfigured) {
                            TextButton(enabled = !busy, onClick = { scope.launch { connectGoogleService(GoogleService.Contacts, account.username, account.id) } }) {
                                Text(tr("contacts.addressBooksReconnectGoogle"))
                            }
                        }
                        TextButton(enabled = !busy, onClick = { disconnecting = account }) {
                            Text(tr("contacts.addressBooksDisconnect"), color = MaterialTheme.colorScheme.error)
                        }
                    }
                }
            }
        }
        item {
            Text(
                tr("contacts.addressBooksAdd"),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 16.dp, bottom = 8.dp),
            )
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                if (mailboxes.isNotEmpty()) {
                    Button(
                        enabled = !busy,
                        onClick = { if (mailboxes.size == 1) find(mailboxes.single()) else picking = true },
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Icon(Icons.Filled.Search, contentDescription = null, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.width(8.dp))
                        Text(tr(if (busy) "settings.calendars.connecting" else "contacts.addressBooksFind"))
                    }
                } else {
                    Text(tr("contacts.addressBooksNoMailbox"), style = MaterialTheme.typography.bodySmall)
                }
                if (googleSignInConfigured) {
                    OutlinedButton(
                        enabled = !busy && googleSignIn == null,
                        onClick = { googleEmailOpen = true },
                        modifier = Modifier.fillMaxWidth(),
                    ) { Text(tr("accounts.oauth.signInWith", mapOf("provider" to "Google"))) }
                }
                TextButton(enabled = !busy, onClick = { form = ServerForm("", "", false) }) {
                    Icon(Icons.Filled.Add, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                    Text(tr("contacts.addressBooksManual"))
                }
            }
        }
    }

    if (picking) {
        MailboxPickerDialog(
            title = tr("contacts.addressBooksFind"),
            mailboxes = mailboxes,
            onDismiss = { picking = false },
            onPick = {
                picking = false
                find(it)
            },
        )
    }
    if (googleEmailOpen) {
        EmailDialog(
            title = tr("accounts.oauth.signInWith", mapOf("provider" to "Google")),
            onDismiss = { googleEmailOpen = false },
            onSubmit = { email ->
                googleEmailOpen = false
                scope.launch { connectGoogleService(GoogleService.Contacts, email) }
            },
        )
    }
    form?.let { initial ->
        AddressBookServerDialog(
            state = state,
            initial = initial,
            busy = busy,
            error = error,
            // While connecting, stop and keep the form to fix it; then close.
            onCancel = {
                if (busy) {
                    attempt++
                    busy = false
                } else {
                    form = null
                    error = ""
                }
            },
            onSubmit = { server, username, password, mailAccountId ->
                run {
                    // One cancelled while it ran must not close the form a
                    // newer attempt is using.
                    val mine = attempt
                    val added =
                        coreCall(
                            "carddav.add",
                            buildJsonObject {
                                put("server_url", server)
                                put("username", username)
                                put("password", password)
                                if (mailAccountId != null) put("mail_account_id", mailAccountId)
                            },
                        )
                    coreCall("carddav.sync", buildJsonObject { put("account_id", added.text("account_id")) })
                    if (mine == attempt) form = null
                }
            },
        )
    }
    disconnecting?.let { account ->
        AlertDialog(
            onDismissRequest = { disconnecting = null },
            title = { Text(tr("contacts.addressBooksDisconnect")) },
            text = { Text(tr("contacts.addressBooksDisconnectConfirm", mapOf("name" to account.username))) },
            confirmButton = {
                TextButton(onClick = {
                    disconnecting = null
                    run { coreCall("carddav.remove", buildJsonObject { put("account_id", account.id) }) }
                }) { Text(tr("contacts.addressBooksDisconnect"), color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { disconnecting = null }) { Text(tr("buttons.cancel")) } },
        )
    }
}

@Composable
private fun AddressBookServerDialog(
    state: MeronMobileState,
    initial: ServerForm,
    busy: Boolean,
    error: String,
    onCancel: () -> Unit,
    onSubmit: (server: String, username: String, password: String, mailAccountId: String?) -> Unit,
) {
    var server by remember(initial) { mutableStateOf(initial.server) }
    var username by remember(initial) { mutableStateOf(initial.email) }
    var password by remember(initial) { mutableStateOf("") }
    // A mail account on the same login usually opens the address book too, so
    // its password (which stays in the core) can stand in for a blank one.
    val mailAccount = state.mailAccountForLogin(username)
    val canSubmit = server.isNotBlank() && username.isNotBlank() && (password.isNotEmpty() || mailAccount != null) && !busy
    AlertDialog(
        onDismissRequest = onCancel,
        title = { Text(tr("contacts.addressBooksAdd")) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                if (initial.email.isNotBlank()) {
                    Text(tr(if (initial.discovered) "contacts.addressBooksFound" else "contacts.addressBooksNotFound"), style = MaterialTheme.typography.bodySmall)
                }
                OutlinedTextField(
                    value = server,
                    onValueChange = { server = it },
                    label = { Text(tr("contacts.addressBooksServer")) },
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
                if (error.isNotBlank()) Text(error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
            }
        },
        confirmButton = {
            Button(enabled = canSubmit, onClick = {
                onSubmit(server.trim(), username.trim(), password, if (password.isEmpty()) mailAccount?.id else null)
            }) { Text(tr(if (busy) "settings.calendars.connecting" else "contacts.addressBooksConnect")) }
        },
        dismissButton = { TextButton(onClick = onCancel) { Text(tr("buttons.cancel")) } },
    )
}

/** What the contact editor was opened for. */
internal data class ContactEditorTarget(
    val card: BookCard? = null,
    val name: String = "",
    val email: String = "",
    val note: String? = null,
    /** The note is the one kept on this device: drop that copy once saved. */
    val movesLocalNote: Boolean = false,
    /** Where a new contact goes unless another book is picked. */
    val bookId: String = "",
)

/** What the editor saved, or null when it deleted the contact. */
internal data class ContactEditorResult(
    val name: String,
    val emails: List<String>,
    val note: String,
)

/** Create a contact in an address book, or edit or delete one, on its server. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalUuidApi::class)
@Composable
internal fun ContactEditorSheet(
    contacts: ContactsScreenState,
    target: ContactEditorTarget,
    onDone: (ContactEditorResult?) -> Unit,
    onDismiss: () -> Unit,
    onRefreshWarning: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    val card = target.card
    val books = contacts.books
    var name by remember { mutableStateOf(card?.name ?: target.name) }
    var emails by remember { mutableStateOf(card?.emails?.takeIf { it.isNotEmpty() } ?: listOf(target.email)) }
    var note by remember { mutableStateOf(target.note ?: card?.note.orEmpty()) }
    var bookId by remember { mutableStateOf(card?.bookId ?: books.firstOrNull { it.id == target.bookId }?.id ?: books.firstOrNull()?.id.orEmpty()) }
    // One per new contact, so a retried create cannot make a second card.
    val creationId = remember { if (card == null) Uuid.random().toString() else "" }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf("") }
    var bookMenu by remember { mutableStateOf(false) }
    var confirmDelete by remember { mutableStateOf(false) }
    val failed = tr("contacts.editorFailed")
    val cleanEmails = emails.map { it.trim() }.filter { it.isNotEmpty() }
    // Contacts are reached by address here, so one without any would vanish.
    val canSave = !busy && bookId.isNotBlank() && cleanEmails.isNotEmpty() && (name.isNotBlank() || (card != null && card.name.isBlank()))

    fun send(
        method: String,
        build: kotlinx.serialization.json.JsonObjectBuilder.() -> Unit,
        result: ContactEditorResult?,
    ) {
        busy = true
        error = ""
        scope.launch {
            runCatching {
                contacts.mutateCard(
                    method,
                    buildJsonObject {
                        put("book_id", bookId)
                        build()
                    },
                )
            }.onSuccess { answer ->
                busy = false
                if (answer.text("sync_error").isNotBlank()) onRefreshWarning()
                onDone(result)
            }.onFailure {
                if (it is CancellationException) throw it
                busy = false
                error = it.message ?: failed
            }
        }
    }

    ModalBottomSheet(
        onDismissRequest = { if (!busy) onDismiss() },
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = MaterialTheme.colorScheme.surface,
    ) {
        Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp)
                .padding(bottom = 16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(tr(if (card == null) "contacts.newContact" else "contacts.editContact"), style = MaterialTheme.typography.titleLarge)
            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                label = { Text(tr("contacts.editorName")) },
                singleLine = true,
                enabled = !busy,
                modifier = Modifier.fillMaxWidth(),
            )
            Text(tr("contacts.editorEmails"), style = MaterialTheme.typography.labelMedium)
            emails.forEachIndexed { index, email ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    OutlinedTextField(
                        value = email,
                        onValueChange = { value -> emails = emails.mapIndexed { i, old -> if (i == index) value else old } },
                        placeholder = { Text("name@example.com") },
                        singleLine = true,
                        enabled = !busy,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email, autoCorrectEnabled = false),
                        modifier = Modifier.weight(1f),
                    )
                    if (emails.size > 1) {
                        IconButton(onClick = { emails = emails.filterIndexed { i, _ -> i != index } }) {
                            Icon(Icons.Filled.Close, contentDescription = tr("contacts.editorRemoveEmail"))
                        }
                    }
                }
            }
            TextButton(enabled = !busy, onClick = { emails = emails + "" }) {
                Icon(Icons.Filled.Add, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text(tr("contacts.editorAddEmail"))
            }
            OutlinedTextField(
                value = note,
                onValueChange = { note = it },
                label = { Text(tr("contacts.editorNote")) },
                placeholder = { Text(tr("contacts.notesPlaceholder")) },
                minLines = 3,
                maxLines = 10,
                enabled = !busy,
                supportingText = { Text(tr("contacts.editorNoteHint")) },
                modifier = Modifier.fillMaxWidth(),
            )
            if (card != null) {
                Text(
                    tr("contacts.editorSavedIn", mapOf("book" to bookLabel(card.bookName, card.account))),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else if (books.size > 1) {
                Column {
                    Text(tr("contacts.editorBook"), style = MaterialTheme.typography.labelMedium)
                    TextButton(onClick = { bookMenu = true }) {
                        Text(books.firstOrNull { it.id == bookId }?.let { bookLabel(it.name, it.account) }.orEmpty())
                    }
                    DropdownMenu(expanded = bookMenu, onDismissRequest = { bookMenu = false }) {
                        books.forEach { book ->
                            DropdownMenuItem(
                                text = { Text(bookLabel(book.name, book.account)) },
                                onClick = {
                                    bookMenu = false
                                    bookId = book.id
                                },
                            )
                        }
                    }
                }
            }
            if (error.isNotBlank()) Text(error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (card != null) {
                    IconButton(enabled = !busy, onClick = { confirmDelete = true }) {
                        Icon(Icons.Filled.Delete, contentDescription = tr("contacts.editorDelete"), tint = MaterialTheme.colorScheme.error)
                    }
                }
                Spacer(Modifier.weight(1f))
                TextButton(enabled = !busy, onClick = onDismiss) { Text(tr("buttons.cancel")) }
                Button(
                    enabled = canSave,
                    onClick = {
                        val result = ContactEditorResult(name.trim(), cleanEmails, note.trim())
                        if (card != null) {
                            send("carddav.updateContact", {
                                put("contact_id", card.id)
                                put("version", card.version)
                                put("name", name.trim())
                                put("emails", JsonArray(cleanEmails.map(::JsonPrimitive)))
                                // Only a changed note is sent: the server's NOTE
                                // lines stay exactly as they are otherwise.
                                if (note.trim() != card.note.trim()) put("note", note.trim())
                            }, result)
                        } else {
                            send("carddav.createContact", {
                                put("creation_id", creationId)
                                put("name", name.trim())
                                put("emails", JsonArray(cleanEmails.map(::JsonPrimitive)))
                                put("note", note.trim())
                            }, result)
                        }
                    },
                ) { Text(tr(if (busy) "contacts.editorSaving" else "buttons.save")) }
            }
        }
    }

    if (confirmDelete && card != null) {
        AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text(tr("contacts.editorDelete")) },
            text = { Text(tr("contacts.editorDeleteConfirm", mapOf("name" to card.name.ifBlank { card.emails.firstOrNull().orEmpty() }, "book" to card.bookName))) },
            confirmButton = {
                TextButton(onClick = {
                    confirmDelete = false
                    send("carddav.deleteContact", {
                        put("contact_id", card.id)
                        put("version", card.version)
                    }, null)
                }) { Text(tr("contacts.editorDelete"), color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { confirmDelete = false }) { Text(tr("buttons.cancel")) } },
        )
    }
}
