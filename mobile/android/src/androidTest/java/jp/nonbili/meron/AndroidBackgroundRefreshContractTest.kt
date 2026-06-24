package jp.nonbili.meron

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class AndroidBackgroundRefreshContractTest {
    @Test
    fun mailAccountBuildsMailSyncRequestForInboxRefresh() {
        val request =
            androidRefreshSyncRequest(
                JSONObject(
                    mapOf(
                        "id" to "mail-account",
                        "engine" to "mail",
                        "provider" to "gmail",
                        "auth_type" to "gmail_oauth",
                    ),
                ),
                id = 7,
            )

        val requestJson = JSONObject(request?.requestJson.orEmpty())
        val params = requestJson.getJSONObject("params")

        assertEquals("mail.sync", request?.method)
        assertEquals(7, requestJson.getLong("id"))
        assertEquals("mail.sync", requestJson.getString("method"))
        assertEquals("mail-account", params.getString("account_id"))
        assertEquals("inbox", params.getString("folder_id"))
        assertEquals(50, params.getInt("limit"))
        assertEquals(true, params.getBoolean("folders"))
    }

    @Test
    fun rssAccountBuildsRssSyncRequest() {
        val request =
            androidRefreshSyncRequest(
                JSONObject(
                    mapOf(
                        "id" to "rss-account",
                        "engine" to "rss",
                        "provider" to "custom",
                        "auth_type" to "password",
                    ),
                ),
                id = 8,
            )

        val requestJson = JSONObject(request?.requestJson.orEmpty())
        val params = requestJson.getJSONObject("params")

        assertEquals("rss.sync", request?.method)
        assertEquals(8, requestJson.getLong("id"))
        assertEquals("rss.sync", requestJson.getString("method"))
        assertEquals("rss-account", params.getString("account_id"))
        assertEquals(false, params.has("folder_id"))
    }

    @Test
    fun pausedOrDisconnectedAccountsAreSkipped() {
        assertNull(
            androidRefreshSyncRequest(
                JSONObject(mapOf("id" to "mail-account", "paused" to true)),
                id = 9,
            ),
        )
        assertNull(
            androidRefreshSyncRequest(
                JSONObject(mapOf("id" to "mail-account", "needs_reconnect" to true)),
                id = 10,
            ),
        )
        assertNull(androidRefreshSyncRequest(JSONObject(mapOf("id" to "")), id = 11))
    }
}
