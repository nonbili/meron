package jp.nonbili.meron.ui

import androidx.compose.runtime.staticCompositionLocalOf
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.parseSenderImageKind
import jp.nonbili.meron.shared.parseSenderImageResponse
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.withContext
import kotlin.io.encoding.Base64

internal val LocalAvatarCore = staticCompositionLocalOf<MeronCore?> { null }

/** Core clamps the requested size to this range. */
private const val MIN_SENDER_IMAGE_PX = 16
private const val MAX_SENDER_IMAGE_PX = 512

internal const val SENDER_IMAGE_KIND_FAVICON = "favicon"

/** The pixel size to ask core for: what the avatar covers on screen, so a
 *  Gravatar photo is not fetched at 96px and stretched over a 114px circle on
 *  a 3x display. */
internal fun senderImageRequestPx(drawnPx: Int): Int = drawnPx.coerceIn(MIN_SENDER_IMAGE_PX, MAX_SENDER_IMAGE_PX)

/** Core owns provider selection, placeholder rejection, parent fallback and caching. */
internal suspend fun loadSenderImage(
    core: MeronCore,
    email: String,
    sizePx: Int,
): KindedImageBitmap? {
    val requestPx = senderImageRequestPx(sizePx)
    // Keyed by size like core's own cache, so a small avatar's picture is not
    // reused for a larger one.
    return loadCachedKindedImageBitmap("sender:$email:$requestPx") {
        withContext(ioDispatcher) {
            try {
                val response = MobileMailCommandClient(core).resolveSenderImage(email, requestPx)
                val src = parseSenderImageResponse(response)
                if (!src.startsWith("data:image/") || !src.contains(";base64,")) {
                    null
                } else {
                    decodeImageBitmap(Base64.Default.decode(src.substringAfter(";base64,")), AVATAR_IMAGE_BYTES)
                        ?.let { KindedImageBitmap(it, parseSenderImageKind(response)) }
                }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: Exception) {
                null
            }
        }
    }
}
