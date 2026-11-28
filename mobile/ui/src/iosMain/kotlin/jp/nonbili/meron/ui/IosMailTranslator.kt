package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.CloseableHandle
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import platform.Foundation.NSLocale
import platform.Foundation.localizedStringForLocaleIdentifier
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

    override suspend fun languages(displayLanguage: String): List<MailTranslationLanguage> {
        val languages =
            suspendCancellableCoroutine { continuation ->
                bridge.languages { continuation.resume(it) }
            }
        val display = NSLocale(localeIdentifier = displayLanguage.replace('-', '_'))
        return languages
            .map { language ->
                // The bridge names each language in its own; add the app's name for it.
                val name =
                    display
                        .localizedStringForLocaleIdentifier(language.tag)
                        .replaceFirstChar { it.uppercaseChar() }
                        .takeIf { it.isNotBlank() && !it.equals(language.name, ignoreCase = true) }
                language.copy(name = name?.let { "$it · ${language.name}" } ?: language.name)
            }.sortedBy { it.name.lowercase() }
    }

    // Apple's translation task shows its own download prompt and progress.
    override suspend fun translate(
        text: String,
        targetLanguage: String,
        allowMeteredDownload: Boolean,
        onDownloading: () -> Unit,
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
