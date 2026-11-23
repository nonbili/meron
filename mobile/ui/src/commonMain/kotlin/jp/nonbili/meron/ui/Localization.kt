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
    return formatIcuMessage(template, args, locale)
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

/**
 * Minimal ICU MessageFormat: `{name}`, `{name, number}` and
 * `{name, plural, =0 {…} one {…} other {…}}` anywhere in the message, any
 * number of times, nested, with `#` inside a plural branch replaced by its count.
 * Anything else in braces is kept literally. Public so the Android app's
 * generated native catalog formats the same way.
 */
fun formatIcuMessage(
    template: String,
    args: Map<String, Any?>,
    locale: String,
): String = formatMessage(template, args, locale.replace('_', '-').lowercase(), null)

private fun formatMessage(
    template: String,
    args: Map<String, Any?>,
    locale: String,
    pound: String?,
): String {
    val out = StringBuilder()
    var index = 0
    while (index < template.length) {
        val char = template[index]
        when {
            char == '{' -> {
                val end = matchingBrace(template, index)
                if (end < 0) {
                    out.append(template, index, template.length)
                    return out.toString()
                }
                out.append(
                    formatArgument(template.substring(index + 1, end), args, locale)
                        ?: template.substring(index, end + 1),
                )
                index = end + 1
            }

            char == '#' && pound != null -> {
                out.append(pound)
                index += 1
            }

            else -> {
                out.append(char)
                index += 1
            }
        }
    }
    return out.toString()
}

private val argumentPattern = Regex("""^\s*([A-Za-z][A-Za-z0-9_]*)\s*(?:,\s*([A-Za-z]+)\s*(?:,([\s\S]*))?)?$""")

/** Format the inside of one `{…}`, or null when it is not an argument we understand. */
private fun formatArgument(
    body: String,
    args: Map<String, Any?>,
    locale: String,
): String? {
    val match = argumentPattern.find(body) ?: return null
    val name = match.groupValues[1]
    return when (match.groupValues[2]) {
        "", "number" -> {
            args[name]?.toString().orEmpty()
        }

        "plural" -> {
            val variants = parsePluralVariants(match.groupValues[3])
            val value = args[name]
            val count = (value as? Number)?.toLong() ?: value?.toString()?.toLongOrNull() ?: 0L
            val branch =
                variants["=$count"]
                    ?: variants[pluralCategory(locale, count)]
                    ?: variants["other"]
                    ?: return null
            formatMessage(branch, args, locale, value?.toString() ?: count.toString())
        }

        else -> {
            null
        }
    }
}

private fun parsePluralVariants(source: String): Map<String, String> {
    val variants = linkedMapOf<String, String>()
    var index = 0
    while (index < source.length) {
        while (index < source.length && source[index].isWhitespace()) index += 1
        val selectorStart = index
        while (index < source.length && (source[index].isLetterOrDigit() || source[index] == '=' || source[index] == '-')) index += 1
        if (selectorStart == index) break
        val selector = source.substring(selectorStart, index)
        while (index < source.length && source[index].isWhitespace()) index += 1
        if (index >= source.length || source[index] != '{') break
        val end = matchingBrace(source, index)
        if (end < 0) break
        variants[selector] = source.substring(index + 1, end)
        index = end + 1
    }
    return variants
}

/** Index of the `}` closing the `{` at [start], or -1. */
private fun matchingBrace(
    text: String,
    start: Int,
): Int {
    var depth = 0
    for (index in start until text.length) {
        when (text[index]) {
            '{' -> {
                depth += 1
            }

            '}' -> {
                depth -= 1
                if (depth == 0) return index
            }
        }
    }
    return -1
}

/** CLDR cardinal category for [locale] (a lowercase `lang` or `lang-region` tag). */
private fun pluralCategory(
    locale: String,
    count: Long,
): String {
    val n = if (count == Long.MIN_VALUE) Long.MAX_VALUE else kotlin.math.abs(count)
    val lastTwo = n % 100
    val last = n % 10
    return when (locale.substringBefore('-')) {
        "fr" -> {
            if (n <= 1L) "one" else "other"
        }

        "pt" -> {
            if (n == 1L || (n == 0L && locale == "pt-br")) "one" else "other"
        }

        "ru", "uk" -> {
            when {
                last == 1L && lastTwo != 11L -> "one"
                last in 2L..4L && lastTwo !in 12L..14L -> "few"
                else -> "many"
            }
        }

        "pl" -> {
            when {
                n == 1L -> "one"
                last in 2L..4L && lastTwo !in 12L..14L -> "few"
                else -> "many"
            }
        }

        "ar" -> {
            when {
                n == 0L -> "zero"
                n == 1L -> "one"
                n == 2L -> "two"
                lastTwo in 3L..10L -> "few"
                lastTwo in 11L..99L -> "many"
                else -> "other"
            }
        }

        "lv" -> {
            when {
                last == 0L || lastTwo in 11L..19L -> "zero"
                last == 1L && lastTwo != 11L -> "one"
                else -> "other"
            }
        }

        else -> {
            if (n == 1L) "one" else "other"
        }
    }
}
