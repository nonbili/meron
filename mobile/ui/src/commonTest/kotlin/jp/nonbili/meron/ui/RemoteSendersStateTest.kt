package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.CloseableHandle
import jp.nonbili.meron.shared.CoreEvent
import jp.nonbili.meron.shared.CoreEventStream
import jp.nonbili.meron.shared.MeronCore
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.job
import kotlinx.coroutines.joinAll
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.yield
import kotlin.test.Test
import kotlin.test.assertEquals

/**
 * The remote-content allowlist is one whole-list row shared with desktop, so
 * every edit is a read-modify-write against the store. These pin what that has
 * to guarantee: no edit is written against a list this state has not read yet,
 * concurrent edits do not drop each other, and a failed write takes back only
 * its own change.
 */
class RemoteSendersStateTest {
    @Test
    fun anEditMadeBeforeTheStartupReadLandsKeepsTheStoredSenders() =
        runBlocking {
            val core = FakeSettingsCore("stored@example.com")
            // Hold both reads: the edit is made while this state still shows an
            // empty allowlist, which is the race.
            core.holdReads(count = 2)
            val state = testState(core, this)

            state.loadRemoteImageSenders()
            state.setRemoteImageSender("new@example.com", allowed = true)
            core.awaitReads(2)
            core.releaseReads()
            settle()

            // The edit went in on top of what the store held, not on top of the
            // empty list it was made against...
            assertEquals(listOf("stored@example.com", "new@example.com"), core.storedSenders())
            // ...and the read that was in flight did not put the pre-edit
            // snapshot back on screen.
            assertEquals(listOf("stored@example.com", "new@example.com"), state.remoteImageSenders)
        }

    @Test
    fun aStartupReadStillInFlightCannotUndoAnEditThatLanded() =
        runBlocking {
            val core = FakeSettingsCore("stored@example.com")
            // Only the startup read is held, so it answers with the row as it
            // was before the edit below — the snapshot that must not win.
            core.holdReads(count = 1)
            val state = testState(core, this)

            state.loadRemoteImageSenders()
            core.awaitReads(1)
            state.setRemoteImageSender("new@example.com", allowed = true)
            core.awaitWrites(1)
            core.releaseReads()
            settle()

            assertEquals(listOf("stored@example.com", "new@example.com"), state.remoteImageSenders)
        }

    @Test
    fun backToBackEditsBothSurvive() =
        runBlocking {
            val core = FakeSettingsCore()
            val state = testState(core, this)

            state.setRemoteImageSender("one@example.com", allowed = true)
            state.setRemoteImageSender("two@example.com", allowed = true)
            settle()

            assertEquals(listOf("one@example.com", "two@example.com"), state.remoteImageSenders)
            assertEquals(listOf("one@example.com", "two@example.com"), core.storedSenders())

            // ...and removing one leaves the other trusted.
            state.setRemoteImageSender("one@example.com", allowed = false)
            settle()
            assertEquals(listOf("two@example.com"), state.remoteImageSenders)
            assertEquals(listOf("two@example.com"), core.storedSenders())
        }

    @Test
    fun aFailedWriteTakesBackOnlyItsOwnEdit() =
        runBlocking {
            val core = FakeSettingsCore()
            val state = testState(core, this)

            state.setRemoteImageSender("kept@example.com", allowed = true)
            settle()
            core.failWrites = true
            state.setRemoteImageSender("failed@example.com", allowed = true)
            settle()

            assertEquals(listOf("kept@example.com"), state.remoteImageSenders)
            assertEquals(listOf("kept@example.com"), core.storedSenders())
        }

    /** Let every read/write the calls above launched run to completion. They are
     *  children of the test's own scope, so this is exact rather than a wait. */
    private suspend fun CoroutineScope.settle() {
        coroutineContext.job.children
            .toList()
            .joinAll()
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

    /**
     * A core holding just the allowlist row, with the reads and the write
     * openable one at a time so a test can interleave them deterministically.
     */
    private class FakeSettingsCore(
        vararg initial: String,
    ) : MeronCore {
        private var row: String = initial.joinToString(",", "[", "]") { "\"$it\"" }
        private var readGate: CompletableDeferred<Unit>? = null
        private var gatedReads = 0
        private var readsStarted = 0
        private var writes = 0
        var failWrites: Boolean = false

        fun storedSenders(): List<String> =
            row
                .removeSurrounding("[", "]")
                .split(",")
                .map { it.trim().removeSurrounding("\"") }
                .filter { it.isNotEmpty() }

        /** Make the first [count] reads wait until [releaseReads]. Each still
         *  answers with the row as it was when the read began, which is what
         *  makes a held read a stale snapshot. */
        fun holdReads(count: Int) {
            gatedReads = count
            readGate = CompletableDeferred()
        }

        fun releaseReads() {
            readGate?.complete(Unit)
        }

        /** Wait for [count] reads to have begun. They run on the IO dispatcher,
         *  so this yields rather than joining anything. */
        suspend fun awaitReads(count: Int) = awaitCount(count) { readsStarted }

        /** Wait for [count] writes to have been stored. */
        suspend fun awaitWrites(count: Int) = awaitCount(count) { writes }

        private suspend fun awaitCount(
            count: Int,
            seen: () -> Int,
        ) {
            repeat(WAIT_YIELDS) {
                if (seen() >= count) return
                yield()
            }
        }

        override suspend fun invoke(
            command: String,
            payloadJson: String,
        ): String =
            when (command) {
                "app.prefsGet" -> {
                    readsStarted++
                    val snapshot = row
                    if (readsStarted <= gatedReads) readGate?.await()
                    """{"prefs":{"$REMOTE_IMAGE_SENDERS_SETTING_KEY":$snapshot}}"""
                }

                "app.prefsSet" -> {
                    if (failWrites) throw IllegalStateException("write failed")
                    row = payloadJson.substringAfter(""""value":""").removeSuffix("}")
                    writes++
                    "{}"
                }

                else -> {
                    "{}"
                }
            }

        override fun events(): CoreEventStream =
            object : CoreEventStream {
                override fun subscribe(listener: (CoreEvent) -> Unit): CloseableHandle = CloseableHandle {}
            }

        override suspend fun protocolVersion(): Int = 0
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
}

// Yields are cheap; the ceiling only stops a broken read or write from hanging
// the suite.
private const val WAIT_YIELDS = 100_000
