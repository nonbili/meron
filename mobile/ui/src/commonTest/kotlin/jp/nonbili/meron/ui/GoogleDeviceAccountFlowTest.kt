package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.CloseableHandle
import jp.nonbili.meron.shared.CoreEvent
import jp.nonbili.meron.shared.CoreEventStream
import jp.nonbili.meron.shared.MeronCore
import kotlinx.coroutines.CoroutineScope
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class GoogleDeviceAccountFlowTest {
    @Test
    fun cancelledDeviceAuthDoesNotOpenBrowserFallback() {
        val services = FakePlatformServices()
        val state =
            testState(
                services = services,
                mobileHost =
                    FakeMobileHost(
                        result = GoogleDeviceAccountResult.Cancelled,
                        googleRedirectUri = "com.googleusercontent.apps.test:/oauth2redirect",
                    ),
            )

        state.connectGoogleDeviceAccount()

        assertFalse(services.openedOAuth)
        assertEquals("Google sign-in cancelled.", state.status)
    }

    @Test
    fun failedDeviceAuthOpensBrowserFallbackWhenRedirectIsConfigured() {
        val services = FakePlatformServices()
        val state =
            testState(
                services = services,
                mobileHost =
                    FakeMobileHost(
                        result = GoogleDeviceAccountResult.Failed("Device auth failed."),
                        googleRedirectUri = "com.googleusercontent.apps.test:/oauth2redirect",
                    ),
            )

        state.connectGoogleDeviceAccount()

        assertTrue(services.openedOAuth)
        assertEquals("Opened Gmail sign-in", state.status)
    }

    @Test
    fun failedDeviceAuthWithoutRedirectDoesNotOpenBrowserFallback() {
        val services = FakePlatformServices()
        val state =
            testState(
                services = services,
                mobileHost =
                    FakeMobileHost(
                        result = GoogleDeviceAccountResult.Failed("Device auth failed."),
                        googleRedirectUri = "",
                    ),
            )

        state.connectGoogleDeviceAccount()

        assertFalse(services.openedOAuth)
        assertEquals(
            "Device auth failed. Google browser sign-in requires a configured HTTPS redirect URI.",
            state.status,
        )
    }

    private fun testState(
        services: PlatformServices,
        mobileHost: MobileHost,
    ) = MeronMobileState(
        scope = CoroutineScope(EmptyCoroutineContext),
        core = FakeCore(),
        coreLoaded = true,
        prefs = FakePreferences(),
        kanbanPrefs = FakePreferences(),
        services = services,
        locale = FakeLocaleController(),
        mobileHost = mobileHost,
    )

    private class FakeMobileHost(
        private val result: GoogleDeviceAccountResult,
        override val googleClientId: String = "google-client-id",
        override val googleRedirectUri: String = "",
    ) : DefaultMobileHost(
            googleClientId = googleClientId,
            googleRedirectUri = googleRedirectUri,
        ) {
        override val supportsGoogleDeviceAuth: Boolean = true

        override fun connectGoogleDeviceAccount(onResult: (GoogleDeviceAccountResult) -> Unit) {
            onResult(result)
        }
    }

    private class FakePlatformServices : PlatformServices {
        var openedOAuth = false

        override fun openUrl(url: String) {}

        override fun openOAuthUrl(
            url: String,
            callbackScheme: String,
            onCallback: (String) -> Unit,
            onFailure: (String) -> Unit,
        ) {
            openedOAuth = true
        }

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
        override fun currentLanguageTag(): String = ""

        override fun apply(tag: String) {}

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
