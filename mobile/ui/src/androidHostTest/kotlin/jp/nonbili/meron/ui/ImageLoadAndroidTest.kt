package jp.nonbili.meron.ui

import java.io.File
import java.nio.file.Files
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

class ImageLoadAndroidTest {
    @Test
    fun androidMediaRootDirectoryKeepsAccountMediaAndAttachmentsSeparate() {
        assertEquals("media", androidMediaRootDirectoryName("avatars/account/avatar.png"))
        assertEquals("media", androidMediaRootDirectoryName("wallpapers/account/wallpaper.webp"))
        assertEquals("attachments", androidMediaRootDirectoryName("account/inbox/1/0.png"))
        assertEquals("attachments", androidMediaRootDirectoryName("rss-account/sub/item/0.jpg"))
    }

    @Test
    fun remoteImageCacheKeyIsStableAndUrlScoped() {
        val first = remoteImageCacheKey("https://example.com/image.png?a=1")
        val second = remoteImageCacheKey("https://example.com/image.png?a=1")
        val other = remoteImageCacheKey("https://example.com/image.png?a=2")

        assertEquals(64, first.length)
        assertEquals(first, second)
        assertNotEquals(first, other)
        assertTrue(first.all { it in '0'..'9' || it in 'a'..'f' })
    }

    @Test
    fun cachedRemoteImageFileFindsNewestImageForUrl() {
        withTempDir("meron-image-cache-find") { dir ->
            val key = remoteImageCacheKey("https://example.com/photo")
            File(dir, "$key.txt").writeText("not-image")
            val old = File(dir, "$key.jpg").apply {
                writeBytes(byteArrayOf(1))
                setLastModified(100)
            }
            val newest = File(dir, "$key.webp").apply {
                writeBytes(byteArrayOf(2))
                setLastModified(200)
            }

            assertEquals(newest, cachedRemoteImageFile(dir, "https://example.com/photo"))
            assertTrue(old.exists())
            assertNull(cachedRemoteImageFile(dir, "https://example.com/other"))
        }
    }

    @Test
    fun pruneRemoteImageCacheDeletesOldestFilesFirst() {
        withTempDir("meron-image-cache-prune") { dir ->
            val oldest = File(dir, "old.png").apply {
                writeBytes(ByteArray(60))
                setLastModified(100)
            }
            val middle = File(dir, "middle.png").apply {
                writeBytes(ByteArray(50))
                setLastModified(200)
            }
            val newest = File(dir, "new.png").apply {
                writeBytes(ByteArray(40))
                setLastModified(300)
            }

            pruneRemoteImageCache(dir, maxBytes = 90)

            assertFalse(oldest.exists())
            assertTrue(middle.exists())
            assertTrue(newest.exists())
            assertTrue(dir.listFiles().orEmpty().sumOf { it.length() } <= 90)
        }
    }
}

private fun withTempDir(
    prefix: String,
    block: (File) -> Unit,
) {
    val dir = Files.createTempDirectory(prefix).toFile()
    try {
        block(dir)
    } finally {
        dir.deleteRecursively()
    }
}
