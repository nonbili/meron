package jp.nonbili.meron

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.core.content.FileProvider
import androidx.test.core.app.ActivityScenario
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File

@RunWith(AndroidJUnit4::class)
class MailtoIntentContractTest {
    @Test
    fun mailtoIntentPrefillsComposeFields() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val intent =
            Intent(
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

    @Test
    fun sendTextIntentPrefillsComposeBody() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val intent =
            Intent(Intent.ACTION_SEND)
                .setType("text/plain")
                .putExtra(Intent.EXTRA_SUBJECT, "Shared subject")
                .putExtra(Intent.EXTRA_TEXT, "Shared body")
                .setClass(context, ComposeMainActivity::class.java)

        ActivityScenario.launch<ComposeMainActivity>(intent).use {
            it.onActivity { activity ->
                val draft = activity.currentMailtoDraftForTesting()
                assertEquals("Shared subject", draft?.subject)
                assertEquals("Shared body", draft?.body)
            }
        }
    }

    @Test
    fun sendFileIntentAddsComposeAttachment() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val file =
            File(context.cacheDir, "attachments/shared.txt")
                .apply {
                    parentFile?.mkdirs()
                    writeText("shared attachment")
                }
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
        val intent =
            Intent(Intent.ACTION_SEND)
                .setType("text/plain")
                .putExtra(Intent.EXTRA_STREAM, uri)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                .setClass(context, ComposeMainActivity::class.java)

        ActivityScenario.launch<ComposeMainActivity>(intent).use {
            it.onActivity { activity ->
                val attachment = activity.currentMailtoDraftForTesting()?.attachments?.single()
                assertEquals("shared.txt", attachment?.displayName)
                assertEquals("text/plain", attachment?.mimeType)
                assertEquals("shared attachment".length.toLong(), attachment?.sizeBytes)
                assertTrue(attachment?.dataBase64?.isNotBlank() == true)
            }
        }
    }
}
