package jp.nonbili.meron

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import com.google.android.gms.tasks.Task
import com.google.mlkit.common.model.DownloadConditions
import com.google.mlkit.common.model.RemoteModelManager
import com.google.mlkit.nl.languageid.LanguageIdentification
import com.google.mlkit.nl.translate.TranslateLanguage
import com.google.mlkit.nl.translate.TranslateRemoteModel
import com.google.mlkit.nl.translate.Translation
import com.google.mlkit.nl.translate.TranslatorOptions
import jp.nonbili.meron.ui.MailTranslation
import jp.nonbili.meron.ui.MailTranslationException
import jp.nonbili.meron.ui.MailTranslationLanguage
import jp.nonbili.meron.ui.MailTranslationProblem
import jp.nonbili.meron.ui.MailTranslator
import jp.nonbili.meron.ui.translateMailText
import kotlinx.coroutines.TimeoutCancellationException
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeout
import java.text.Collator
import java.util.Locale
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

internal fun createAndroidMailTranslator(context: Context): MailTranslator = AndroidMailTranslator(context.applicationContext)

private class AndroidMailTranslator(
    private val context: Context,
) : MailTranslator {
    override suspend fun languages(displayLanguage: String): List<MailTranslationLanguage> {
        val display = Locale.forLanguageTag(displayLanguage.replace('_', '-'))
        val collator = Collator.getInstance(display)
        return TranslateLanguage
            .getAllLanguages()
            .map { tag ->
                val displayTag = if (tag == TranslateLanguage.CHINESE) "zh-Hans" else tag
                val locale = Locale.forLanguageTag(displayTag)
                val name = locale.getDisplayName(display).replaceFirstChar { it.titlecase(display) }
                val native = locale.getDisplayName(locale).replaceFirstChar { it.titlecase(locale) }
                // Named in the app's language, then in its own, so either finds it.
                MailTranslationLanguage(displayTag, if (native.equals(name, ignoreCase = true)) name else "$name · $native")
            }.sortedWith(compareBy(collator) { it.name })
    }

    override suspend fun translate(
        text: String,
        targetLanguage: String,
        allowMeteredDownload: Boolean,
        onDownloading: () -> Unit,
    ): MailTranslation {
        val targetTag = if (targetLanguage == "zh-Hans") TranslateLanguage.CHINESE else targetLanguage
        val target = requireNotNull(TranslateLanguage.fromLanguageTag(targetTag))
        val source = identifyLanguage(text)
        if (source == target) return MailTranslation(text, source)
        ensureModels(source, target, allowMeteredDownload, onDownloading)
        val translator =
            Translation.getClient(
                TranslatorOptions
                    .Builder()
                    .setSourceLanguage(source)
                    .setTargetLanguage(target)
                    .build(),
            )
        try {
            // Loads the downloaded model; nothing is fetched here.
            translator.downloadModelIfNeeded().await()
            return MailTranslation(translateMailText(text) { translator.translate(it).await() }, source)
        } finally {
            translator.close()
        }
    }

    private suspend fun identifyLanguage(text: String): String {
        val identifier = LanguageIdentification.getClient()
        val detectedTag =
            try {
                identifier.identifyLanguage(text).await()
            } finally {
                identifier.close()
            }
        // Language Identification returns Filipino under a different tag than Translation uses.
        val sourceTag = if (detectedTag == "fil") TranslateLanguage.TAGALOG else detectedTag
        return TranslateLanguage.fromLanguageTag(sourceTag)
            ?: throw MailTranslationException(MailTranslationProblem.UnknownLanguage)
    }

    private suspend fun ensureModels(
        source: String,
        target: String,
        allowMeteredDownload: Boolean,
        onDownloading: () -> Unit,
    ) {
        val models = RemoteModelManager.getInstance()
        val missing =
            listOf(source, target)
                .map { TranslateRemoteModel.Builder(it).build() }
                .filterNot { models.isModelDownloaded(it).await() }
        if (missing.isEmpty()) return
        val connectivity = context.getSystemService(ConnectivityManager::class.java)
        val online =
            connectivity
                ?.getNetworkCapabilities(connectivity.activeNetwork)
                ?.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET) == true
        // ML Kit would otherwise wait for a network indefinitely.
        if (connectivity == null || !online) throw MailTranslationException(MailTranslationProblem.Offline)
        if (connectivity.isActiveNetworkMetered && !allowMeteredDownload) {
            throw MailTranslationException(MailTranslationProblem.MeteredDownload)
        }
        onDownloading()
        try {
            withTimeout(MODEL_DOWNLOAD_TIMEOUT_MS) {
                missing.forEach { models.download(it, DownloadConditions.Builder().build()).await() }
            }
        } catch (_: TimeoutCancellationException) {
            // A network that drops mid-download stalls it rather than failing it.
            throw MailTranslationException(MailTranslationProblem.Offline)
        }
    }
}

// A language model is about 30 MB.
private const val MODEL_DOWNLOAD_TIMEOUT_MS = 5 * 60 * 1000L

private suspend fun <T> Task<T>.await(): T =
    suspendCancellableCoroutine { continuation ->
        addOnSuccessListener { continuation.resume(it) }
        addOnFailureListener { continuation.resumeWithException(it) }
        addOnCanceledListener { continuation.cancel() }
    }
