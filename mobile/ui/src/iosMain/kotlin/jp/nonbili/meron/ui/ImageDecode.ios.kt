package jp.nonbili.meron.ui

import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asComposeImageBitmap
import androidx.compose.ui.graphics.toComposeImageBitmap
import org.jetbrains.skia.Bitmap
import org.jetbrains.skia.FilterMipmap
import org.jetbrains.skia.FilterMode
import org.jetbrains.skia.Image
import org.jetbrains.skia.MipmapMode

actual fun decodeImageBitmap(
    bytes: ByteArray,
    maxBytes: Long,
): ImageBitmap? =
    runCatching {
        val image = Image.makeFromEncoded(bytes)
        val scale = imageDecodeScale(image.width, image.height, maxBytes)
        if (scale >= 1.0) return@runCatching image.toComposeImageBitmap()
        // Scale into a bitmap of the budgeted size rather than keeping the
        // full-size pixels around.
        try {
            val (width, height) = scaledImageSize(image.width, image.height, scale)
            val bitmap = Bitmap()
            var scaled = false
            try {
                if (bitmap.allocN32Pixels(width, height)) {
                    val pixels = bitmap.peekPixels()
                    if (pixels != null) {
                        try {
                            scaled = image.scalePixels(pixels, FilterMipmap(FilterMode.LINEAR, MipmapMode.LINEAR), false)
                        } finally {
                            pixels.close()
                        }
                    }
                }
                if (scaled) {
                    bitmap.setImmutable()
                    bitmap.asComposeImageBitmap()
                } else {
                    null
                }
            } finally {
                if (!scaled) bitmap.close()
            }
        } finally {
            image.close()
        }
    }.getOrNull()
