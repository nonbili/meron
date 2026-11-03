package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.CloseableHandle
import jp.nonbili.meron.shared.ContactSuggestion
import jp.nonbili.meron.shared.CoreEvent
import jp.nonbili.meron.shared.CoreEventStream
import jp.nonbili.meron.shared.MeronCore
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class ContactsScreenStateTest {
    private val alice = ContactSuggestion("Alice", "Alice@Example.com")
    private val bob = ContactSuggestion("Bob", "bob@example.com")

    @Test
    fun preferencesAndHistoryReadNativeJsonRpcEnvelopes() =
        runBlocking {
            val core =
                object : MeronCore {
                    override suspend fun invoke(
                        command: String,
                        payloadJson: String,
                    ): String =
                        when (command) {
                            "app.prefsGet" -> """{"id":1,"result":{"prefs":{"contacts_sort":"name","pinned_contacts":[{"name":"Alice","addr":"alice@example.com"}],"contact_note:alice@example.com":"saved note"}}}"""
                            "mail.contactThreads" -> """{"id":1,"result":{"threads":[],"next_cursor":"opaque-next"}}"""
                            else -> """{"id":1,"result":{"ok":true}}"""
                        }

                    override fun events() =
                        object : CoreEventStream {
                            override fun subscribe(listener: (CoreEvent) -> Unit) = CloseableHandle {}
                        }

                    override suspend fun protocolVersion() = 1
                }
            val state = ContactsScreenState(core)
            state.loadPins()
            state.select(alice)
            state.loadNote(alice.addr)
            state.loadHistory()
            assertTrue(state.isPinned(alice))
            assertEquals("name", state.sort)
            assertEquals("saved note", state.note(alice.addr).text)
            assertEquals("opaque-next", state.cursor)
        }

    @Test
    fun contactIdentityIsTrimmedAndCaseInsensitive() {
        assertEquals("alice@example.com", contactAddressKey(" Alice@Example.COM "))
        val state = ContactsScreenState(ContactsTestCore())
        state.select(ContactSuggestion("Alice", " Alice@Example.COM "))
        assertEquals("alice@example.com", state.selected?.addr)
        state.note(" Alice@Example.COM ").text = "draft"
        assertEquals("draft", state.note("alice@example.com").text)
    }

    @Test
    fun directoryRequestsAcrossAccountsWithDirectoryFiltersAndSort() =
        runBlocking {
            var request = ""
            val core =
                object : MeronCore {
                    override suspend fun invoke(
                        command: String,
                        payloadJson: String,
                    ): String {
                        assertEquals("mail.suggestContacts", command)
                        request = payloadJson
                        return """{"contacts":[{"name":"Alice","addr":"alice@example.com"}]}"""
                    }

                    override fun events() =
                        object : CoreEventStream {
                            override fun subscribe(listener: (CoreEvent) -> Unit) = CloseableHandle {}
                        }

                    override suspend fun protocolVersion() = 1
                }
            val state = ContactsScreenState(core)
            state.loadDirectory("ali", "recent", 200)
            val params = Json.parseToJsonElement(request).jsonObject
            assertEquals("", params["account"]?.jsonPrimitive?.content)
            assertEquals("ali", params["query"]?.jsonPrimitive?.content)
            assertEquals("recent", params["sort"]?.jsonPrimitive?.content)
            assertEquals("true", params["directory"]?.jsonPrimitive?.content)
            assertEquals("200", params["limit"]?.jsonPrimitive?.content)
            assertEquals("alice@example.com", state.contacts.single().addr)
            assertFalse(state.directoryLoading)
            assertFalse(state.directoryError)
        }

    @Test
    fun sortLoadsPersistsAndFailedWriteDoesNotConfirm() =
        runBlocking {
            val core = ContactsTestCore()
            core.prefsResponse = """{"prefs":{"contacts_sort":"recent"}}"""
            val state = ContactsScreenState(core)
            state.loadPins()
            assertEquals("recent", state.sort)
            state.query = "Alice"
            state.limit = 200
            core.failWrites = true
            state.setSort("name")
            assertEquals("recent", state.sort)
            assertTrue(state.sortError)
            assertEquals(200, state.limit)
            core.failWrites = false
            state.retrySort()
            assertEquals("name", state.sort)
            assertEquals("contacts_sort", core.lastWriteKey)
            assertEquals(100, state.limit)
            assertEquals("Alice", state.query)
        }

    @Test
    fun failedPinIsNotConfirmedAndCanBeRetried() =
        runBlocking {
            val core = ContactsTestCore()
            val state = ContactsScreenState(core)
            state.loadPins()
            core.failWrites = true
            state.togglePin(alice)
            assertFalse(state.isPinned(alice))
            assertTrue(state.pinsError)
            core.failWrites = false
            state.retryPin()
            assertTrue(state.isPinned(alice))
            assertEquals("alice@example.com", state.pins.single().addr)
            state.togglePin(alice)
            assertTrue(state.pins.isEmpty())
        }

    @Test
    fun draftsSurviveContactSwitchesAndFailedSaves() =
        runBlocking {
            val core = ContactsTestCore()
            val state = ContactsScreenState(core)
            state.select(alice)
            state.loadNote(alice.addr)
            state.note(alice.addr).text = "private draft"
            state.select(bob)
            state.loadNote(bob.addr)
            state.select(alice)
            core.failWrites = true
            state.saveNote(alice.addr)
            assertEquals("private draft", state.note(alice.addr).text)
            assertEquals("", state.note(alice.addr).saved)
            assertTrue(state.note(alice.addr).error)
            core.failWrites = false
            state.saveNote(alice.addr)
            assertEquals("private draft", state.note(alice.addr).saved)
            assertFalse(state.note(alice.addr).dirty)
            assertEquals("contact_note:alice@example.com", core.lastWriteKey)
        }

    @Test
    fun lateHistoryCannotPopulateAnotherContact() =
        runBlocking {
            val core = ContactsTestCore()
            val oldResponse = CompletableDeferred<String>()
            val started = CompletableDeferred<Unit>()
            core.history = { addr, _ ->
                if (addr.startsWith("alice")) {
                    started.complete(Unit)
                    oldResponse.await()
                } else {
                    """{"threads":[],"next_cursor":"bob-next"}"""
                }
            }
            val state = ContactsScreenState(core)
            state.select(alice)
            val old = launch { state.loadHistory() }
            started.await()
            state.select(bob)
            state.loadHistory()
            oldResponse.complete("""{"threads":[],"next_cursor":"alice-next"}""")
            old.join()
            assertEquals("bob-next", state.cursor)
            assertEquals("bob@example.com", state.selected?.addr)
        }

    @Test
    fun historyUsesOpaqueCursorAndStopsRepeatedCursor() =
        runBlocking {
            val core = ContactsTestCore()
            val cursors = mutableListOf<String>()
            core.history = { _, cursor ->
                cursors += cursor
                """{"threads":[],"next_cursor":"opaque:cursor/50"}"""
            }
            val state = ContactsScreenState(core)
            state.select(alice)
            state.loadHistory()
            state.loadHistory(more = true)
            assertEquals(listOf("", "opaque:cursor/50"), cursors)
            assertEquals("", state.cursor)
        }

    @Test
    fun equalFolderScopedUidsRemainSeparateActionableRows() =
        runBlocking {
            val core = ContactsTestCore()
            core.history = { _, _ ->
                """{"threads":[{"id":"a:INBOX:42","account_id":"a","folder":"INBOX","thread_id":"uid:42","subject":"Inbox"},{"id":"a:Archive:42","account_id":"a","folder":"Archive","thread_id":"uid:42","subject":"Archive"}]}"""
            }
            val state = ContactsScreenState(core)
            state.select(alice)
            state.loadHistory()
            assertEquals(listOf("a:INBOX:42", "a:Archive:42"), state.threads.map { it.id })
        }
}

private class ContactsTestCore : MeronCore {
    var prefsResponse = """{"prefs":{}}"""
    var failWrites = false
    var lastWriteKey = ""
    var history: suspend (String, String) -> String = { _, _ -> """{"threads":[]}""" }

    override suspend fun invoke(
        command: String,
        payloadJson: String,
    ): String {
        val payload = Json.parseToJsonElement(payloadJson).jsonObject
        return when (command) {
            "app.prefsGet" -> {
                prefsResponse
            }

            "app.prefsSet" -> {
                if (failWrites) error("write failed")
                lastWriteKey = payload.getValue("key").jsonPrimitive.content
                "{}"
            }

            "mail.contactThreads" -> {
                history(payload.getValue("addr").jsonPrimitive.content, payload["before_cursor"]?.jsonPrimitive?.content.orEmpty())
            }

            else -> {
                error(command)
            }
        }
    }

    override fun events() =
        object : CoreEventStream {
            override fun subscribe(listener: (CoreEvent) -> Unit) = CloseableHandle {}
        }

    override suspend fun protocolVersion() = 1
}
