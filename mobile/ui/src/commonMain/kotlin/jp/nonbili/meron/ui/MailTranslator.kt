package jp.nonbili.meron.ui

data class MailTranslationLanguage(
    val tag: String,
    val name: String,
)

data class MailTranslation(
    val text: String,
    val sourceLanguage: String,
)

interface MailTranslator {
    val attribution: String? get() = null

    suspend fun languages(): List<MailTranslationLanguage>

    suspend fun translate(
        text: String,
        targetLanguage: String,
    ): MailTranslation
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
