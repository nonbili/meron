package jp.nonbili.meron.ui

import androidx.compose.ui.graphics.ImageBitmap
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

/** Load encoded image bytes for an avatar/media reference. Supports `/media/...`
 *  (app media dir), `content://`, `file://`/paths, and `http(s)://`. Returns null
 *  on any failure (caller falls back to a generated avatar). */
expect suspend fun loadImageBytes(ref: String): ByteArray?

internal suspend fun loadImageBitmapRef(ref: String): ImageBitmap? =
    loadCachedImageBitmap(ref) {
        loadImageBytes(ref)?.let { bytes ->
            withContext(ioDispatcher) { decodeImageBitmap(bytes) }
        }
    }

internal suspend fun loadFirstImageBitmap(refs: List<String>): ImageBitmap? {
    for (ref in refs) loadImageBitmapRef(ref)?.let { return it }
    return null
}

private const val IMAGE_BITMAP_CACHE_MAX_BYTES = 64L * 1024L * 1024L

private class CachedBitmap(
    val bitmap: ImageBitmap,
    val bytes: Long,
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
): ImageBitmap? {
    imageBitmapCacheMutex.withLock {
        imageBitmapCache.remove(key)?.let { cached ->
            imageBitmapCache[key] = cached
            return cached.bitmap
        }
    }
    val bitmap = load() ?: return null
    imageBitmapCacheMutex.withLock {
        val size = bitmap.width.toLong() * bitmap.height.toLong() * 4L
        imageBitmapCache[key] = CachedBitmap(bitmap, size)
        imageBitmapCacheBytes += size
        while (imageBitmapCacheBytes > IMAGE_BITMAP_CACHE_MAX_BYTES) {
            val eldestKey = imageBitmapCache.keys.firstOrNull() ?: break
            val eldest = imageBitmapCache.remove(eldestKey) ?: break
            imageBitmapCacheBytes -= eldest.bytes
        }
    }
    return bitmap
}
