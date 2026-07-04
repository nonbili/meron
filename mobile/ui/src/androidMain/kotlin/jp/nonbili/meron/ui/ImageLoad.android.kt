package jp.nonbili.meron.ui

import android.content.Context
import android.net.Uri
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.net.URL

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
                    val root = File(ctx.filesDir, "media").canonicalFile
                    val file = File(root, relative).canonicalFile
                    if (!file.startsWith(root)) return@withContext null
                    file.readBytes()
                }

                ref.startsWith("content://") -> {
                    val ctx = androidImageContext ?: return@withContext null
                    ctx.contentResolver.openInputStream(Uri.parse(ref))?.use { it.readBytes() }
                }

                ref.startsWith("http://") || ref.startsWith("https://") -> {
                    URL(ref).openStream().use { it.readBytes() }
                }

                else -> {
                    File(ref).readBytes()
                }
            }
        }.getOrNull()
    }
