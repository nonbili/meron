package jp.nonbili.meron

import android.content.Intent
import android.net.Uri
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

class OpenFileIntentContractTest {
    @Test
    fun attachmentUsesViewIntentWithReadPermission() {
        val uri = Uri.parse("content://jp.nonbili.meron.fileprovider/attachment_cache/hiking_map.pdf")
        val intent = openFileIntent(uri, "application/pdf")

        assertEquals(Intent.ACTION_VIEW, intent.action)
        assertEquals(uri, intent.data)
        assertEquals("application/pdf", intent.type)
        assertTrue(intent.flags and Intent.FLAG_GRANT_READ_URI_PERMISSION != 0)
        assertNotNull(intent.clipData)
    }

    @Test
    fun saveUsesCreateDocumentIntent() {
        val intent = createDocumentIntent("hiking_map.pdf", "application/pdf")

        assertEquals(Intent.ACTION_CREATE_DOCUMENT, intent.action)
        assertTrue(intent.hasCategory(Intent.CATEGORY_OPENABLE))
        assertEquals("application/pdf", intent.type)
        assertEquals("hiking_map.pdf", intent.getStringExtra(Intent.EXTRA_TITLE))
    }

    @Test
    fun executableAndroidPackagesCannotBeOpened() {
        assertTrue(!isSafeAttachmentToOpen("update.apk", "application/octet-stream"))
        assertTrue(!isSafeAttachmentToOpen("update.bin", "application/vnd.android.package-archive"))
        assertTrue(isSafeAttachmentToOpen("hiking_map.pdf", "application/pdf"))
    }

    @Test
    fun externalViewIntentIsBrowsable() {
        val intent = externalViewIntent("zoommtg://zoom.us/join")
        assertEquals(Intent.ACTION_VIEW, intent.action)
        assertTrue(intent.hasCategory(Intent.CATEGORY_BROWSABLE))
    }
}
