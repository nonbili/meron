package jp.nonbili.meron.ui

import kotlinx.coroutines.runBlocking
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class RemoteImageAttachmentTest {
    @Test
    fun htmlImageUsesUrlFilenameWithoutQueryOrFragment() {
        val attachment = remoteHtmlImageAttachment("https://example.com/photos/picture.webp?width=800#preview")
        assertEquals("picture.webp", attachment.filename)
        assertEquals("image/webp", attachment.mimeType)
        assertEquals("picture.webp", safeAttachmentFilename(attachment))
        assertEquals("image", remoteHtmlImageAttachment("https://example.com/").filename)
    }

    @Test
    fun remoteImageLoadsBytesWithoutACoreCacheKey() =
        runBlocking {
            val attachment = remoteHtmlImageAttachment("https://example.com/photo.png")
            val bytes = byteArrayOf(0x89.toByte(), 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a)
            val result =
                readRemoteAttachmentBytes(attachment) { url ->
                    assertEquals(attachment.url, url)
                    bytes
                }
            assertContentEquals(bytes, result)
        }

    @Test
    fun infersRealImageFormatAndFilename() {
        val bytes = byteArrayOf(0xff.toByte(), 0xd8.toByte(), 0xff.toByte())
        val image = normalizedRemoteImage(remoteHtmlImageAttachment("https://example.com/my%20photo?size=2"), bytes)
        assertEquals("my photo.jpg", image.filename)
        assertEquals("image/jpeg", image.mimeType)
        assertEquals(null, remoteImageFormat("<html>error</html>".encodeToByteArray()))
    }

    @Test
    fun webpSignatureUsesByteOffsetsNotUtf8CharacterOffsets() {
        val bytes = "RIFF".encodeToByteArray() + byteArrayOf(0xc3.toByte(), 0xa9.toByte(), 0, 0) + "WEBP".encodeToByteArray()
        assertEquals("image/webp" to "webp", remoteImageFormat(bytes))
    }

    @Test
    fun failedOrEmptyDownloadsAreReportedAsErrors(): Unit =
        runBlocking {
            val attachment = remoteHtmlImageAttachment("https://example.com/photo.png")
            assertFailsWith<IllegalStateException> { readRemoteAttachmentBytes(attachment) { null } }
            assertFailsWith<IllegalStateException> { readRemoteAttachmentBytes(attachment) { byteArrayOf() } }
            assertFailsWith<IllegalArgumentException> {
                readRemoteAttachmentBytes(attachment.copy(url = "file:///private/photo.png")) {
                    error("Must not load a non-HTTP URL")
                }
            }
        }
}
