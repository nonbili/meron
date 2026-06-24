package jp.nonbili.meron

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.test.core.app.ActivityScenario
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import jp.nonbili.meron.shared.defaultOAuthRedirectUri
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class OAuthCallbackIntentContractTest {
    @Test
    fun oauthCallbackIntentIsCapturedByActivity() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val callbackUrl = "jp.nonbili.meron.oauth://oauth?code=mobile-code&state=state-123"
        val intent =
            Intent(Intent.ACTION_VIEW, Uri.parse(callbackUrl))
                .setClass(context, ComposeMainActivity::class.java)

        ActivityScenario.launch<ComposeMainActivity>(intent).use {
            it.onActivity { activity ->
                assertEquals(callbackUrl, activity.currentOAuthCallbackUrlForTesting())
            }
        }
    }

    @Test
    fun appLinkStyleOAuthCallbackIntentIsCapturedByActivity() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val callbackUrl = "https://oauth.example.invalid/meron/oauth?code=mobile-code&state=state-123"
        val intent =
            Intent(Intent.ACTION_VIEW, Uri.parse(callbackUrl))
                .setClass(context, ComposeMainActivity::class.java)

        ActivityScenario.launch<ComposeMainActivity>(intent).use {
            it.onActivity { activity ->
                assertEquals(callbackUrl, activity.currentOAuthCallbackUrlForTesting())
            }
        }
    }

    @Test
    fun manifestRegistersPlaceholderHttpsAppLinkRedirect() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val redirectUri = Uri.parse("https://oauth.example.invalid/meron/oauth?code=abc&state=expected")
        val intent =
            Intent(Intent.ACTION_VIEW, redirectUri)
                .addCategory(Intent.CATEGORY_BROWSABLE)
                .setPackage(context.packageName)

        val matches = context.packageManager.queryIntentActivities(intent, 0)

        assertTrue(
            matches.any { it.activityInfo.name == ComposeMainActivity::class.java.name },
        )
    }

    @Test
    fun manifestRegistersSharedOAuthRedirectUri() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val redirectUri = Uri.parse(defaultOAuthRedirectUri())
        val intent =
            Intent(Intent.ACTION_VIEW, redirectUri)
                .addCategory(Intent.CATEGORY_BROWSABLE)
                .setPackage(context.packageName)

        val matches = context.packageManager.queryIntentActivities(intent, 0)

        assertEquals("jp.nonbili.meron.oauth", redirectUri.scheme)
        assertEquals("oauth", redirectUri.host)
        assertTrue(
            matches.any { it.activityInfo.name == ComposeMainActivity::class.java.name },
        )
    }
}
