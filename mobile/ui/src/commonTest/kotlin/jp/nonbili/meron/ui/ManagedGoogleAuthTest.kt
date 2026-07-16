package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.CloseableHandle
import jp.nonbili.meron.shared.CoreEvent
import jp.nonbili.meron.shared.CoreEventStream
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.MobileCommand
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.ThreadActionParams
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

private const val OAUTH_FAILURE_MESSAGE =
    "oauth login failed: no response: code: None, " +
        "info: Some(\\\"[AUTHENTICATIONFAILED] Invalid credentials (Failure)\\\")"

class ManagedGoogleAuthTest {
    @Test
    fun retriesOnceAfterForceMintWhenServerRejectsOAuthPayload() {
        val core = ScriptedCore(failDeletes = 1)
        val host = ManagedHost()
        val state = testState(core, host)

        val response = runAuthWrapped(state, core)

        assertTrue(response.contains("result"))
        assertEquals(
            listOf(MobileCommand.Delete, MobileCommand.AccountUpdateOAuthToken, MobileCommand.Delete),
            core.commands,
        )
        assertEquals(1, host.forceRefreshes)
        assertNull(state.googleReauthAccountId)
    }

    @Test
    fun retriesOnceWhenCoreThrowsOAuthFailure() {
        val core = ScriptedCore(failDeletes = 1, throwOnFailure = true)
        val host = ManagedHost()
        val state = testState(core, host)

        val response = runAuthWrapped(state, core)

        assertTrue(response.contains("result"))
        assertEquals(
            listOf(MobileCommand.Delete, MobileCommand.AccountUpdateOAuthToken, MobileCommand.Delete),
            core.commands,
        )
        assertEquals(1, host.forceRefreshes)
    }

    @Test
    fun rethrowsWhenRetryFailsToo() {
        val core = ScriptedCore(failDeletes = 2, throwOnFailure = true)
        val host = ManagedHost()
        val state = testState(core, host)

        val failure = assertFailsWith<RuntimeException> { runAuthWrapped(state, core) }

        assertTrue(failure.message.orEmpty().contains("oauth login failed"))
        // One force mint, one retry — no retry loop.
        assertEquals(1, host.forceRefreshes)
        assertEquals(
            listOf(MobileCommand.Delete, MobileCommand.AccountUpdateOAuthToken, MobileCommand.Delete),
            core.commands,
        )
    }

    @Test
    fun doesNotRetryNonAuthErrors() {
        val core = ScriptedCore(failDeletes = 1, failureMessage = "delete failed: no such folder")
        val host = ManagedHost()
        val state = testState(core, host)

        val response = runAuthWrapped(state, core)

        assertTrue(response.contains("no such folder"))
        assertEquals(listOf(MobileCommand.Delete), core.commands)
        assertEquals(0, host.forceRefreshes)
    }

    @Test
    fun surfacesReconnectWhenForceMintFails() {
        val core = ScriptedCore(failDeletes = 1)
        val host = ManagedHost(failForceMint = true)
        val state = testState(core, host)

        val response = runAuthWrapped(state, core)

        // Original auth error comes back untouched; no retry without a token.
        assertTrue(response.contains("oauth login failed"))
        assertEquals(listOf(MobileCommand.Delete), core.commands)
        assertEquals("acc1", state.googleReauthAccountId)
        assertNotNull(state.errorBanner)
    }

    @Test
    fun skipsTokenPushWhileFreshAndRunsActionUnchanged() {
        val core = ScriptedCore(failDeletes = 0)
        val host = ManagedHost()
        val state = testState(core, host)

        runAuthWrapped(state, core)

        // Pre-flight saw StillFresh: no account.updateOAuthToken traffic.
        assertEquals(listOf(MobileCommand.Delete), core.commands)
        assertEquals(1, host.softRefreshes)
        assertEquals(0, host.forceRefreshes)
    }

    // The wrapper suspends only on synchronous fakes, so an Unconfined launch
    // runs it to completion before returning.
    private fun runAuthWrapped(
        state: MeronMobileState,
        core: ScriptedCore,
    ): String {
        var response: String? = null
        var failure: Throwable? = null
        CoroutineScope(Dispatchers.Unconfined).launch {
            try {
                val client = MobileMailCommandClient(core)
                response = state.withManagedGoogleAuth(client, "acc1") { client.invokeDelete() }
            } catch (ex: Throwable) {
                failure = ex
            }
        }
        failure?.let { throw it }
        return assertNotNull(response, "wrapper did not complete synchronously")
    }

    private suspend fun MobileMailCommandClient.invokeDelete(): String = delete(ThreadActionParams(threadId = "acc1#INBOX#1", folderId = "INBOX"))

    private fun testState(
        core: MeronCore,
        host: MobileHost,
    ) = MeronMobileState(
        scope = CoroutineScope(EmptyCoroutineContext),
        core = core,
        coreLoaded = true,
        prefs = MemoryPreferences(),
        kanbanPrefs = MemoryPreferences(),
        services = NoopPlatformServices(),
        locale = NoopLocaleController(),
        mobileHost = host,
    )

    /** Managed-account host: soft refreshes report StillFresh, force mints a token. */
    private class ManagedHost(
        private val failForceMint: Boolean = false,
    ) : DefaultMobileHost() {
        var softRefreshes = 0
        var forceRefreshes = 0

        override val supportsGoogleDeviceAuth: Boolean = true

        override suspend fun refreshManagedGoogleToken(
            accountId: String,
            force: Boolean,
        ): ManagedTokenRefresh =
            if (force) {
                forceRefreshes++
                if (failForceMint) {
                    ManagedTokenRefresh.Failed
                } else {
                    ManagedTokenRefresh.Refreshed(accessToken = "fresh-token", expiresAtEpochSeconds = 4_102_444_800)
                }
            } else {
                softRefreshes++
                ManagedTokenRefresh.StillFresh
            }
    }

    /** Fails the first [failDeletes] mail.delete calls, as an error payload or a throw. */
    private class ScriptedCore(
        private var failDeletes: Int,
        private val throwOnFailure: Boolean = false,
        private val failureMessage: String = OAUTH_FAILURE_MESSAGE,
    ) : MeronCore {
        val commands = mutableListOf<String>()

        override suspend fun invoke(
            command: String,
            payloadJson: String,
        ): String {
            commands += command
            if (command == MobileCommand.Delete && failDeletes > 0) {
                failDeletes--
                if (throwOnFailure) {
                    throw RuntimeException(failureMessage.replace("\\\"", "\""))
                }
                return """{"id":1,"error":{"message":"$failureMessage"}}"""
            }
            return """{"id":1,"result":{}}"""
        }

        override fun events(): CoreEventStream =
            object : CoreEventStream {
                override fun subscribe(listener: (CoreEvent) -> Unit): CloseableHandle = CloseableHandle {}
            }

        override suspend fun protocolVersion(): Int = 0
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

    private class MemoryPreferences : AppPreferences {
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

    private class NoopLocaleController : LocaleController {
        override fun currentLanguageTag(): String = ""

        override fun apply(tag: String) {}

        override fun displayName(tag: String): String = tag
    }
}
