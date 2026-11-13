package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.AccountSummary
import kotlinx.coroutines.CancellationException
import kotlin.coroutines.Continuation
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.coroutines.startCoroutine
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull

class MobileConnectivityTest {
    private val accounts =
        listOf(
            AccountSummary(id = "one", displayName = "Alex Doe", email = "ping.one@example.com"),
            AccountSummary(id = "two", displayName = "Alex Doe", email = "ping.two@example.com"),
            AccountSummary(id = "three", displayName = "Ada", email = "ada@example.com"),
        )

    @Test
    fun duplicateAccountNamesIncludeEmailAddress() {
        assertEquals("Alex Doe (ping.one@example.com)", mobileConnectivityAccountLabel("one", accounts))
        assertEquals("Alex Doe (ping.two@example.com)", mobileConnectivityAccountLabel("two", accounts))
        assertEquals("Ada", mobileConnectivityAccountLabel("three", accounts))
    }

    @Test
    fun accountLabelFallsBackSafely() {
        assertEquals("missing", mobileConnectivityAccountLabel("missing", accounts))
        assertNull(mobileConnectivityAccountLabel(null, accounts))
    }

    @Test
    fun extractsProxyEndpointsFromCoreErrors() {
        assertEquals(
            "127.0.0.1:1",
            mobileProxyEndpointFromSyncError(
                "sync inbox: connect to proxy 127.0.0.1:1: tcp connect: Connection refused",
            ),
        )
        assertEquals(
            "proxy.example:1080",
            mobileProxyEndpointFromSyncError("socks5 proxy proxy.example:1080 to imap.example:993: timed out"),
        )
        assertEquals(
            "::1:9050",
            mobileProxyEndpointFromSyncError("connect to proxy ::1:9050: Connection refused"),
        )
        assertNull(mobileProxyEndpointFromSyncError("connect imap.example:993: timed out"))
    }

    @Test
    fun accountContextPreservesFailureAndCancellation() {
        val failure =
            assertFailsWith<AccountSyncException> {
                runSuspend {
                    withSyncAccountContext("one") {
                        throw IllegalStateException("proxy failed")
                    }
                }
            }
        assertEquals("one", failure.accountId)
        assertEquals("proxy failed", failure.message)

        assertFailsWith<CancellationException> {
            runSuspend {
                withSyncAccountContext("one") {
                    throw CancellationException("cancelled")
                }
            }
        }
    }

    private fun <T> runSuspend(block: suspend () -> T): T {
        var completed: Result<T>? = null
        block.startCoroutine(
            object : Continuation<T> {
                override val context = EmptyCoroutineContext

                override fun resumeWith(result: Result<T>) {
                    completed = result
                }
            },
        )
        return checkNotNull(completed).getOrThrow()
    }
}
