package jp.nonbili.meron.shared

fun parseMailtoUrl(rawUrl: String): ComposeDraft? {
    val trimmed = rawUrl.trim()
    if (!trimmed.startsWith("mailto:", ignoreCase = true)) return null

    val payload = trimmed.substringAfter(':')
    val path = payload.substringBefore('?')
    val query = payload.substringAfter('?', missingDelimiterValue = "")
    val fields = parseMailtoQuery(query)

    val recipients = buildList {
        addAll(splitAddressField(percentDecode(path, plusAsSpace = false)))
        fields["to"]?.let { addAll(splitAddressField(it)) }
    }.distinctBy { it.lowercase() }

    return ComposeDraft(
        to = recipients.joinToString(", "),
        cc = fields["cc"].orEmpty(),
        bcc = fields["bcc"].orEmpty(),
        subject = fields["subject"].orEmpty(),
        body = fields["body"].orEmpty(),
    )
}

private fun parseMailtoQuery(query: String): Map<String, String> {
    if (query.isBlank()) return emptyMap()
    val fields = linkedMapOf<String, MutableList<String>>()
    query.split('&')
        .filter { it.isNotEmpty() }
        .forEach { pair ->
            val key = pair.substringBefore('=').lowercase()
            if (key.isBlank()) return@forEach
            val value = pair.substringAfter('=', missingDelimiterValue = "")
            fields.getOrPut(key) { mutableListOf() }.add(percentDecode(value, plusAsSpace = true))
        }
    return fields.mapValues { (key, values) ->
        if (key == "to" || key == "cc" || key == "bcc") {
            values.flatMap(::splitAddressField).distinctBy { it.lowercase() }.joinToString(", ")
        } else {
            values.lastOrNull().orEmpty()
        }
    }
}

private fun splitAddressField(value: String): List<String> {
    return value
        .split(',', ';')
        .map { it.trim() }
        .filter { it.isNotEmpty() }
}

private fun percentDecode(value: String, plusAsSpace: Boolean): String {
    val bytes = mutableListOf<Byte>()
    var index = 0
    while (index < value.length) {
        val ch = value[index]
        when {
            ch == '%' && index + 2 < value.length -> {
                val hex = value.substring(index + 1, index + 3)
                val decoded = hex.toIntOrNull(16)
                if (decoded == null) {
                    bytes.add(ch.code.toByte())
                } else {
                    bytes.add(decoded.toByte())
                    index += 2
                }
            }
            ch == '+' && plusAsSpace -> bytes.add(' '.code.toByte())
            else -> ch.toString().encodeToByteArray().forEach(bytes::add)
        }
        index += 1
    }
    return bytes.toByteArray().decodeToString()
}
