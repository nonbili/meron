package jp.nonbili.meron.ui

import android.graphics.BitmapFactory
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap

/**
 * Bounds on a decoded image. A hardware canvas refuses to draw a bitmap over
 * 100 MB and takes the whole app down with it ("Canvas: trying to draw too
 * large bitmap"), which a single oversized picture in a feed entry or a mail
 * is enough to trigger: 12 megapixels is 48 MB at four bytes a pixel. The side
 * limit covers pictures that are small in area but absurdly long or wide.
 */
private const val MAX_DECODED_PIXELS = 12_000_000L
private const val MAX_DECODED_SIDE = 8192

/**
 * How much to shrink a [width] by [height] image while decoding so it stays
 * within the bounds above: 1 for none, otherwise a power of two, which is all
 * the decoder accepts. Unknown dimensions (a file that is not an image) get 1
 * and fail in the decoder as before.
 */
internal fun imageSampleSize(
    width: Int,
    height: Int,
): Int {
    if (width <= 0 || height <= 0) return 1
    var sample = 1
    while (
        (width.toLong() / sample) * (height.toLong() / sample) > MAX_DECODED_PIXELS ||
        maxOf(width, height) / sample > MAX_DECODED_SIDE
    ) {
        sample *= 2
    }
    return sample
}

actual fun decodeImageBitmap(bytes: ByteArray): ImageBitmap? =
    runCatching {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
        val options = BitmapFactory.Options().apply { inSampleSize = imageSampleSize(bounds.outWidth, bounds.outHeight) }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options)?.asImageBitmap()
    }.getOrNull()
