package jp.nonbili.meron

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidBackgroundSyncTest {

    @Test
    fun testIsTransientNetworkError() {
        assertTrue(isTransientNetworkError("tcp connect: failed to lookup address information: No address associated with hostname"))
        assertTrue(isTransientNetworkError("dial tcp: lookup mail.example.com on 8.8.8.8:53: no such host"))
        assertTrue(isTransientNetworkError("i/o timeout"))
        assertTrue(isTransientNetworkError("connection timed out"))
        assertTrue(isTransientNetworkError("connection refused"))
        assertTrue(isTransientNetworkError("connection reset by peer"))
        assertTrue(isTransientNetworkError("network is unreachable"))
        assertTrue(isTransientNetworkError("oauth login failed: io: Software caused connection abort (os error 103)"))
        assertTrue(isTransientNetworkError("write: broken pipe"))

        assertFalse(isTransientNetworkError("invalid password"))
        assertFalse(isTransientNetworkError("authentication failed"))
        assertFalse(isTransientNetworkError("no such account"))
        assertFalse(isTransientNetworkError(""))
    }

    @Test
    fun testShouldNotifyRefreshComplete() {
        assertTrue(shouldNotifyRefreshComplete(manualRun = true, failed = 0))
        assertTrue(shouldNotifyRefreshComplete(manualRun = true, failed = 1))
        assertTrue(shouldNotifyRefreshComplete(manualRun = false, failed = 1))
        assertFalse(shouldNotifyRefreshComplete(manualRun = false, failed = 0))
    }
}
