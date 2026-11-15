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
        val (width, height) = scaledImageSize(image.width, image.height, scale)
        val bitmap = Bitmap()
        if (!bitmap.allocN32Pixels(width, height)) return@runCatching null
        val pixels = bitmap.peekPixels() ?: return@runCatching null
        if (!image.scalePixels(pixels, FilterMipmap(FilterMode.LINEAR, MipmapMode.LINEAR), false)) return@runCatching null
        image.close()
        bitmap.asComposeImageBitmap()
    }.getOrNull()
