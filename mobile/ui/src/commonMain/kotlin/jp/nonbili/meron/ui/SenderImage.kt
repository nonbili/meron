package jp.nonbili.meron.ui

import androidx.compose.runtime.staticCompositionLocalOf
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.parseSenderImageKind
import jp.nonbili.meron.shared.parseSenderImageResponse
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import kotlin.io.encoding.Base64

internal val LocalAvatarCore = staticCompositionLocalOf<MeronCore?> { null }

internal const val SENDER_IMAGE_KIND_FAVICON = "favicon"

/** Core resolves a sender image with blocking network calls. Scrolling an
 *  uncached inbox starts one per row, so cap how many run at once: the rest
 *  wait here, are dropped if their row leaves the screen first, and never
 *  take the shared IO threads that mail reads need. */
@OptIn(ExperimentalCoroutinesApi::class)
private val senderImageDispatcher = ioDispatcher.limitedParallelism(3)

/** The address to look up a sender image for: the caller's bare address as
 *  is, since re-parsing it would cut a valid local part such as
 *  `billing=receipts@example.com` down to another mailbox, or else the first
 *  address found in the label. */
internal fun senderImageAddress(
    email: String,
    label: String,
): String? = email.trim().takeIf { '@' in it } ?: extractEmail(label)

/** The smallest size asked of core; anything smaller shares this one. */
private const val MIN_SENDER_IMAGE_REQUEST_PX = 64

/** The pixel size to ask core for: what the avatar covers on screen, rounded
 *  up to a power of two. The exact size would keep a Gravatar photo sharp too,
 *  but core caches and fetches per size, so 38dp and 40dp avatars of the same
 *  sender (114 and 120px at 3x) share one 128px picture instead of fetching
 *  twice. Core clamps the result to the range it serves. */
internal fun senderImageRequestPx(drawnPx: Int): Int =
    if (drawnPx <= MIN_SENDER_IMAGE_REQUEST_PX) {
        MIN_SENDER_IMAGE_REQUEST_PX
    } else {
        (drawnPx - 1).takeHighestOneBit() shl 1
    }

/** Core owns provider selection, placeholder rejection, parent fallback and caching. */
internal suspend fun loadSenderImage(
    core: MeronCore,
    email: String,
    sizePx: Int,
): KindedImageBitmap? {
    val requestPx = senderImageRequestPx(sizePx)
    // Core trims and lowercases the address; do the same so spellings of one
    // sender share a cache entry and an in-flight lookup.
    val address = email.trim().lowercase()
    // Keyed by the requested size like core's own cache, so a small avatar's
    // picture is not reused for a larger one.
    val key = "sender:$address:$requestPx"
    return loadCachedKindedImageBitmap(key) {
        // Rows of the same sender share one lookup instead of each taking a
        // worker slot only to block in core behind the first.
        senderImageLoads.load(key) { resolveSenderImage(core, address, requestPx) }
    }
}

private val senderImageLoads = InFlightLoads<KindedImageBitmap?>()

private suspend fun resolveSenderImage(
    core: MeronCore,
    email: String,
    requestPx: Int,
): KindedImageBitmap? =
    withContext(senderImageDispatcher) {
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

/** Runs one load per key at a time; callers asking for a key already loading
 *  wait for that result. If the caller doing the load is cancelled (its row
 *  scrolled away), a waiter takes over and loads it itself. */
internal class InFlightLoads<T> {
    private class OwnerGone : Exception()

    private val mutex = Mutex()
    private val loads = HashMap<String, CompletableDeferred<T>>()

    suspend fun load(
        key: String,
        block: suspend () -> T,
    ): T {
        while (true) {
            var owner = false
            val deferred =
                mutex.withLock {
                    loads.getOrPut(key) { CompletableDeferred<T>().also { owner = true } }
                }
            if (!owner) {
                try {
                    return deferred.await()
                } catch (_: OwnerGone) {
                    currentCoroutineContext().ensureActive()
                    continue
                }
            }
            val result =
                try {
                    block()
                } catch (failure: Throwable) {
                    finish(key, deferred)
                    deferred.completeExceptionally(OwnerGone())
                    throw failure
                }
            finish(key, deferred)
            deferred.complete(result)
            return result
        }
    }

    private suspend fun finish(
        key: String,
        deferred: CompletableDeferred<T>,
    ) {
        withContext(NonCancellable) {
            mutex.withLock { if (loads[key] === deferred) loads.remove(key) }
        }
    }
}
