package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.CloseableHandle
import jp.nonbili.meron.shared.CoreEvent
import jp.nonbili.meron.shared.CoreEventStream
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.MobileCommand
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse

/**
 * Two reloads of one Kanban column can be out at once — the Sent copy event's
 * and a quick reply's post-discard one — and the earlier read the store before
 * the discard. Landing last, it used to put the card back with the draft
 * counted and badged; only the newest request may write.
 */
class KanbanColumnLoadOrderTest {
    @Test
    fun staleColumnLoadLandingLastDoesNotOverwriteTheNewerRows() =
        runBlocking {
            val core = GatedCore()
            val state = state(core, this)
            val column = KanbanColumnSpec(accountId = "a", folderId = "INBOX")
            val key = kanbanColumnKey(column)

            state.loadKanbanColumn(column)
            waitUntil { core.threadListCalls == 1 }
            // The post-discard reload, while the first one is still out.
            state.loadKanbanColumn(column)
            waitUntil { state.kanbanColumns[key]?.threads?.isNotEmpty() == true }
            val fresh = state.kanbanColumns[key]?.threads?.single()
            assertEquals(2, fresh?.messageCount)

            core.gate.complete(Unit)
            delay(50)
            val card = state.kanbanColumns[key]?.threads?.single()
            assertEquals(2, card?.messageCount)
            assertFalse(card?.hasDraft ?: true)
            assertFalse(state.kanbanColumns[key]?.loading ?: true)
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

    /** Holds the first thread list read open until [gate] completes; every
     *  later read answers at once with one message fewer on the card. */
    private class GatedCore : MeronCore {
        val gate = CompletableDeferred<Unit>()
        var threadListCalls = 0

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
                    if (threadListCalls == 1) {
                        gate.await()
                        card(messageCount = 3, hasDraft = true)
                    } else {
                        card(messageCount = 2, hasDraft = false)
                    }
                }

                else -> {
                    "{}"
                }
            }

        private fun card(
            messageCount: Int,
            hasDraft: Boolean,
        ): String =
            """{"threads":[{"id":"a:INBOX:t1","account_id":"a","folder_id":"INBOX","subject":"Re: hello",""" +
                """"message_count":$messageCount,"has_draft":$hasDraft,"date":1}]}"""

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
