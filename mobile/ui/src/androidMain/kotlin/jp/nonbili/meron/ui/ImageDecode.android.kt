package jp.nonbili.meron.ui

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.Build
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap

/** The decoder's density arithmetic is in integers; this is its denominator. */
private const val SCALE_DENSITY = 10_000

/**
 * The decoder options that scale an image by [scale] (see [imageDecodeScale]).
 * `inSampleSize` only halves, so it takes the largest power of two that still
 * leaves the image at or above the target, and density scaling does the rest:
 * the result lands on the budget instead of up to four times under it.
 */
internal fun imageDecodeOptions(scale: Double): BitmapFactory.Options =
    BitmapFactory.Options().apply {
        if (scale >= 1.0) return@apply
        var sample = 1
        while (sample * 2 * scale <= 1.0) sample *= 2
        inSampleSize = sample
        inScaled = true
        inDensity = SCALE_DENSITY
        // Rounded down, so the scaled image never ends up over the budget.
        inTargetDensity = maxOf(1, (SCALE_DENSITY * scale * sample).toInt())
    }

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
        val scale = imageDecodeScale(bounds.outWidth, bounds.outHeight, maxBytes, if (wide) 8 else 4)
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, imageDecodeOptions(scale))?.asImageBitmap()
    }.getOrNull()
