package jp.nonbili.meron

import com.google.android.gms.tasks.Tasks
import com.google.mlkit.nl.languageid.LanguageIdentification
import com.google.mlkit.nl.translate.TranslateLanguage
import com.google.mlkit.nl.translate.Translation
import com.google.mlkit.nl.translate.TranslatorOptions
import jp.nonbili.meron.ui.MailTranslation
import jp.nonbili.meron.ui.MailTranslationLanguage
import jp.nonbili.meron.ui.MailTranslator
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runInterruptible
import java.util.Locale

internal fun createAndroidMailTranslator(): MailTranslator = AndroidMailTranslator

private object AndroidMailTranslator : MailTranslator {
    override val attribution = "Powered by Google Translate"
    private val paragraphBreak = Regex("""\r?\n(?:[\t ]*\r?\n)+""")

    override suspend fun languages(): List<MailTranslationLanguage> =
        TranslateLanguage.getAllLanguages().map { tag ->
            val displayTag = if (tag == TranslateLanguage.CHINESE) "zh-Hans" else tag
            val locale = Locale.forLanguageTag(displayTag)
            MailTranslationLanguage(displayTag, locale.getDisplayName(locale))
        }

    override suspend fun translate(
        text: String,
        targetLanguage: String,
    ): MailTranslation =
        runInterruptible(Dispatchers.IO) {
            val targetTag = if (targetLanguage == "zh-Hans") TranslateLanguage.CHINESE else targetLanguage
            val target = requireNotNull(TranslateLanguage.fromLanguageTag(targetTag))
            val identifier = LanguageIdentification.getClient()
            val source =
                try {
                    val detectedTag = Tasks.await(identifier.identifyLanguage(text))
                    // Language Identification returns Filipino under a different tag than Translation uses.
                    val sourceTag = if (detectedTag == "fil") TranslateLanguage.TAGALOG else detectedTag
                    requireNotNull(TranslateLanguage.fromLanguageTag(sourceTag))
                } finally {
                    identifier.close()
                }
            if (source == target) return@runInterruptible MailTranslation(text, source)
            val translator =
                Translation.getClient(
                    TranslatorOptions
                        .Builder()
                        .setSourceLanguage(source)
                        .setTargetLanguage(target)
                        .build(),
                )
            try {
                Tasks.await(translator.downloadModelIfNeeded())
                val translated =
                    text.split(paragraphBreak).joinToString("\n\n") { paragraph ->
                        if (paragraph.isBlank()) paragraph else Tasks.await(translator.translate(paragraph))
                    }
                MailTranslation(translated, source)
            } finally {
                translator.close()
            }
        }
}
