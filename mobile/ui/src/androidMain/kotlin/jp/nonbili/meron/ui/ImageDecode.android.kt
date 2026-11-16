package jp.nonbili.meron.ui

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.Build
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap

/** The decoder's density arithmetic is in integers; this is its denominator. */
private const val SCALE_DENSITY = 10_000

/**
 * The decoder options that fit a [width] by [height] image into [maxBytes] at
 * [bytesPerPixel] (see [imageDecodeScale]). `inSampleSize` only halves, so it
 * takes the largest power of two that still leaves the image at or above the
 * target, and density scaling does the rest: the result lands on the budget
 * instead of up to four times under it.
 */
internal fun imageDecodeOptions(
    width: Int,
    height: Int,
    maxBytes: Long,
    bytesPerPixel: Int = 4,
): BitmapFactory.Options =
    BitmapFactory.Options().apply {
        val scale = imageDecodeScale(width, height, maxBytes, bytesPerPixel)
        if (scale >= 1.0) return@apply
        var sample = 1
        while (sample * 2 * scale <= 1.0) sample *= 2
        inSampleSize = sample
        inScaled = true
        inDensity = SCALE_DENSITY
        // A sampled JPEG rounds its size up, and the density step rounds to the
        // nearest pixel, so step the target down until the worst case fits.
        val sampledWidth = (width + sample - 1) / sample
        val sampledHeight = (height + sample - 1) / sample
        var target = maxOf(1, (SCALE_DENSITY * scale * sample).toInt())
        while (target > 1 &&
            densityScaled(sampledWidth, target).toLong() * densityScaled(sampledHeight, target) * bytesPerPixel > maxBytes
        ) {
            target--
        }
        inTargetDensity = target
    }

/** The size BitmapFactory gives [size] pixels scaled from [SCALE_DENSITY] to [target]. */
internal fun densityScaled(
    size: Int,
    target: Int,
): Int = (size * (target.toFloat() / SCALE_DENSITY) + 0.5f).toInt()

actual fun decodeImageBitmap(
    bytes: ByteArray,
    maxBytes: Long,
): ImageBitmap? =
    runCatching {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
        // A 16-bit or wide-gamut source decodes to half-float pixels, twice
        // the size; the bounds pass says which it will be.
        val wide = Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && bounds.outConfig == Bitmap.Config.RGBA_F16
        val options = imageDecodeOptions(bounds.outWidth, bounds.outHeight, maxBytes, if (wide) 8 else 4)
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options)?.let { bitmap ->
            // Density scaling leaves the made-up target density on the bitmap;
            // clear it so nothing that honours density draws it at the wrong size.
            bitmap.density = Bitmap.DENSITY_NONE
            bitmap.asImageBitmap()
        }
    }.getOrNull()
