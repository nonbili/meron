package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.CloseableHandle
import jp.nonbili.meron.shared.CoreEvent
import jp.nonbili.meron.shared.CoreEventStream
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.MobileCommand
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlin.test.Test
import kotlin.test.assertEquals

/**
 * Stepping aside for the load already out for a mailbox is not the same as
 * being answered by it: that load read the store before the change the reload
 * is reacting to. A quick reply's post-send draft discard used to run its reload
 * while the reload the send itself started was still out, and the rows that
 * one then painted still counted the discarded draft — the card showed one
 * message too many until something else reloaded the list.
 */
class MailboxReloadDeferralTest {
    @Test
    fun reloadThatSteppedAsideRunsOnceTheLoadInFlightSettles() =
        runBlocking {
            val core = GatedCore()
            val state = state(core, this)

            state.syncCoreThreads(syncFirst = false)
            waitUntil { core.threadListCalls == 1 }
            // The post-discard reload, while the first one is still out.
            state.syncCoreThreads(syncFirst = false)
            delay(20)
            assertEquals(1, core.threadListCalls)

            core.gate.complete(Unit)
            waitUntil { core.threadListCalls == 2 }
            waitUntil { !state.syncing }
            assertEquals(2, core.threadListCalls)
        }

    // A search paints cache hits first, then runs the live IMAP search from the
    // cache stage's completion. Re-running the deferred reload ahead of that
    // started a cache read the live search stepped aside for, whose completion
    // re-ran it again — the two took turns for good and the search never ran.
    @Test
    fun deferredReloadDuringSearchYieldsToTheLiveSearchAndStops() =
        runBlocking {
            val core = GatedCore()
            val state = state(core, this)
            state.mailSearch = "deploy"

            state.syncCoreThreads(syncFirst = false)
            waitUntil { core.threadListCalls == 1 }
            state.syncCoreThreads(syncFirst = false)
            delay(20)
            assertEquals(1, core.threadListCalls)

            core.gate.complete(Unit)
            waitUntil { core.threadListCalls == 2 && !state.syncing }
            delay(50)
            assertEquals(2, core.threadListCalls)
            assertEquals(1, core.liveSearchCalls)
        }

    // A live search that stepped aside is re-run as one: as a cache read it
    // would only schedule the search again.
    @Test
    fun deferredLiveSearchIsReRunAsALiveSearch() =
        runBlocking {
            val core = GatedCore()
            val state = state(core, this)
            state.mailSearch = "deploy"

            state.syncCoreThreads(syncFirst = false, refreshSearch = true)
            waitUntil { core.threadListCalls == 1 }
            state.syncCoreThreads(syncFirst = false, refreshSearch = true)
            delay(20)
            assertEquals(1, core.threadListCalls)

            core.gate.complete(Unit)
            waitUntil { core.threadListCalls == 2 && !state.syncing }
            delay(50)
            assertEquals(2, core.threadListCalls)
            assertEquals(2, core.liveSearchCalls)
        }

    @Test
    fun aReloadWithNothingInItsWayRunsOnce() =
        runBlocking {
            val core = GatedCore().apply { gate.complete(Unit) }
            val state = state(core, this)

            state.syncCoreThreads(syncFirst = false)
            waitUntil { core.threadListCalls == 1 && !state.syncing }
            delay(20)
            assertEquals(1, core.threadListCalls)
        }

    private suspend fun waitUntil(condition: () -> Boolean) {
        withTimeout(5_000) {
            while (!condition()) delay(5)
        }
    }

    private fun state(
        core: MeronCore,
        scope: CoroutineScope,
    ): MeronMobileState =
        MeronMobileState(
            scope = scope,
            core = core,
            coreLoaded = true,
            prefs = MemoryPreferences(),
            kanbanPrefs = MemoryPreferences(),
            services = NoopPlatformServices(),
            locale = NoopLocaleController(),
            mobileHost = DefaultMobileHost(),
            settingsMirror = SettingsMirror(core, MemoryPreferences()) { true },
        ).apply {
            coreAccounts =
                listOf(
                    AccountSummary(
                        id = "a",
                        email = "a@example.com",
                        imapHost = "127.0.0.1",
                        imapPort = 1143,
                        smtpHost = "127.0.0.1",
                        smtpPort = 1025,
                        tls = false,
                        starttls = true,
                        smtpTls = false,
                        smtpStarttls = true,
                    ),
                )
            selectedCoreAccountId = "a"
            selectedCoreFolder = "INBOX"
            initialThreadsLoaded = true
        }

    /** Holds the first thread list read open until [gate] completes. */
    private class GatedCore : MeronCore {
        val gate = CompletableDeferred<Unit>()
        var threadListCalls = 0
        var liveSearchCalls = 0

        override suspend fun invoke(
            command: String,
            payloadJson: String,
        ): String =
            when (command) {
                MobileCommand.FolderList -> {
                    """{"folders":[{"account_id":"a","name":"INBOX","role":"inbox"}]}"""
                }

                MobileCommand.ThreadList -> {
                    threadListCalls += 1
                    if (payloadJson.contains("\"refresh\":true")) liveSearchCalls += 1
                    gate.await()
                    """{"threads":[]}"""
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

    private class MemoryPreferences : AppPreferences {
        private val values = mutableMapOf<String, String>()

        override fun getString(
            key: String,
            default: String,
        ): String = values[key] ?: default

        override fun putString(
            key: String,
            value: String,
        ) {
            values[key] = value
        }

        override fun getBoolean(
            key: String,
            default: Boolean,
        ): Boolean = default

        override fun putBoolean(
            key: String,
            value: Boolean,
        ) {}

        override fun getInt(
            key: String,
            default: Int,
        ): Int = default

        override fun putInt(
            key: String,
            value: Int,
        ) {}

        override fun getStringSet(
            key: String,
            default: Set<String>,
        ): Set<String> = default

        override fun putStringSet(
            key: String,
            value: Set<String>,
        ) {}

        override fun remove(key: String) {
            values.remove(key)
        }
    }

    private class NoopPlatformServices : PlatformServices {
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

    private class NoopLocaleController : LocaleController {
        override fun systemLanguageTag(): String = ""

        override fun applySystem(tag: String) {}

        override fun deviceLanguageTag(): String = "en-US"

        override fun displayName(tag: String): String = tag
    }
}
