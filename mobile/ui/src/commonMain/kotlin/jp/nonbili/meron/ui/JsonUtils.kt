package jp.nonbili.meron.ui

internal fun String.jsonStringValue(key: String): String {
    val pattern = Regex(""""${Regex.escape(key)}"\s*:\s*"((?:\\.|[^"\\])*)"""")
    return pattern
        .find(this)
        ?.groupValues
        ?.get(1)
        ?.decodeJsonString()
        .orEmpty()
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
