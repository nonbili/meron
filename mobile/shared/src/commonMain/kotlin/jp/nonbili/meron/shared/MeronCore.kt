package jp.nonbili.meron.shared

interface MeronCore {
    suspend fun invoke(command: String, payloadJson: String = "{}"): String
    fun events(): CoreEventStream
    suspend fun protocolVersion(): Int
}

interface CoreEventStream {
    fun subscribe(listener: (CoreEvent) -> Unit): CloseableHandle
}

fun interface CloseableHandle {
    fun close()
}

data class CoreEvent(
    val name: String,
    val detailJson: String,
)

fun parseCoreEventEnvelope(eventJson: String): CoreEvent {
    val objectBody = eventJson.trim().removeJsonObjectBraces()
    var index = 0
    var name = ""
    var detailJson = "{}"

    while (index < objectBody.length) {
        index = objectBody.skipWhitespaceAndCommas(index)
        if (index >= objectBody.length) break
        val key = objectBody.readJsonString(index)
        index = objectBody.skipWhitespace(key.nextIndex)
        require(index < objectBody.length && objectBody[index] == ':') {
            "Invalid core event envelope: missing ':' after ${key.value}"
        }
        index = objectBody.skipWhitespace(index + 1)
        when (key.value) {
            "event" -> {
                val value = objectBody.readJsonString(index)
                name = value.value
                index = value.nextIndex
            }
            "detail" -> {
                val value = objectBody.readJsonValue(index)
                detailJson = value.value
                index = value.nextIndex
            }
            else -> {
                index = objectBody.readJsonValue(index).nextIndex
            }
        }
    }

    return CoreEvent(name = name, detailJson = detailJson)
}

interface SecretStore
interface FilePicker
interface OAuthLauncher
interface NotificationService
interface BackgroundSyncScheduler
interface MailtoHandler

private data class JsonSlice(
    val value: String,
    val nextIndex: Int,
)

private fun String.removeJsonObjectBraces(): String {
    require(startsWith("{") && endsWith("}")) {
        "Invalid core event envelope: expected JSON object"
    }
    return substring(1, lastIndex)
}

private fun String.skipWhitespaceAndCommas(start: Int): Int {
    var index = start
    while (index < length && (this[index].isWhitespace() || this[index] == ',')) index += 1
    return index
}

private fun String.skipWhitespace(start: Int): Int {
    var index = start
    while (index < length && this[index].isWhitespace()) index += 1
    return index
}

private fun String.readJsonString(start: Int): JsonSlice {
    require(start < length && this[start] == '"') {
        "Invalid core event envelope: expected JSON string"
    }
    val out = StringBuilder()
    var index = start + 1
    while (index < length) {
        val ch = this[index]
        when (ch) {
            '"' -> return JsonSlice(out.toString(), index + 1)
            '\\' -> {
                require(index + 1 < length) { "Invalid core event envelope: dangling escape" }
                val escaped = this[index + 1]
                when (escaped) {
                    '"', '\\', '/' -> out.append(escaped)
                    'b' -> out.append('\b')
                    'f' -> out.append('\u000c')
                    'n' -> out.append('\n')
                    'r' -> out.append('\r')
                    't' -> out.append('\t')
                    'u' -> {
                        require(index + 5 < length) { "Invalid core event envelope: short unicode escape" }
                        val hex = substring(index + 2, index + 6)
                        out.append(hex.toInt(16).toChar())
                        index += 4
                    }
                    else -> error("Invalid core event envelope: bad escape \\$escaped")
                }
                index += 2
            }
            else -> {
                out.append(ch)
                index += 1
            }
        }
    }
    error("Invalid core event envelope: unterminated JSON string")
}

private fun String.readJsonValue(start: Int): JsonSlice {
    require(start < length) { "Invalid core event envelope: expected JSON value" }
    return when (this[start]) {
        '"' -> {
            val end = findJsonStringEnd(start)
            JsonSlice(substring(start, end), end)
        }
        '{' -> readBalancedJson(start, '{', '}')
        '[' -> readBalancedJson(start, '[', ']')
        else -> {
            var index = start
            while (index < length && this[index] != ',') index += 1
            JsonSlice(substring(start, index).trim(), index)
        }
    }
}

private fun String.findJsonStringEnd(start: Int): Int {
    var index = start + 1
    while (index < length) {
        when (this[index]) {
            '"' -> return index + 1
            '\\' -> index += 2
            else -> index += 1
        }
    }
    error("Invalid core event envelope: unterminated JSON string value")
}

private fun String.readBalancedJson(start: Int, open: Char, close: Char): JsonSlice {
    var depth = 0
    var index = start
    var inString = false
    while (index < length) {
        val ch = this[index]
        if (inString) {
            when (ch) {
                '"' -> inString = false
                '\\' -> index += 1
            }
        } else {
            when (ch) {
                '"' -> inString = true
                open -> depth += 1
                close -> {
                    depth -= 1
                    if (depth == 0) return JsonSlice(substring(start, index + 1), index + 1)
                }
            }
        }
        index += 1
    }
    error("Invalid core event envelope: unbalanced JSON value")
}
