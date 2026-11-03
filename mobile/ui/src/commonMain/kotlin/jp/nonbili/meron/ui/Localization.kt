package jp.nonbili.meron.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf

/** Current app language tag (e.g. "en", "ja", "pt-BR"). Provided by the root. */
val LocalAppLocale = staticCompositionLocalOf { "en" }

/** Resolve and ICU-format a catalog string for [locale]; falls back to English
 *  then the key itself. */
internal fun localizedString(
    locale: String,
    key: String,
    args: Map<String, Any?> = emptyMap(),
): String {
    val messages =
        generatedStrings[locale]
            ?: generatedStrings[locale.substringBefore('-')]
            ?: generatedStrings[locale.substringBefore('_')]
            ?: generatedStrings["en"]
            ?: emptyMap()
    val template = messages[key] ?: generatedStrings["en"]?.get(key) ?: key
    return formatIcu(template, args, locale)
}

/** Composable string lookup keyed by the current [LocalAppLocale]. */
@Composable
@ReadOnlyComposable
internal fun tr(
    key: String,
    args: Map<String, Any?> = emptyMap(),
): String = localizedString(LocalAppLocale.current, key, args)

/**
 * Catalog lookup outside a composition. `tr` needs `LocalAppLocale`, which the
 * state functions have no access to, so resolve the tag from prefs the same way
 * the root composable does.
 */
internal fun MeronMobileState.trs(
    key: String,
    args: Map<String, Any?> = emptyMap(),
): String = localizedString(loadAppLanguageTag(prefs).ifBlank { "en" }, key, args)

/**
 * [trs] for a failure, followed by its reason when [error] has one: "Move
 * failed: connection reset". The joining punctuation is itself a catalog
 * string, since languages differ on it.
 */
internal fun MeronMobileState.trsError(
    key: String,
    error: Throwable?,
    args: Map<String, Any?> = emptyMap(),
): String = withReason(trs(key, args), error?.message) { template, fields -> trs(template, fields) }

/** [message], then [reason] if there is one, joined by `common.errorWithReason`. */
internal fun withReason(
    message: String,
    reason: String?,
    lookup: (String, Map<String, Any?>) -> String,
): String =
    if (reason.isNullOrBlank()) {
        message
    } else {
        lookup("common.errorWithReason", mapOf("message" to message, "error" to reason))
    }

/** Composable lookup for catalog strings that use positional `%1$s` / `%1$d`
 *  placeholders (the simple/non-ICU arg'd messages). */
@Composable
@ReadOnlyComposable
internal fun trf(
    key: String,
    vararg formatArgs: Any?,
): String = formatPositional(localizedString(LocalAppLocale.current, key), formatArgs)

private fun formatPositional(
    template: String,
    args: Array<out Any?>,
): String {
    var out = template
    args.forEachIndexed { index, value ->
        val text = value?.toString().orEmpty()
        out = out.replace("%${index + 1}\$s", text).replace("%${index + 1}\$d", text)
    }
    return out
}

private fun formatIcu(
    template: String,
    args: Map<String, Any?>,
    locale: String,
): String {
    val plural = parsePlural(template)
    val selected =
        if (plural == null) {
            template
        } else {
            val count =
                (args[plural.argument] as? Number)?.toLong()
                    ?: args[plural.argument]?.toString()?.toLongOrNull()
                    ?: 0L
            val category =
                if (locale.substringBefore('-').substringBefore('_').lowercase() == "ru") {
                    russianPluralCategory(count)
                } else {
                    if (count == 1L) "one" else "other"
                }
            plural.variants[category]
                ?: plural.variants["other"]
                ?: template
        }
    return replacePlaceholders(selected, args)
}

private fun russianPluralCategory(count: Long): String {
    val lastTwo = kotlin.math.abs(count % 100)
    val last = lastTwo % 10
    return when {
        last == 1L && lastTwo != 11L -> "one"
        last in 2L..4L && lastTwo !in 12L..14L -> "few"
        else -> "many"
    }
}

private data class IcuPlural(
    val argument: String,
    val variants: Map<String, String>,
)

private fun parsePlural(template: String): IcuPlural? {
    val prefix = Regex("""^\{([A-Za-z][A-Za-z0-9_]*)\s*,\s*plural\s*,""").find(template) ?: return null
    var index = prefix.range.last + 1
    val variants = linkedMapOf<String, String>()
    while (index < template.lastIndex) {
        while (index < template.length && template[index].isWhitespace()) index += 1
        val categoryStart = index
        while (index < template.length && template[index].isLetter()) index += 1
        if (categoryStart == index) break
        val category = template.substring(categoryStart, index)
        while (index < template.length && template[index].isWhitespace()) index += 1
        if (index >= template.length || template[index] != '{') break
        val branch = readBraceBranch(template, index)
        variants[category] = branch.first
        index = branch.second
    }
    return IcuPlural(prefix.groupValues[1], variants)
}

private fun readBraceBranch(
    template: String,
    start: Int,
): Pair<String, Int> {
    var depth = 0
    var index = start
    while (index < template.length) {
        when (template[index]) {
            '{' -> {
                depth += 1
            }

            '}' -> {
                depth -= 1
                if (depth == 0) {
                    return template.substring(start + 1, index) to index + 1
                }
            }
        }
        index += 1
    }
    return template.substring(start + 1) to template.length
}

private fun replacePlaceholders(
    value: String,
    args: Map<String, Any?>,
): String =
    Regex("""\{([A-Za-z][A-Za-z0-9_]*)(?:\s*,\s*number)?\}""").replace(value) { match ->
        args[match.groupValues[1]]?.toString().orEmpty()
    }
