package jp.nonbili.meron.ui

import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.ImageBitmap
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.parseSenderImageResponse
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.withContext
import kotlin.io.encoding.Base64

internal val LocalAvatarCore = staticCompositionLocalOf<MeronCore?> { null }

/** Core owns provider selection, placeholder rejection, parent fallback and caching. */
internal suspend fun loadSenderImage(
    core: MeronCore,
    email: String,
): ImageBitmap? =
    loadCachedImageBitmap("sender:$email") {
        withContext(ioDispatcher) {
            try {
                val src = parseSenderImageResponse(MobileMailCommandClient(core).resolveSenderImage(email))
                if (!src.startsWith("data:image/") || !src.contains(";base64,")) {
                    null
                } else {
                    decodeImageBitmap(Base64.Default.decode(src.substringAfter(";base64,")))
                }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: Exception) {
                null
            }
        }
    }
