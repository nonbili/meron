package jp.nonbili.meron.ui

import android.content.Context
import android.net.Uri
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL

private const val REMOTE_IMAGE_CACHE_MAX_BYTES = 1024L * 1024L * 1024L
private const val REMOTE_IMAGE_CONNECT_TIMEOUT_MS = 10_000
private const val REMOTE_IMAGE_READ_TIMEOUT_MS = 15_000

/** Set by the Android host at startup so image refs that need app storage
 *  (the `/media/` dir, `content://`) can resolve. */
var androidImageContext: Context? = null

actual suspend fun loadImageBytes(ref: String): ByteArray? =
    withContext(Dispatchers.IO) {
        runCatching {
            when {
                ref.startsWith("/media/") -> {
                    val ctx = androidImageContext ?: return@withContext null
                    val relative = ref.removePrefix("/media/").trimStart('/')
                    if (relative.isBlank() || relative.split('/').any { it == ".." || it.isBlank() }) return@withContext null
                    val root = androidMediaRoot(ctx, relative)
                    val file = File(root, relative).canonicalFile
                    if (!file.startsWith(root)) return@withContext null
                    file.readBytes()
                }

                ref.startsWith("content://") -> {
                    val ctx = androidImageContext ?: return@withContext null
                    ctx.contentResolver.openInputStream(Uri.parse(ref))?.use { it.readBytes() }
                }

                ref.startsWith("http://") || ref.startsWith("https://") -> {
                    loadRemoteImageBytes(ref, androidImageContext)
                }

                else -> {
                    File(ref).readBytes()
                }
            }
        }.getOrNull()
    }

internal fun androidMediaRoot(
    ctx: Context,
    relative: String,
): File =
    File(
        ctx.filesDir,
        androidMediaRootDirectoryName(relative),
    ).canonicalFile

internal fun androidMediaRootDirectoryName(relative: String): String = if (relative.startsWith("avatars/") || relative.startsWith("wallpapers/")) "media" else "attachments"

internal fun remoteImageCacheKey(url: String): String = sha256Hex(url)

internal fun cachedRemoteImageFile(
    cacheDir: File,
    url: String,
): File? {
    val prefix = "${remoteImageCacheKey(url)}."
    return cacheDir
        .listFiles { file -> file.isFile && file.name.startsWith(prefix) && imageExtensionFromFileName(file.name) != null }
        ?.maxByOrNull { it.lastModified() }
}

internal fun pruneRemoteImageCache(
    cacheDir: File,
    maxBytes: Long = REMOTE_IMAGE_CACHE_MAX_BYTES,
) {
    val files = cacheDir.listFiles { file -> file.isFile }?.toList().orEmpty()
    var total = files.sumOf { it.length().coerceAtLeast(0L) }
    if (total <= maxBytes) return
    for (file in files.sortedBy { it.lastModified() }) {
        val size = file.length().coerceAtLeast(0L)
        if (file.delete()) total -= size
        if (total <= maxBytes) break
    }
}

private fun loadRemoteImageBytes(
    url: String,
    context: Context?,
): ByteArray? {
    val cacheDir = context?.cacheDir?.let { File(it, "image-cache") }
    if (cacheDir != null) {
        cacheDir.mkdirs()
        cachedRemoteImageFile(cacheDir, url)?.let { file ->
            val bytes =
                try {
                    file.readBytes()
                } catch (_: IOException) {
                    null
                }
            if (bytes != null && bytes.isNotEmpty()) {
                file.setLastModified(System.currentTimeMillis())
                return bytes
            }
            file.delete()
        }
    }

    val connection =
        (URL(url).openConnection() as HttpURLConnection).apply {
            connectTimeout = REMOTE_IMAGE_CONNECT_TIMEOUT_MS
            readTimeout = REMOTE_IMAGE_READ_TIMEOUT_MS
            instanceFollowRedirects = true
        }
    return try {
        if (connection.responseCode !in 200..299) return null
        val bytes = connection.inputStream.use { it.readBytes() }
        if (bytes.isEmpty()) return null
        val ext = sniffImageExtension(bytes, connection.contentType)
        if (cacheDir != null && ext != null) {
            cacheDir.mkdirs()
            val target = File(cacheDir, "${remoteImageCacheKey(url)}.$ext")
            val tmp = File(cacheDir, "${target.name}.${System.nanoTime()}.tmp")
            tmp.writeBytes(bytes)
            if (!tmp.renameTo(target)) {
                tmp.copyTo(target, overwrite = true)
                tmp.delete()
            }
            target.setLastModified(System.currentTimeMillis())
            pruneRemoteImageCache(cacheDir)
        }
        bytes
    } finally {
        connection.disconnect()
    }
}

private fun sniffImageExtension(
    bytes: ByteArray,
    contentType: String?,
): String? {
    val fromContentType =
        when (contentType?.substringBefore(';')?.trim()?.lowercase()) {
            "image/jpeg", "image/jpg" -> "jpg"
            "image/png" -> "png"
            "image/gif" -> "gif"
            "image/webp" -> "webp"
            "image/svg+xml" -> "svg"
            else -> null
        }
    if (fromContentType != null) return fromContentType
    return when {
        bytes.size >= 3 && bytes[0] == 0xFF.toByte() && bytes[1] == 0xD8.toByte() && bytes[2] == 0xFF.toByte() -> "jpg"
        bytes.size >= 8 && bytes.copyOfRange(0, 8).contentEquals(byteArrayOf(0x89.toByte(), 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A)) -> "png"
        bytes.size >= 6 && (bytes.decodeHead(6) == "GIF87a" || bytes.decodeHead(6) == "GIF89a") -> "gif"
        bytes.size >= 12 && bytes.decodeHead(4) == "RIFF" && bytes.copyOfRange(8, 12).decodeToString() == "WEBP" -> "webp"
        bytes.decodeHead(512).trimStart().let { it.startsWith("<svg") || (it.startsWith("<?xml") && it.contains("<svg")) } -> "svg"
        else -> null
    }
}

private fun imageExtensionFromFileName(name: String): String? = name.substringAfterLast('.', "").lowercase().takeIf { it in setOf("jpg", "png", "gif", "webp", "svg") }

private fun ByteArray.decodeHead(maxBytes: Int): String = copyOfRange(0, size.coerceAtMost(maxBytes)).decodeToString()
