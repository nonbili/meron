package jp.nonbili.meron.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import jp.nonbili.meron.shared.AppPrefsGetParams
import jp.nonbili.meron.shared.AppPrefsSetParams
import jp.nonbili.meron.shared.ContactSuggestParams
import jp.nonbili.meron.shared.ContactSuggestion
import jp.nonbili.meron.shared.ContactThreadsParams
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.MobileCommand
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.parseContactSuggestResponse
import jp.nonbili.meron.shared.parseContactThreadsPage
import jp.nonbili.meron.shared.requireCoreOk
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put

internal fun contactAddressKey(addr: String): String = addr.trim().lowercase()

/** [ContactsScreenState.book] for the correspondents no address book holds. */
internal const val OTHER_CONTACTS = ":other"

internal class ContactNoteDraft {
    var text by mutableStateOf("")
    var saved by mutableStateOf("")
    var loaded by mutableStateOf(false)
    var loading by mutableStateOf(false)
    var saving by mutableStateOf(false)
    var error by mutableStateOf(false)
    var editing by mutableStateOf(false)
    val dirty: Boolean get() = text != saved
}

/** Remember above NavHost to retain private drafts while another route is open. */
internal class ContactsScreenState(
    private val core: MeronCore,
) {
    var query by mutableStateOf("")
    var limit by mutableStateOf(100)
    var sort by mutableStateOf("frequent")
        private set
    var sortSaving by mutableStateOf(false)
    var sortError by mutableStateOf(false)
    private var failedSort: String? = null
    var contacts by mutableStateOf(emptyList<ContactSuggestion>())
    var pins by mutableStateOf(emptyList<ContactSuggestion>())
    var pinsLoaded by mutableStateOf(false)
    var pinsLoading by mutableStateOf(false)
    var pinsSaving by mutableStateOf(false)
    var pinsError by mutableStateOf(false)
    var directoryLoading by mutableStateOf(true)
    var directoryError by mutableStateOf(false)
    var selected by mutableStateOf<ContactSuggestion?>(null)
        private set
    var threads by mutableStateOf(emptyList<ThreadSummary>())
    var cursor by mutableStateOf("")
    var historyLoading by mutableStateOf(false)
    var historyLoaded by mutableStateOf(false)
    var historyError by mutableStateOf(false)

    /** Enabled address books (where a new contact can go), with the login of
     *  the account each is in. */
    var books by mutableStateOf(emptyList<AddressBook>())
    var booksLoaded by mutableStateOf(false)

    /** The address book the list shows: a book's id, [OTHER_CONTACTS] for the
     *  correspondents no address book holds, or empty for everyone. A book
     *  since disabled or removed shows everyone again, without forgetting the
     *  choice before books load. */
    var book by mutableStateOf("")
        private set

    /** The chosen book's rows for the current search, and where the next page
     *  starts (rows read, before dropping repeats). */
    var bookCards by mutableStateOf(emptyList<ListedCard>())
    var bookCardsFor by mutableStateOf<Pair<String, String>?>(null)
    private var bookOffset = 0
    var bookMore by mutableStateOf(false)
    var bookLoading by mutableStateOf(false)
    var bookError by mutableStateOf(false)
    private var bookGeneration = 0

    /** The cards saved in address books for the open contact, and the address
     *  they are for, so another contact's never show. */
    var cards by mutableStateOf(emptyList<BookCard>())
    var cardsAddr by mutableStateOf("")
    var cardsFailed by mutableStateOf("")
    private var directoryGeneration = 0
    private var historyGeneration = 0
    private var failedPin: ContactSuggestion? = null
    private val observedNotes = mutableStateMapOf<String, ContactNoteDraft>()

    fun note(addr: String): ContactNoteDraft {
        val key = contactAddressKey(addr)
        return observedNotes.getOrPut(key) { ContactNoteDraft() }
    }

    /** The open contact under a new name, its history kept. */
    fun rename(name: String) {
        selected = selected?.copy(name = name)
    }

    fun select(contact: ContactSuggestion?) {
        historyGeneration++
        selected = contact?.copy(addr = contactAddressKey(contact.addr))
        threads = emptyList()
        cursor = ""
        historyLoaded = false
        historyLoading = false
        historyError = false
    }

    /** The book shown, once books have loaded: one gone shows everyone. */
    val shownBook: String
        get() =
            when {
                book == OTHER_CONTACTS && books.isNotEmpty() -> OTHER_CONTACTS
                books.any { it.id == book } -> book
                else -> ""
            }

    suspend fun loadBooks() {
        runCatching { Json.parseToJsonElement(invoke("carddav.accounts", "{}")).jsonObject }
            .onSuccess { result ->
                books = parseAddressBookAccounts(result).flatMap { account -> account.books.filter { it.enabled } }
            }.onFailure { if (it is CancellationException) throw it }
        booksLoaded = true
    }

    suspend fun chooseBook(id: String) {
        if (id == book) return
        book = id
        bookCards = emptyList()
        bookCardsFor = null
        runCatching { setPref("contacts_book", JsonPrimitive(id).toString()) }.onFailure { if (it is CancellationException) throw it }
    }

    /** The chosen book's cards matching [query]: the first page afresh, or
     *  with [more] the next one after those shown. */
    suspend fun loadBookCards(
        query: String,
        more: Boolean = false,
    ) {
        val id = shownBook.takeIf { it.isNotBlank() && it != OTHER_CONTACTS } ?: return
        val generation = ++bookGeneration
        bookLoading = true
        bookError = false
        val offset = if (more) bookOffset else 0
        try {
            val result =
                Json
                    .parseToJsonElement(
                        invoke(
                            "carddav.contacts",
                            buildJsonObject {
                                put("book_id", id)
                                put("query", query)
                                put("offset", offset)
                            }.toString(),
                        ),
                    ).jsonObject
            currentCoroutineContext().ensureActive()
            if (generation != bookGeneration) return
            val page = parseListedCards(result)
            // A sync between pages can shift a card into the next one.
            bookCards = (if (more) bookCards + page else page).distinctBy { it.id }
            bookOffset = offset + page.size
            bookMore = (result["more"] as? JsonPrimitive)?.contentOrNull == "true" && page.isNotEmpty()
            bookCardsFor = id to query
        } catch (error: Exception) {
            if (error is CancellationException) throw error
            if (generation == bookGeneration) bookError = true
        } finally {
            if (generation == bookGeneration) bookLoading = false
        }
    }

    /** The cards saved for [addr] in enabled address books. */
    suspend fun loadCards(addr: String) {
        cardsFailed = ""
        runCatching {
            Json.parseToJsonElement(invoke("carddav.cardsFor", buildJsonObject { put("email", addr) }.toString())).jsonObject
        }.onSuccess { result ->
            cards = parseBookCards(result)
            cardsAddr = addr
        }.onFailure {
            if (it is CancellationException) throw it
            cardsFailed = addr
        }
    }

    /** Create, change or delete a card on its server: the core's answer, whose
     *  `sync_error` means saved but not yet refreshed here. Throws with the
     *  server's reason when it refuses. */
    suspend fun mutateCard(
        method: String,
        params: JsonObject,
    ): JsonObject = Json.parseToJsonElement(invoke(method, params.toString())).jsonObject

    suspend fun loadDirectory(
        query: String,
        sort: String,
        limit: Int,
        unsaved: Boolean = false,
    ) {
        val generation = ++directoryGeneration
        directoryLoading = true
        directoryError = false
        try {
            val response =
                invoke(
                    MobileCommand.ContactSuggest,
                    ContactSuggestParams(query = query, sort = sort, limit = limit, directory = true, unsaved = unsaved).toJson(),
                )
            currentCoroutineContext().ensureActive()
            if (generation == directoryGeneration) {
                contacts = parseContactSuggestResponse(response).distinctBy { contactAddressKey(it.addr) }
            }
        } catch (error: Exception) {
            if (error is CancellationException) throw error
            if (generation == directoryGeneration) directoryError = true
        } finally {
            if (generation == directoryGeneration) directoryLoading = false
        }
    }

    suspend fun loadPins() {
        if (pinsLoading || pinsLoaded) return
        pinsLoading = true
        pinsError = false
        try {
            val prefs = Json.parseToJsonElement(invoke("app.prefsGet", AppPrefsGetParams(listOf("pinned_contacts", "contacts_sort", "contacts_book")).toJson())).jsonObject["prefs"]?.jsonObject ?: JsonObject(emptyMap())
            (prefs["contacts_sort"] as? JsonPrimitive)?.contentOrNull?.takeIf { it in listOf("frequent", "recent", "name") }?.let { sort = it }
            (prefs["contacts_book"] as? JsonPrimitive)?.contentOrNull?.let { book = it }
            pins =
                (prefs["pinned_contacts"] as? JsonArray)
                    .orEmpty()
                    .mapNotNull { value ->
                        val obj = value as? JsonObject ?: return@mapNotNull null
                        val addr =
                            (obj["addr"] as? JsonPrimitive)
                                ?.contentOrNull
                                ?.let(::contactAddressKey)
                                ?.takeIf { it.isNotBlank() } ?: return@mapNotNull null
                        ContactSuggestion((obj["name"] as? JsonPrimitive)?.contentOrNull.orEmpty(), addr)
                    }.distinctBy { it.addr }
            pinsLoaded = true
        } catch (error: Exception) {
            if (error is CancellationException) throw error
            pinsError = true
        } finally {
            pinsLoading = false
        }
    }

    suspend fun setSort(value: String) {
        if (sortSaving || !pinsLoaded || value !in listOf("frequent", "recent", "name")) return
        sortSaving = true
        sortError = false
        try {
            setPref("contacts_sort", JsonPrimitive(value).toString())
            sort = value
            limit = 100
            failedSort = null
        } catch (error: Exception) {
            if (error is CancellationException) throw error
            sortError = true
            failedSort = value
        } finally {
            sortSaving = false
        }
    }

    suspend fun retrySort() {
        failedSort?.let { setSort(it) }
    }

    fun isPinned(contact: ContactSuggestion): Boolean = pins.any { contactAddressKey(it.addr) == contactAddressKey(contact.addr) }

    suspend fun togglePin(contact: ContactSuggestion) {
        if (!pinsLoaded || pinsSaving) return
        pinsSaving = true
        pinsError = false
        val updated =
            if (isPinned(contact)) {
                pins.filterNot { contactAddressKey(it.addr) == contactAddressKey(contact.addr) }
            } else {
                pins + contact.copy(addr = contactAddressKey(contact.addr))
            }
        try {
            setPref(
                "pinned_contacts",
                JsonArray(
                    updated.map { item ->
                        buildJsonObject {
                            put("addr", item.addr)
                            put("name", item.name)
                        }
                    },
                ).toString(),
            )
            pins = updated // Never confirm a pin until the core accepted the write.
            failedPin = null
        } catch (error: Exception) {
            if (error is CancellationException) throw error
            pinsError = true
            failedPin = contact
        } finally {
            pinsSaving = false
        }
    }

    suspend fun retryPin() {
        failedPin?.let { togglePin(it) }
    }

    suspend fun loadHistory(more: Boolean = false) {
        val contact = selected ?: return
        if (historyLoading) return
        val generation = historyGeneration
        val before = if (more) cursor else ""
        historyLoading = true
        historyError = false
        try {
            val response =
                invoke(
                    MobileCommand.ContactThreads,
                    ContactThreadsParams(addr = contact.addr, beforeCursor = before.takeIf { it.isNotEmpty() }).toJson(),
                )
            currentCoroutineContext().ensureActive()
            val page = parseContactThreadsPage(response)
            val next = page.nextCursor
            if (generation == historyGeneration) {
                threads = (if (more) threads + page.threads else page.threads).distinctBy { it.id }
                cursor = next.takeUnless { it == before }.orEmpty()
                historyLoaded = true
            }
        } catch (error: Exception) {
            if (error is CancellationException) throw error
            if (generation == historyGeneration) historyError = true
        } finally {
            if (generation == historyGeneration) historyLoading = false
        }
    }

    suspend fun loadNote(addr: String) {
        val draft = note(addr)
        if (draft.loaded || draft.loading) return
        draft.loading = true
        draft.error = false
        try {
            val text = (getPrefs("contact_note:${contactAddressKey(addr)}")["contact_note:${contactAddressKey(addr)}"] as? JsonPrimitive)?.contentOrNull.orEmpty()
            draft.saved = text
            draft.text = text
            draft.loaded = true
        } catch (error: Exception) {
            if (error is CancellationException) throw error
            draft.error = true
        } finally {
            draft.loading = false
        }
    }

    suspend fun saveNote(addr: String) {
        val draft = note(addr)
        if (!draft.loaded || draft.saving || !draft.dirty) return
        val text = draft.text
        draft.saving = true
        draft.error = false
        try {
            setPref("contact_note:${contactAddressKey(addr)}", JsonPrimitive(text).toString())
            draft.saved = text
            if (!draft.dirty) draft.editing = false
        } catch (error: Exception) {
            if (error is CancellationException) throw error
            draft.error = true
        } finally {
            draft.saving = false
        }
    }

    /** Drop the note kept on this device once [moved], its text, is in an
     *  address book: not if it has been edited since, which would lose that. */
    suspend fun clearNoteIfUnchanged(
        addr: String,
        moved: String,
    ) {
        loadNote(addr)
        val draft = note(addr)
        if (!draft.loaded || draft.saved.trim() != moved.trim() || draft.text.trim() != moved.trim()) return
        draft.text = ""
        saveNote(addr)
    }

    private suspend fun getPrefs(key: String): JsonObject = Json.parseToJsonElement(invoke("app.prefsGet", AppPrefsGetParams(listOf(key)).toJson())).jsonObject["prefs"]?.jsonObject ?: JsonObject(emptyMap())

    private suspend fun setPref(
        key: String,
        value: String,
    ) {
        invoke("app.prefsSet", AppPrefsSetParams(key, value).toJson())
    }

    private suspend fun invoke(
        command: String,
        payload: String,
    ): String {
        val response = withContext(ioDispatcher) { requireCoreOk(core.invoke(command, payload)) }
        // Native hosts return the JSON-RPC envelope; test/other hosts may
        // return the result directly. Structural reads must handle both.
        val objectValue = Json.parseToJsonElement(response).jsonObject
        return (objectValue["result"] ?: objectValue).toString()
    }
}
