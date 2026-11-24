package jp.nonbili.meron.ui

import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.ImageBitmapConfig
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

/** Load encoded image bytes for an avatar/media reference. Supports `/media/...`
 *  (app media dir), `content://`, `file://`/paths, and `http(s)://`. Returns null
 *  on any failure (caller falls back to a generated avatar). */
expect suspend fun loadImageBytes(ref: String): ByteArray?

// Remote actions require a loader that enforces the size limit while reading.
internal expect val boundedRemoteImageActions: Boolean

internal suspend fun loadImageBitmapRef(
    ref: String,
    maxBytes: Long = MAX_DECODED_IMAGE_BYTES,
): ImageBitmap? =
    // Keyed by budget too, so a picture first loaded small is not handed to a
    // full-size viewer, nor a full-size one to every avatar.
    loadCachedImageBitmap(if (maxBytes == MAX_DECODED_IMAGE_BYTES) ref else "$ref#$maxBytes") {
        loadImageBytes(ref)?.let { bytes ->
            withContext(ioDispatcher) { decodeImageBitmap(bytes, maxBytes) }
        }
    }

private const val IMAGE_BITMAP_CACHE_MAX_BYTES = 64L * 1024L * 1024L

private class CachedBitmap(
    val bitmap: ImageBitmap,
    val kind: String,
    val bytes: Long,
)

/** A cached bitmap with the loader's note on what it is (a sender image's
 *  "gravatar" or "favicon"); empty when the loader has nothing to say. */
internal class KindedImageBitmap(
    val bitmap: ImageBitmap,
    val kind: String = "",
)

private val imageBitmapCacheMutex = Mutex()
private val imageBitmapCache = LinkedHashMap<String, CachedBitmap>()
private var imageBitmapCacheBytes = 0L

/** Decoded [ImageBitmap]s are expensive to produce (disk/network read + decode) and a
 *  Composable's `remember` is discarded once a LazyColumn row scrolls out of the
 *  composition window, so scrolling back over the same media redoes that work from
 *  scratch. This cache keeps recently used bitmaps in memory (bounded by approximate
 *  pixel-buffer size) so re-entering the viewport is a cache hit. */
internal suspend fun loadCachedImageBitmap(
    key: String,
    load: suspend () -> ImageBitmap?,
): ImageBitmap? = loadCachedKindedImageBitmap(key) { load()?.let { KindedImageBitmap(it) } }?.bitmap

internal suspend fun loadCachedKindedImageBitmap(
    key: String,
    load: suspend () -> KindedImageBitmap?,
): KindedImageBitmap? {
    imageBitmapCacheMutex.withLock {
        imageBitmapCache.remove(key)?.let { cached ->
            imageBitmapCache[key] = cached
            return KindedImageBitmap(cached.bitmap, cached.kind)
        }
    }
    val loaded = load() ?: return null
    val bitmap = loaded.bitmap
    imageBitmapCacheMutex.withLock {
        // Half-float pixels (a 16-bit or wide-gamut source) take twice the room.
        val bytesPerPixel = if (bitmap.config == ImageBitmapConfig.F16) 8L else 4L
        val size = bitmap.width.toLong() * bitmap.height.toLong() * bytesPerPixel
        imageBitmapCache.remove(key)?.let { imageBitmapCacheBytes -= it.bytes }
        imageBitmapCache[key] = CachedBitmap(bitmap, loaded.kind, size)
        imageBitmapCacheBytes += size
        while (imageBitmapCacheBytes > IMAGE_BITMAP_CACHE_MAX_BYTES) {
            val eldestKey = imageBitmapCache.keys.firstOrNull() ?: break
            val eldest = imageBitmapCache.remove(eldestKey) ?: break
            imageBitmapCacheBytes -= eldest.bytes
        }
    }
    return loaded
}
