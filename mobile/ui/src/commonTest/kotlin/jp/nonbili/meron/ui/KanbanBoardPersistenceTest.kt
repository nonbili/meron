package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.CloseableHandle
import jp.nonbili.meron.shared.CoreEvent
import jp.nonbili.meron.shared.CoreEventStream
import jp.nonbili.meron.shared.MeronCore
import kotlinx.coroutines.CoroutineScope
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class KanbanBoardPersistenceTest {
    @Test
    fun seedsAStarterBoardForAProfileThatNeverStoredAny() {
        val prefs = FakePreferences()

        val boards = ensureKanbanDefaults(prefs, emptyList(), emptyList())

        assertEquals(1, boards.size)
        assertTrue(parseKanbanBoards(prefs.getString(KANBAN_BOARDS_PREF, "")).size == 1)
    }

    @Test
    fun keepsAnEmptyBoardListTheUserDeletedDownTo() {
        val prefs = FakePreferences()
        saveKanbanBoards(prefs, emptyList())

        assertEquals(emptyList(), ensureKanbanDefaults(prefs, emptyList(), emptyList()))
    }

    @Test
    fun leavesStoredBoardsAlone() {
        val prefs = FakePreferences()
        val stored = listOf(defaultKanbanBoard(emptyList<AccountSummary>()))
        saveKanbanBoards(prefs, stored)

        assertEquals(stored.map { it.id }, ensureKanbanDefaults(prefs, stored, emptyList()).map { it.id })
    }

    @Test
    fun deletingTheLastBoardLeavesNoBoard() {
        val prefs = FakePreferences()
        saveKanbanBoards(prefs, listOf(defaultKanbanBoard(emptyList<AccountSummary>())))
        val state = testState(prefs)
        val board = state.kanbanBoards.single()
        state.activeKanbanBoardId = board.id

        state.deleteKanbanBoard(board.id)

        assertEquals(emptyList(), state.kanbanBoards)
        assertEquals("", state.activeKanbanBoardId)
        assertEquals(emptyList(), parseKanbanBoards(prefs.getString(KANBAN_BOARDS_PREF, "")))
    }

    @Test
    fun aColumnSelectionMadeWithNoBoardLeftCreatesOne() {
        val state = testState(FakePreferences())
        val column = KanbanColumnSpec(UNIFIED_ACCOUNT_ID, INBOX_FOLDER)

        state.applyKanbanColumns(listOf(column))

        val board = state.kanbanBoards.single()
        assertEquals(listOf(column), board.columns)
        assertEquals(board.id, state.activeKanbanBoardId)
    }

    @Test
    fun anEmptySelectionWithNoBoardLeftCreatesNothing() {
        val state = testState(FakePreferences())

        state.applyKanbanColumns(emptyList())

        assertEquals(emptyList(), state.kanbanBoards)
        assertEquals("", state.activeKanbanBoardId)
    }

    private fun testState(kanbanPrefs: AppPreferences): MeronMobileState {
        val core = FakeCore()
        return MeronMobileState(
            scope = CoroutineScope(EmptyCoroutineContext),
            core = core,
            coreLoaded = true,
            prefs = FakePreferences(),
            kanbanPrefs = kanbanPrefs,
            services = FakePlatformServices(),
            locale = FakeLocaleController(),
            mobileHost = DefaultMobileHost(),
            settingsMirror = SettingsMirror(core, FakePreferences()) { true },
        )
    }

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

    private class FakeCore : MeronCore {
        override suspend fun invoke(
            command: String,
            payloadJson: String,
        ): String = "{}"

        override fun events(): CoreEventStream =
            object : CoreEventStream {
                override fun subscribe(listener: (CoreEvent) -> Unit): CloseableHandle = CloseableHandle {}
            }

        override suspend fun protocolVersion(): Int = 0
    }
}
