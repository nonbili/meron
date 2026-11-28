package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.CloseableHandle
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

interface IosMailTranslationBridge {
    fun languages(completion: (List<MailTranslationLanguage>) -> Unit)

    fun translate(
        text: String,
        targetLanguage: String,
        completion: (MailTranslation?) -> Unit,
    ): CloseableHandle
}

internal class IosMailTranslator(
    private val bridge: IosMailTranslationBridge,
) : MailTranslator {
    private val session = Mutex()

    override suspend fun languages(): List<MailTranslationLanguage> =
        suspendCancellableCoroutine { continuation ->
            bridge.languages { continuation.resume(it) }
        }

    override suspend fun translate(
        text: String,
        targetLanguage: String,
    ): MailTranslation =
        session.withLock {
            suspendCancellableCoroutine { continuation ->
                val handle =
                    bridge.translate(text, targetLanguage) { result ->
                        if (result == null) {
                            continuation.resumeWithException(IllegalStateException("Apple translation failed"))
                        } else {
                            continuation.resume(result)
                        }
                    }
                continuation.invokeOnCancellation { handle.close() }
            }
        }
}
