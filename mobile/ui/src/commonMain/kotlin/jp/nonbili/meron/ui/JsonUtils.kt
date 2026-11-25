package jp.nonbili.meron.ui

/**
 * The first `"key": "value"` string in this JSON text, decoded. Scanned by hand:
 * Kotlin/Native's regex engine recurses once per character of a repeated group,
 * so matching a long value (a core response carrying a message body) with a
 * regex overflows the main thread's stack.
 */
internal fun String.jsonStringValue(key: String): String {
    val needle = "\"$key\""
    var from = 0
    while (true) {
        val at = indexOf(needle, from)
        if (at < 0) return ""
        from = at + 1
        var index = skipJsonWhitespace(at + needle.length)
        if (index >= length || this[index] != ':') continue
        index = skipJsonWhitespace(index + 1)
        if (index >= length || this[index] != '"') continue
        val start = index + 1
        index = start
        while (index < length && this[index] != '"') {
            index += if (this[index] == '\\') 2 else 1
        }
        if (index >= length) continue
        return substring(start, index).decodeJsonString()
    }
}

private fun String.skipJsonWhitespace(from: Int): Int {
    var index = from
    while (index < length && this[index].isWhitespace()) index++
    return index
}

internal fun String.jsonIntValue(
    key: String,
    defaultValue: Int,
): Int {
    val pattern = Regex(""""${Regex.escape(key)}"\s*:\s*(-?\d+)""")
    return pattern
        .find(this)
        ?.groupValues
        ?.get(1)
        ?.toIntOrNull() ?: defaultValue
}

internal fun String.decodeJsonString(): String {
    val out = StringBuilder()
    var index = 0
    while (index < length) {
        val ch = this[index]
        if (ch == '\\' && index + 1 < length) {
            when (val escaped = this[index + 1]) {
                '"', '\\', '/' -> out.append(escaped)
                'b' -> out.append('\b')
                'f' -> out.append('\u000c')
                'n' -> out.append('\n')
                'r' -> out.append('\r')
                't' -> out.append('\t')
                else -> out.append(escaped)
            }
            index += 2
        } else {
            out.append(ch)
            index += 1
        }
    }
    return out.toString()
}
