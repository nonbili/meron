package jp.nonbili.meron.shared

fun parseAccountListResponse(responseJson: String): List<AccountSummary> {
    val accountsJson = responseJson.findJsonArrayProperty("accounts") ?: return emptyList()
    return accountsJson.jsonArrayElements().mapNotNull { item ->
        val id = item.findJsonStringProperty("id").orEmpty()
        if (id.isBlank()) return@mapNotNull null
        val wallpaperJson = item.findJsonPropertyValue("chat_wallpaper").orEmpty()
        AccountSummary(
            id = id,
            email = item.findJsonStringProperty("email").orEmpty(),
            displayName = item.findJsonStringProperty("display_name").orEmpty(),
            senderName = item.findJsonStringProperty("sender_name").orEmpty(),
            avatarUrl = item.findJsonStringProperty("avatar_url").orEmpty(),
            needsReconnect = item.findJsonBooleanProperty("needs_reconnect") ?: false,
            engine = item.findJsonStringProperty("engine").orEmpty(),
            provider = item.findJsonStringProperty("provider").orEmpty(),
            authType = item.findJsonStringProperty("auth_type").orEmpty(),
            imapHost = item.findJsonStringProperty("imap_host").orEmpty(),
            imapPort = item.findJsonLongProperty("imap_port")?.toInt() ?: 0,
            smtpHost = item.findJsonStringProperty("smtp_host").orEmpty(),
            smtpPort = item.findJsonLongProperty("smtp_port")?.toInt() ?: 0,
            loadRemoteImages = item.findJsonBooleanProperty("load_remote_images") ?: false,
            includedInUnified = item.findJsonBooleanProperty("included_in_unified") ?: true,
            muted = item.findJsonBooleanProperty("muted") ?: false,
            paused = item.findJsonBooleanProperty("paused") ?: false,
            conversationHtml = item.findJsonBooleanProperty("conversation_html") ?: true,
            rssSyncIntervalMinutes = item.findJsonLongProperty("rss_sync_interval_minutes")?.toInt() ?: 60,
            aliases = item.findJsonArrayProperty("aliases")?.jsonArrayElements()?.mapNotNull { aliasJson ->
                val email = aliasJson.findJsonStringProperty("email").orEmpty()
                if (email.isBlank()) null else AccountAlias(
                    email = email,
                    name = aliasJson.findJsonStringProperty("name").orEmpty(),
                )
            }.orEmpty(),
            chatWallpaperKind = wallpaperJson.findJsonStringProperty("kind").orEmpty(),
            chatWallpaperPresetId = wallpaperJson.findJsonStringProperty("presetId").orEmpty(),
            chatWallpaperUrl = wallpaperJson.findJsonStringProperty("url").orEmpty(),
        )
    }
}

fun parseFolderListResponse(responseJson: String): List<FolderSummary> {
    val foldersJson = responseJson.findJsonArrayProperty("folders") ?: return emptyList()
    return foldersJson.jsonArrayElements().mapNotNull { item ->
        val name = item.findJsonStringProperty("name") ?: item.findJsonStringProperty("id").orEmpty()
        if (name.isBlank()) return@mapNotNull null
        FolderSummary(
            accountId = item.findJsonStringProperty("account_id").orEmpty(),
            name = name,
            unread = item.findJsonLongProperty("unread")?.toInt() ?: 0,
        )
    }
}

fun parseContactSuggestResponse(responseJson: String): List<ContactSuggestion> {
    val contactsJson = responseJson.findJsonArrayProperty("contacts") ?: return emptyList()
    return contactsJson.jsonArrayElements().mapNotNull { item ->
        val addr = item.findJsonStringProperty("addr").orEmpty()
        if (addr.isBlank()) return@mapNotNull null
        ContactSuggestion(
            name = item.findJsonStringProperty("name").orEmpty(),
            addr = addr,
        )
    }
}

data class DiscoveredAccountSettings(
    val imapHost: String,
    val imapPort: Int,
    val smtpHost: String,
    val smtpPort: Int,
    val username: String,
    val providerName: String,
    val source: String,
    val appPasswordProvider: String,
    val appPasswordUrl: String,
)

fun parseAutodiscoverResponse(responseJson: String): DiscoveredAccountSettings {
    val hintJson = responseJson.findJsonPropertyValue("app_password_hint").orEmpty()
    return DiscoveredAccountSettings(
        imapHost = responseJson.findJsonStringProperty("imap_host").orEmpty(),
        imapPort = responseJson.findJsonLongProperty("imap_port")?.toInt() ?: 993,
        smtpHost = responseJson.findJsonStringProperty("smtp_host").orEmpty(),
        smtpPort = responseJson.findJsonLongProperty("smtp_port")?.toInt() ?: 465,
        username = responseJson.findJsonStringProperty("username").orEmpty(),
        providerName = responseJson.findJsonStringProperty("provider_name").orEmpty(),
        source = responseJson.findJsonStringProperty("source").orEmpty(),
        appPasswordProvider = hintJson.findJsonStringProperty("provider").orEmpty(),
        appPasswordUrl = hintJson.findJsonStringProperty("url").orEmpty(),
    )
}

data class ThreadListPage(
    val threads: List<ThreadSummary>,
    val nextCursor: String,
)

fun parseThreadListPage(responseJson: String): ThreadListPage {
    val threadsJson = responseJson.findJsonArrayProperty("threads")
        ?: return ThreadListPage(threads = emptyList(), nextCursor = "")
    val threads = threadsJson.jsonArrayElements().mapNotNull { item ->
        val id = item.findJsonStringProperty("id").orEmpty()
        if (id.isBlank()) return@mapNotNull null
        ThreadSummary(
            id = id,
            accountId = item.findJsonStringProperty("account_id").orEmpty(),
            folder = item.findJsonStringProperty("folder_id") ?: item.findJsonStringProperty("folder").orEmpty(),
            subject = item.findJsonStringProperty("subject").orEmpty(),
            sender = item.findJsonStringProperty("from_name") ?: item.findJsonStringProperty("from").orEmpty(),
            preview = item.findJsonStringProperty("preview").orEmpty(),
            unread = item.findJsonBooleanProperty("unread") ?: false,
            starred = item.findJsonBooleanProperty("starred") ?: false,
            dateEpochSeconds = item.findJsonLongProperty("date") ?: item.findJsonLongProperty("date_epoch_seconds") ?: 0,
            feedUrl = item.findJsonStringProperty("feed_url").orEmpty(),
        )
    }
    return ThreadListPage(
        threads = threads,
        nextCursor = responseJson.findJsonStringProperty("next_cursor").orEmpty(),
    )
}

fun parseThreadListResponse(responseJson: String): List<ThreadSummary> {
    return parseThreadListPage(responseJson).threads
}

data class ThreadReadPage(
    val messages: List<MessageBody>,
    val nextCursor: String,
)

fun parseStarredItemsResponse(responseJson: String): List<StarredItemSummary> {
    val itemsJson = responseJson.findJsonArrayProperty("items") ?: return emptyList()
    return itemsJson.jsonArrayElements().mapNotNull { item ->
        val id = item.findJsonStringProperty("id").orEmpty()
        val threadId = item.findJsonStringProperty("thread_id").orEmpty()
        if (id.isBlank() || threadId.isBlank()) return@mapNotNull null
        StarredItemSummary(
            id = id,
            threadId = threadId,
            accountId = item.findJsonStringProperty("account_id").orEmpty(),
            folder = item.findJsonStringProperty("folder_id") ?: item.findJsonStringProperty("folder").orEmpty(),
            subject = item.findJsonStringProperty("subject").orEmpty(),
            sender = item.findJsonStringProperty("from_name") ?: item.findJsonStringProperty("from").orEmpty(),
            preview = item.findJsonStringProperty("preview").orEmpty(),
            unread = item.findJsonBooleanProperty("unread") ?: false,
            dateEpochSeconds = item.findJsonLongProperty("date") ?: item.findJsonLongProperty("date_epoch_seconds") ?: 0,
        )
    }
}

fun parseThreadReadPage(responseJson: String): ThreadReadPage {
    val messagesJson = responseJson.findJsonArrayProperty("messages")
        ?: return ThreadReadPage(messages = emptyList(), nextCursor = "")
    val messages = messagesJson.jsonArrayElements().mapNotNull { item ->
        val id = item.findJsonStringProperty("id").orEmpty()
        if (id.isBlank()) return@mapNotNull null
        val fromName = item.findJsonStringProperty("from_name").orEmpty()
        val fromAddr = item.findJsonStringProperty("from_addr").orEmpty()
        MessageBody(
            id = id,
            from = fromName.ifBlank { fromAddr },
            to = item.findJsonStringProperty("to").orEmpty(),
            cc = item.findJsonStringProperty("cc").orEmpty(),
            bcc = item.findJsonStringProperty("bcc").orEmpty(),
            subject = item.findJsonStringProperty("subject").orEmpty(),
            body = item.findJsonStringProperty("body").orEmpty(),
            bodyHtml = item.findJsonStringProperty("body_html").orEmpty(),
            dateEpochSeconds = item.findJsonLongProperty("date") ?: item.findJsonLongProperty("date_epoch_seconds") ?: 0,
            fromAddr = fromAddr,
            replyTo = item.findJsonStringProperty("reply_to").orEmpty(),
            messageId = item.findJsonStringProperty("message_id").orEmpty(),
            references = item.findJsonStringProperty("references").orEmpty(),
            unread = item.findJsonBooleanProperty("unread") ?: false,
            starred = item.findJsonBooleanProperty("starred") ?: false,
            hasAttachments = item.findJsonBooleanProperty("has_attachments") ?: false,
            attachments = item.findJsonArrayProperty("attachments")?.jsonArrayElements()?.mapNotNull { attachmentJson ->
                val filename = attachmentJson.findJsonStringProperty("filename").orEmpty()
                if (filename.isBlank()) return@mapNotNull null
                MessageAttachment(
                    filename = filename,
                    mimeType = attachmentJson.findJsonStringProperty("mime").orEmpty(),
                    sizeBytes = attachmentJson.findJsonLongProperty("size") ?: 0,
                    key = attachmentJson.findJsonStringProperty("key").orEmpty(),
                    url = attachmentJson.findJsonStringProperty("url").orEmpty(),
                )
            }.orEmpty(),
        )
    }
    return ThreadReadPage(
        messages = messages,
        nextCursor = responseJson.findJsonStringProperty("next_cursor").orEmpty(),
    )
}

fun parseThreadReadResponse(responseJson: String): List<MessageBody> {
    return parseThreadReadPage(responseJson).messages
}

fun parseOpmlExportResponse(responseJson: String): String {
    return responseJson.findJsonStringProperty("opml").orEmpty()
}

fun parseOpmlImportCountResponse(responseJson: String): Int {
    return responseJson.findJsonLongProperty("imported")?.toInt() ?: 0
}

fun parseMediaFileUrlResponse(responseJson: String): String {
    return responseJson.findJsonStringProperty("url").orEmpty()
}

fun parseAttachmentDataResponse(responseJson: String): String {
    return responseJson.findJsonStringProperty("data").orEmpty()
}

fun parseStorageUsageResponse(responseJson: String): StorageUsage {
    return StorageUsage(
        cacheBytes = responseJson.findJsonLongProperty("cacheBytes") ?: 0,
        dbBytes = responseJson.findJsonLongProperty("dbBytes") ?: 0,
    )
}

private fun String.findJsonArrayProperty(name: String): String? {
    val key = name.jsonString()
    var index = 0
    while (index < length) {
        val found = indexOf(key, startIndex = index)
        if (found < 0) return null
        var cursor = skipWhitespace(found + key.length)
        if (cursor < length && this[cursor] == ':') {
            cursor = skipWhitespace(cursor + 1)
            if (cursor < length && this[cursor] == '[') {
                return readBalancedJson(cursor, '[', ']').value
            }
        }
        index = found + key.length
    }
    return null
}

private fun String.findJsonStringProperty(name: String): String? {
    val value = findJsonPropertyValue(name) ?: return null
    if (!value.startsWith('"')) return null
    return value.readJsonString(0).value
}

private fun String.findJsonBooleanProperty(name: String): Boolean? {
    return when (findJsonPropertyValue(name)) {
        "true" -> true
        "false" -> false
        else -> null
    }
}

private fun String.findJsonLongProperty(name: String): Long? {
    return findJsonPropertyValue(name)?.toLongOrNull()
}

private fun String.findJsonPropertyValue(name: String): String? {
    val key = name.jsonString()
    var index = 0
    while (index < length) {
        val found = indexOf(key, startIndex = index)
        if (found < 0) return null
        var cursor = skipWhitespace(found + key.length)
        if (cursor < length && this[cursor] == ':') {
            cursor = skipWhitespace(cursor + 1)
            return readJsonValue(cursor).value
        }
        index = found + key.length
    }
    return null
}

private data class ResponseJsonSlice(
    val value: String,
    val nextIndex: Int,
)

private fun String.jsonArrayElements(): List<String> {
    val body = trim().removePrefix("[").removeSuffix("]")
    val out = mutableListOf<String>()
    var index = 0
    while (index < body.length) {
        index = body.skipWhitespaceAndCommas(index)
        if (index >= body.length) break
        val value = body.readJsonValue(index)
        out += value.value
        index = value.nextIndex
    }
    return out
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

private fun String.readJsonString(start: Int): ResponseJsonSlice {
    require(start < length && this[start] == '"') { "Expected JSON string" }
    val out = StringBuilder()
    var index = start + 1
    while (index < length) {
        when (val ch = this[index]) {
            '"' -> return ResponseJsonSlice(out.toString(), index + 1)
            '\\' -> {
                require(index + 1 < length) { "Dangling JSON escape" }
                when (val escaped = this[index + 1]) {
                    '"', '\\', '/' -> out.append(escaped)
                    'b' -> out.append('\b')
                    'f' -> out.append('\u000c')
                    'n' -> out.append('\n')
                    'r' -> out.append('\r')
                    't' -> out.append('\t')
                    'u' -> {
                        require(index + 5 < length) { "Short unicode escape" }
                        out.append(substring(index + 2, index + 6).toInt(16).toChar())
                        index += 4
                    }
                    else -> error("Bad JSON escape: \\$escaped")
                }
                index += 2
            }
            else -> {
                out.append(ch)
                index += 1
            }
        }
    }
    error("Unterminated JSON string")
}

private fun String.readJsonValue(start: Int): ResponseJsonSlice {
    require(start < length) { "Expected JSON value" }
    return when (this[start]) {
        '"' -> {
            val string = readJsonString(start)
            ResponseJsonSlice(substring(start, string.nextIndex), string.nextIndex)
        }
        '{' -> readBalancedJson(start, '{', '}')
        '[' -> readBalancedJson(start, '[', ']')
        else -> {
            var index = start
            while (index < length && this[index] != ',' && this[index] != '}' && this[index] != ']') index += 1
            ResponseJsonSlice(substring(start, index).trim(), index)
        }
    }
}

private fun String.readBalancedJson(start: Int, open: Char, close: Char): ResponseJsonSlice {
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
                    if (depth == 0) return ResponseJsonSlice(substring(start, index + 1), index + 1)
                }
            }
        }
        index += 1
    }
    error("Unbalanced JSON value")
}
