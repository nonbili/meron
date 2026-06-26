package jp.nonbili.meron.ui

import androidx.compose.ui.graphics.ImageBitmap

/** Decode encoded image bytes (PNG/JPEG/...) into a Compose [ImageBitmap]. */
expect fun decodeImageBitmap(bytes: ByteArray): ImageBitmap?
