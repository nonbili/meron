package jp.nonbili.meron

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.test.core.app.ActivityScenario
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class MailtoIntentContractTest {
    @Test
    fun mailtoIntentPrefillsComposeFields() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val intent = Intent(
            Intent.ACTION_VIEW,
            Uri.parse(
                "mailto:to@example.com?cc=cc@example.com&bcc=bcc@example.com" +
                    "&subject=Mobile%20Draft&body=Line%20one%0ALine%20two",
            ),
        ).setClass(context, ComposeMainActivity::class.java)

        ActivityScenario.launch<ComposeMainActivity>(intent).use {
            it.onActivity { activity ->
                val draft = activity.currentMailtoDraftForTesting()
                assertEquals("to@example.com", draft?.to)
                assertEquals("cc@example.com", draft?.cc)
                assertEquals("bcc@example.com", draft?.bcc)
                assertEquals("Mobile Draft", draft?.subject)
                assertEquals("Line one\nLine two", draft?.body)
            }
        }
    }
}
