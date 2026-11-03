package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.MessageAttachment

internal fun remoteHtmlImageAttachment(url: String): MessageAttachment {
    val encodedFilename =
        url
            .substringBefore('?')
            .substringBefore('#')
            .substringAfterLast('/')
            .ifBlank { "image" }
    val filename = decodeImageFilename(encodedFilename).replace('/', '_').replace('\\', '_')
    val mimeType =
        when (filename.substringAfterLast('.', "").lowercase()) {
            "jpg", "jpeg" -> "image/jpeg"
            "png" -> "image/png"
            "gif" -> "image/gif"
            "webp" -> "image/webp"
            "avif" -> "image/avif"
            "svg" -> "image/svg+xml"
            else -> "image/*"
        }
    return MessageAttachment(filename = filename, mimeType = mimeType, url = url)
}

internal fun remoteImageFormat(bytes: ByteArray): Pair<String, String>? {
    fun starts(vararg signature: Int) = bytes.size >= signature.size && signature.indices.all { bytes[it] == signature[it].toByte() }

    fun ascii(
        offset: Int,
        text: String,
    ) = bytes.size >= offset + text.length && text.indices.all { bytes[offset + it] == text[it].code.toByte() }
    val head = bytes.copyOfRange(0, minOf(bytes.size, 512)).decodeToString()
    return when {
        starts(0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a) -> "image/png" to "png"
        starts(0xff, 0xd8, 0xff) -> "image/jpeg" to "jpg"
        head.startsWith("GIF87a") || head.startsWith("GIF89a") -> "image/gif" to "gif"
        ascii(0, "RIFF") && ascii(8, "WEBP") -> "image/webp" to "webp"
        starts(0x42, 0x4d) -> "image/bmp" to "bmp"
        ascii(4, "ftyp") && (ascii(8, "avif") || ascii(8, "avis")) -> "image/avif" to "avif"
        head.trimStart().let { it.startsWith("<svg") || (it.startsWith("<?xml") && it.contains("<svg")) } -> "image/svg+xml" to "svg"
        else -> null
    }
}

private fun decodeImageFilename(name: String): String =
    runCatching {
        val bytes = mutableListOf<Byte>()
        var index = 0
        while (index < name.length) {
            if (name[index] == '%') {
                bytes.add(name.substring(index + 1, index + 3).toInt(16).toByte())
                index += 3
            } else {
                val end = name.indexOf('%', index).takeIf { it >= 0 } ?: name.length
                bytes.addAll(name.substring(index, end).encodeToByteArray().toList())
                index = end
            }
        }
        bytes.toByteArray().decodeToString(throwOnInvalidSequence = true)
    }.getOrDefault(name)

internal fun normalizedRemoteImage(
    attachment: MessageAttachment,
    bytes: ByteArray,
): MessageAttachment {
    if (attachment.key.isNotBlank() || attachment.url.isBlank()) return attachment
    val (mime, extension) = remoteImageFormat(bytes) ?: error("Unsupported remote image")
    val name = attachment.filename.ifBlank { "image" }
    val suffix = name.substringAfterLast('.', "").lowercase()
    val filename = if (suffix == extension || (mime == "image/jpeg" && suffix == "jpeg")) name else "$name.$extension"
    return attachment.copy(filename = filename, mimeType = mime)
}
