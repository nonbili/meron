package jp.nonbili.meron.shared

const val EXPECTED_PROTOCOL_VERSION: Int = 1

data class CoreRequest(
    val id: Long,
    val method: String,
    val paramsJson: String = "{}",
)

fun CoreRequest.toJson(): String {
    return """{"id":$id,"method":${method.jsonString()},"params":$paramsJson}"""
}

fun pingRequest(id: Long = 1): CoreRequest = CoreRequest(id = id, method = "ping")

fun requireProtocolVersion(actual: Int) {
    require(actual == EXPECTED_PROTOCOL_VERSION) {
        "Unsupported meron-core protocol $actual, expected $EXPECTED_PROTOCOL_VERSION"
    }
}

internal fun String.jsonString(): String {
    val out = StringBuilder(length + 2)
    out.append('"')
    for (ch in this) {
        when (ch) {
            '\\' -> out.append("\\\\")
            '"' -> out.append("\\\"")
            '\n' -> out.append("\\n")
            '\r' -> out.append("\\r")
            '\t' -> out.append("\\t")
            else -> {
                if (ch.code < 0x20) {
                    out.append("\\u")
                    out.append(ch.code.toString(16).padStart(4, '0'))
                } else {
                    out.append(ch)
                }
            }
        }
    }
    out.append('"')
    return out.toString()
}
