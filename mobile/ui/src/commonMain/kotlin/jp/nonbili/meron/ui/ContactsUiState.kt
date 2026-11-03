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
    private var directoryGeneration = 0
    private var historyGeneration = 0
    private var failedPin: ContactSuggestion? = null
    private val observedNotes = mutableStateMapOf<String, ContactNoteDraft>()

    fun note(addr: String): ContactNoteDraft {
        val key = contactAddressKey(addr)
        return observedNotes.getOrPut(key) { ContactNoteDraft() }
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

    suspend fun loadDirectory(
        query: String,
        sort: String,
        limit: Int,
    ) {
        val generation = ++directoryGeneration
        directoryLoading = true
        directoryError = false
        try {
            val response =
                invoke(
                    MobileCommand.ContactSuggest,
                    ContactSuggestParams(query = query, sort = sort, limit = limit, directory = true).toJson(),
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
            val prefs = Json.parseToJsonElement(invoke("app.prefsGet", AppPrefsGetParams(listOf("pinned_contacts", "contacts_sort")).toJson())).jsonObject["prefs"]?.jsonObject ?: JsonObject(emptyMap())
            (prefs["contacts_sort"] as? JsonPrimitive)?.contentOrNull?.takeIf { it in listOf("frequent", "recent", "name") }?.let { sort = it }
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
