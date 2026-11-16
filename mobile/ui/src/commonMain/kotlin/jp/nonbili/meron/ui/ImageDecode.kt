package jp.nonbili.meron.ui

import androidx.compose.ui.graphics.ImageBitmap
import kotlin.math.sqrt

/**
 * The most memory one decoded image may take. Android's hardware canvas
 * refuses to draw a bitmap over 100 MB and takes the app down with it
 * ("Canvas: trying to draw too large bitmap"); iOS has no such check, but a
 * picture that size gets the app killed for memory instead. 64 MB leaves a
 * margin and still holds a full camera frame (4032x3024 is 49 MB).
 */
internal const val MAX_DECODED_IMAGE_BYTES = 64L * 1024L * 1024L

/** A picture shown at avatar size: a few hundred pixels a side is plenty. */
internal const val AVATAR_IMAGE_BYTES = 512L * 512L * 4L

/**
 * How much to scale a [width] by [height] image so that, at [bytesPerPixel],
 * it fits in [maxBytes]: 1.0 when it already does, otherwise the factor to
 * apply to both sides. The limit is on memory alone, so a long screenshot or a
 * wide panorama keeps its resolution as long as it fits. Unknown dimensions (a
 * file that is not an image) get 1.0 and fail in the decoder as before.
 */
internal fun imageDecodeScale(
    width: Int,
    height: Int,
    maxBytes: Long = MAX_DECODED_IMAGE_BYTES,
    bytesPerPixel: Int = 4,
): Double {
    if (width <= 0 || height <= 0) return 1.0
    val bytes = width.toLong() * height.toLong() * bytesPerPixel
    return if (bytes <= maxBytes) 1.0 else sqrt(maxBytes.toDouble() / bytes.toDouble())
}

/**
 * The size [imageDecodeScale] gives a [width] by [height] image, rounded down
 * so the result never exceeds the budget, and never below one pixel.
 */
internal fun scaledImageSize(
    width: Int,
    height: Int,
    scale: Double,
): Pair<Int, Int> = maxOf(1, (width * scale).toInt()) to maxOf(1, (height * scale).toInt())

/**
 * Decode encoded image bytes (PNG/JPEG/...) into a Compose [ImageBitmap] that
 * takes at most [maxBytes] of memory, scaling the picture down if it has to.
 * Callers that know the picture is shown small pass a smaller budget, so it
 * does not crowd everything else out of the bitmap cache.
 */
expect fun decodeImageBitmap(
    bytes: ByteArray,
    maxBytes: Long = MAX_DECODED_IMAGE_BYTES,
): ImageBitmap?
