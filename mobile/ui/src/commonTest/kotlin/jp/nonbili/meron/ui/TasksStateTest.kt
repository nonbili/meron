package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.CloseableHandle
import jp.nonbili.meron.shared.CoreEvent
import jp.nonbili.meron.shared.CoreEventStream
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.ThreadSummary
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlin.test.Test
import kotlin.test.assertEquals

class TasksStateTest {
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

    private class RecordingCore : MeronCore {
        var createdPayload = ""
        var lists = """[{"id":"list-1","title":"First"},{"id":"list-2","title":"Second"}]"""

        override suspend fun invoke(
            command: String,
            payloadJson: String,
        ): String {
            if (command == "tasks.lists") return """{"lists":$lists,"default_list_id":"list-1"}"""
            if (command == "tasks.create") createdPayload = payloadJson
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
