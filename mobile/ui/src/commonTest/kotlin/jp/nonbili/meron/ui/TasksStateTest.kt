package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.CloseableHandle
import jp.nonbili.meron.shared.CoreEvent
import jp.nonbili.meron.shared.CoreEventStream
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.TaskSummary
import jp.nonbili.meron.shared.ThreadSummary
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.cancelChildren
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlin.test.Test
import kotlin.test.assertEquals

private const val TASK_THREAD_ID = "account#INBOX#t.dGhyZWFk"

class TasksStateTest {
    private val taskWithMail =
        TaskSummary(
            id = "task-1",
            listId = "list-1",
            title = "Pay the rent",
            account = "account",
            threadId = TASK_THREAD_ID,
        )

    @Test
    fun mailUsesTheSelectedListEvenFromTheThreadScreen() =
        runBlocking {
            val core = RecordingCore()
            val state = testState(core, this)
            state.activeTaskListId = "list-2"
            state.screen = Screen.Thread
            state.addTaskFromMessage("Follow up", "account", "thread", "message")
            val payload = Json.parseToJsonElement(core.createdPayload).jsonObject
            assertEquals("list-2", payload["list_id"]?.jsonPrimitive?.content)
            assertEquals("account", payload["account"]?.jsonPrimitive?.content)
            assertEquals("thread", payload["thread_id"]?.jsonPrimitive?.content)
            assertEquals("message", payload["message_id"]?.jsonPrimitive?.content)
            assertEquals(Screen.Thread, state.screen)
        }

    @Test
    fun mailUsesTheCoreDefaultBeforeAnyListIsSelected() =
        runBlocking {
            val core = RecordingCore()
            val state = testState(core, this)
            state.addTaskFromMessage("Follow up", "account", "thread")
            val payload = Json.parseToJsonElement(core.createdPayload).jsonObject
            assertEquals("", payload["list_id"]?.jsonPrimitive?.content)
        }

    @Test
    fun multipleListsWaitForAnExplicitChoice() =
        runBlocking {
            val core = RecordingCore()
            val state = testState(core, this)
            state.activeTaskListId = "list-1"
            val thread = ThreadSummary(id = "thread", accountId = "account", folder = "INBOX", subject = "Follow up", sender = "sender")
            state.requestAddMailToTasks(thread)
            assertEquals("", core.createdPayload)
            assertEquals(thread, state.pendingTaskThread)
            state.pickMailTaskList("list-2")
            assertEquals(null, state.pendingTaskThread)
            val payload = Json.parseToJsonElement(core.createdPayload).jsonObject
            assertEquals("list-2", payload["list_id"]?.jsonPrimitive?.content)
            assertEquals("thread", payload["thread_id"]?.jsonPrimitive?.content)
        }

    @Test
    fun aSingleListSkipsThePicker() =
        runBlocking {
            val core = RecordingCore()
            core.lists = """[{"id":"only","title":"Only"}]"""
            val state = testState(core, this)
            state.requestAddMailToTasks(ThreadSummary(id = "thread", accountId = "account", folder = "INBOX", subject = "Follow up", sender = "sender"))
            assertEquals(null, state.pendingTaskThread)
            val payload = Json.parseToJsonElement(core.createdPayload).jsonObject
            assertEquals("only", payload["list_id"]?.jsonPrimitive?.content)
        }

    @Test
    fun dismissingThePickerDoesNotCreateATask() =
        runBlocking {
            val core = RecordingCore()
            val state = testState(core, this)
            state.requestAddMailToTasks(ThreadSummary(id = "thread", accountId = "account", folder = "INBOX", subject = "Follow up", sender = "sender"))
            state.pendingTaskThread = null
            state.pickMailTaskList("list-2")
            assertEquals("", core.createdPayload)
        }

    // A task's mail opens wherever it now lives: the thread id names the folder
    // it was filed from, but the read resolves it across the account.
    @Test
    fun taskMailOpensAfterItHasBeenMovedToAnotherFolder() =
        runBlocking {
            val core = RecordingCore()
            core.threadMessages = """[{"id":"m1","folder_id":"Archive","subject":"Rent","from_addr":"landlord@example.com","date":7}]"""
            val state = testState(core, this)
            state.openTaskThread(taskWithMail)
            awaitState { state.selectedCoreThread != null }
            assertEquals(TASK_THREAD_ID, state.selectedCoreThread?.threadId)
            assertEquals(Screen.Thread, state.screen)
            coroutineContext.cancelChildren()
        }

    // Deleted for good: the user is told, and nothing else about the task
    // changes — the entry stays pressable for when a sync brings the mail back.
    @Test
    fun taskMailThatIsGoneIsReported() =
        runBlocking {
            val core = RecordingCore()
            core.threadMessages = "[]"
            val state = testState(core, this)
            state.openTaskThread(taskWithMail)
            awaitState { state.status.isNotBlank() }
            assertEquals(null, state.selectedCoreThread)
            assertEquals("Message no longer available", state.status)
            coroutineContext.cancelChildren()
        }

    @Test
    fun taskMailOpensOnceItComesBack() =
        runBlocking {
            val core = RecordingCore()
            core.threadMessages = "[]"
            val state = testState(core, this)
            state.openTaskThread(taskWithMail)
            awaitState { state.status.isNotBlank() }
            core.threadMessages = """[{"id":"m1","folder_id":"INBOX","subject":"Rent","from_addr":"landlord@example.com","date":7}]"""

            state.openTaskThread(taskWithMail)
            awaitState { state.selectedCoreThread != null }

            assertEquals(TASK_THREAD_ID, state.selectedCoreThread?.threadId)
            coroutineContext.cancelChildren()
        }

    /**
     * Wait for the state a launched open lands in. The tests cancel what is
     * left running afterwards: opening a conversation starts a wait for the
     * app signature that only a real core ever settles.
     */
    private suspend fun awaitState(reached: () -> Boolean) {
        withTimeout(5_000) {
            while (!reached()) delay(5)
        }
    }

    private class RecordingCore : MeronCore {
        var createdPayload = ""
        var lists = """[{"id":"list-1","title":"First"},{"id":"list-2","title":"Second"}]"""
        var threadMessages = "[]"

        override suspend fun invoke(
            command: String,
            payloadJson: String,
        ): String {
            if (command == "tasks.lists") return """{"lists":$lists,"default_list_id":"list-1"}"""
            if (command == "tasks.create") createdPayload = payloadJson
            if (command == "mail.threadRead") return """{"messages":$threadMessages}"""
            return "{}"
        }

        override fun events(): CoreEventStream =
            object : CoreEventStream {
                override fun subscribe(listener: (CoreEvent) -> Unit): CloseableHandle = CloseableHandle {}
            }

        override suspend fun protocolVersion(): Int = 0
    }

    private fun testState(
        core: MeronCore,
        scope: CoroutineScope,
    ): MeronMobileState =
        MeronMobileState(
            scope = scope,
            core = core,
            coreLoaded = true,
            prefs = FakePreferences(),
            kanbanPrefs = FakePreferences(),
            services = FakePlatformServices(),
            locale = FakeLocaleController(),
            mobileHost = DefaultMobileHost(),
            settingsMirror = SettingsMirror(core, FakePreferences()) { true },
        )

    private class FakePlatformServices : PlatformServices {
        override fun openUrl(url: String) {}

        override fun openOAuthUrl(
            url: String,
            callbackScheme: String,
            onCallback: (String) -> Unit,
            onFailure: (String) -> Unit,
        ) {}

        override fun copyText(
            label: String,
            value: String,
        ) {}

        override fun copyImage(
            bytes: ByteArray,
            mimeType: String,
            label: String,
        ) {}

        override fun shareFile(
            bytes: ByteArray,
            fileName: String,
            mimeType: String,
        ) {}

        override fun saveFile(
            bytes: ByteArray,
            fileName: String,
            mimeType: String,
        ) {}

        override fun pickFile(
            mimeTypes: List<String>,
            onPicked: (PickedFile?) -> Unit,
        ) {}

        override fun pickImage(onPicked: (PickedFile?) -> Unit) {}
    }

    private class FakePreferences : AppPreferences {
        private val strings = mutableMapOf<String, String>()
        private val booleans = mutableMapOf<String, Boolean>()
        private val ints = mutableMapOf<String, Int>()
        private val stringSets = mutableMapOf<String, Set<String>>()

        override fun getString(
            key: String,
            default: String,
        ): String = strings[key] ?: default

        override fun putString(
            key: String,
            value: String,
        ) {
            strings[key] = value
        }

        override fun getBoolean(
            key: String,
            default: Boolean,
        ): Boolean = booleans[key] ?: default

        override fun putBoolean(
            key: String,
            value: Boolean,
        ) {
            booleans[key] = value
        }

        override fun getInt(
            key: String,
            default: Int,
        ): Int = ints[key] ?: default

        override fun putInt(
            key: String,
            value: Int,
        ) {
            ints[key] = value
        }

        override fun getStringSet(
            key: String,
            default: Set<String>,
        ): Set<String> = stringSets[key] ?: default

        override fun putStringSet(
            key: String,
            value: Set<String>,
        ) {
            stringSets[key] = value
        }

        override fun remove(key: String) {
            strings.remove(key)
            booleans.remove(key)
            ints.remove(key)
            stringSets.remove(key)
        }
    }

    private class FakeLocaleController : LocaleController {
        override fun systemLanguageTag(): String = ""

        override fun applySystem(tag: String) {}

        override fun deviceLanguageTag(): String = "en-US"

        override fun displayName(tag: String): String = tag
    }
}
