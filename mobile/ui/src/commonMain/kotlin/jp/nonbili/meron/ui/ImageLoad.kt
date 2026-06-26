package jp.nonbili.meron.ui

import androidx.compose.ui.graphics.ImageBitmap

/** Load encoded image bytes for an avatar/media reference. Supports `/media/...`
 *  (app media dir), `content://`, `file://`/paths, and `http(s)://`. Returns null
 *  on any failure (caller falls back to a generated avatar). */
expect suspend fun loadImageBytes(ref: String): ByteArray?

internal suspend fun loadImageBitmapRef(ref: String): ImageBitmap? = loadImageBytes(ref)?.let { decodeImageBitmap(it) }

internal suspend fun loadFirstImageBitmap(refs: List<String>): ImageBitmap? {
    for (ref in refs) loadImageBitmapRef(ref)?.let { return it }
    return null
}
