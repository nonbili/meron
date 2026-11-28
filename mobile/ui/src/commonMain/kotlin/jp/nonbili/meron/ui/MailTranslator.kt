package jp.nonbili.meron.ui

data class MailTranslationLanguage(
    val tag: String,
    val name: String,
)

data class MailTranslation(
    val text: String,
    val sourceLanguage: String,
)

/** Why a translation could not be made, when the reader can do something about it. */
enum class MailTranslationProblem {
    /** The language model is not on the device and there is no network to fetch it. */
    Offline,

    /** The language model would download over a metered network; ask first. */
    MeteredDownload,

    /** The message's language could not be identified, or is not supported. */
    UnknownLanguage,
}

class MailTranslationException(
    val problem: MailTranslationProblem,
) : Exception(problem.name)

interface MailTranslator {
    /** The target languages, named in [displayLanguage] where the platform can, in display order. */
    suspend fun languages(displayLanguage: String): List<MailTranslationLanguage>

    /**
     * Translates [text] on the device. [onDownloading] is called when a language
     * model has to be fetched first. Throws [MailTranslationException] for the
     * problems the reader can act on.
     */
    suspend fun translate(
        text: String,
        targetLanguage: String,
        allowMeteredDownload: Boolean,
        onDownloading: () -> Unit,
    ): MailTranslation
}

/**
 * Translates [text] a line at a time through [translateLine], so the line breaks
 * that lay out plain mail (signatures, "Label : value" tables) survive: a
 * translator given a whole paragraph runs its lines together. Hard-wrapped
 * prose is rejoined first, so its sentences are still translated whole.
 * Indentation is kept, lines without letters are left alone, and a repeated
 * line is translated once.
 */
suspend fun translateMailText(
    text: String,
    translateLine: suspend (String) -> String,
): String {
    val translated = mutableMapOf<String, String>()
    val out = StringBuilder()
    mailTranslationSegments(text).forEachIndexed { index, segment ->
        if (index > 0) out.append('\n')
        val indent = segment.takeWhile(Char::isWhitespace)
        val content = segment.substring(indent.length)
        if (content.none(Char::isLetter)) {
            out.append(segment)
        } else {
            out.append(indent).append(translated.getOrPut(content) { translateLine(content) })
        }
    }
    return out.toString()
}

/** [text]'s lines, with a hard-wrapped line joined to the one it runs into. */
internal fun mailTranslationSegments(text: String): List<String> {
    val segments = mutableListOf<String>()
    var wrapping = false
    text.replace("\r\n", "\n").split('\n').forEach { raw ->
        val line = raw.trimEnd()
        if (wrapping && line.isNotEmpty() && !line.first().isWhitespace()) {
            segments[segments.lastIndex] = segments.last() + " " + line
        } else {
            segments += line
        }
        wrapping = line.length >= WRAPPED_LINE_LENGTH && line.endsMidSentence()
    }
    return segments
}

// Mail clients wrap plain text at 72-78 columns; a shorter line ended where its
// writer meant it to.
private const val WRAPPED_LINE_LENGTH = 60

// Ends in a word or a comma rather than a stop or a colon. CJK text is left
// alone: it is not wrapped at a space, so joining would add one.
private fun String.endsMidSentence(): Boolean {
    val last = last()
    return last.code < 0x2E80 && (last.isLetterOrDigit() || last == ',' || last == ';')
}

internal fun defaultMailTranslationTarget(
    languages: List<MailTranslationLanguage>,
    appLocale: String,
): MailTranslationLanguage? {
    val tag = appLocale.replace('_', '-')
    val base = tag.substringBefore('-')
    val script = translationScript(if (base.equals("zh", ignoreCase = true)) resolveDeviceLanguageTag(tag) else tag)
    return languages.firstOrNull { it.tag.equals(tag, ignoreCase = true) }
        ?: languages.firstOrNull {
            val candidateScript = translationScript(it.tag)
            it.tag.substringBefore('-').equals(base, ignoreCase = true) &&
                (script == null || candidateScript == null || script.equals(candidateScript, ignoreCase = true))
        }
        ?: translationLanguageAlias(base)?.let { alias -> languages.firstOrNull { it.tag.substringBefore('-').equals(alias, ignoreCase = true) } }
        ?: languages.firstOrNull { it.tag == "en" }
        ?: languages.firstOrNull()
}

private fun translationScript(tag: String): String? = tag.substringAfter('-', "").substringBefore('-').takeIf { it.length == 4 && it.all(Char::isLetter) }

private fun translationLanguageAlias(base: String): String? =
    when (base.lowercase()) {
        "nb" -> "no"
        "no" -> "nb"
        "fil" -> "tl"
        "tl" -> "fil"
        else -> null
    }
