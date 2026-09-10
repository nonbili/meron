package jp.nonbili.meron.shared

// The core reports failures as a {"error":{"message":...}} JSON payload rather
// than throwing, so a raw invoke "succeeds" even when the action failed.
private val coreErrorMessageRegex =
    Regex("\"error\"\\s*:\\s*\\{[^{}]*\"message\"\\s*:\\s*\"((?:[^\"\\\\]|\\\\.)*)\"")

/** The error message from a core `{"error":{"message":...}}` payload, or null. */
fun coreErrorMessage(responseJson: String): String? =
    coreErrorMessageRegex.find(responseJson)?.groupValues?.getOrNull(1)?.let { raw ->
        raw.replace("\\\"", "\"").replace("\\n", "\n").replace("\\\\", "\\")
    }

/**
 * Throws when [responseJson] is a core error payload, so a caller's
 * runCatching/onFailure path runs instead of silently committing an optimistic
 * change the server rejected. Returns the response unchanged on success.
 */
fun requireCoreOk(responseJson: String): String {
    coreErrorMessage(responseJson)?.let { throw RuntimeException(it) }
    return responseJson
}

/**
 * True when a core failure message says the mail server rejected our OAuth
 * credentials — the shapes meron-core produces for an expired/revoked access
 * token: "oauth login failed: ... [AUTHENTICATIONFAILED] Invalid credentials"
 * from IMAP XOAUTH2, and "smtp auth: ..." from an SMTP AUTH rejection. Used to
 * decide whether re-minting a host-managed Google token and retrying once is
 * worthwhile.
 */
fun isOAuthLoginFailure(message: String?): Boolean {
    val normalized = message?.lowercase() ?: return false
    return "oauth login failed" in normalized ||
        "authenticationfailed" in normalized ||
        "invalid credentials" in normalized ||
        "smtp auth" in normalized
}

data class ThreadActionLocation(
    val threadId: String = "",
    val folder: String = "",
    val permanent: Boolean = false,
)

fun parseThreadActionLocationResponse(responseJson: String): ThreadActionLocation =
    ThreadActionLocation(
        threadId = responseJson.findJsonStringProperty("thread_id").orEmpty(),
        folder = responseJson.findJsonStringProperty("folder") ?: responseJson.findJsonStringProperty("trash").orEmpty(),
        permanent = responseJson.findJsonBooleanProperty("permanent") ?: false,
    )

fun parseAllocatedMessageId(responseJson: String): String = responseJson.findJsonStringProperty("message_id").orEmpty()

data class FolderUnreadChange(
    val accountId: String,
    val folderId: String,
    val unread: Int,
)

fun parseFolderUnreadChanges(responseJson: String): List<FolderUnreadChange> =
    responseJson
        .findJsonArrayProperty("folder_counts")
        ?.jsonArrayElements()
        ?.mapNotNull { item ->
            val accountId = item.findJsonStringProperty("account_id").orEmpty()
            val folderId = item.findJsonStringProperty("folder_id").orEmpty()
            val unread = item.findJsonLongProperty("unread")?.toInt() ?: return@mapNotNull null
            if (accountId.isBlank() || folderId.isBlank()) null else FolderUnreadChange(accountId, folderId, unread)
        }.orEmpty()

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
            username = item.findJsonStringProperty("username").orEmpty(),
            imapHost = item.findJsonStringProperty("imap_host").orEmpty(),
            imapPort = item.findJsonLongProperty("imap_port")?.toInt() ?: 0,
            smtpHost = item.findJsonStringProperty("smtp_host").orEmpty(),
            smtpPort = item.findJsonLongProperty("smtp_port")?.toInt() ?: 0,
            tls = item.findJsonBooleanProperty("tls") ?: true,
            starttls = item.findJsonBooleanProperty("starttls") ?: false,
            smtpTls = item.findJsonBooleanProperty("smtp_tls") ?: true,
            smtpStarttls = item.findJsonBooleanProperty("smtp_starttls") ?: false,
            loadRemoteImages = item.findJsonBooleanProperty("load_remote_images") ?: false,
            includedInUnified = item.findJsonBooleanProperty("included_in_unified") ?: true,
            muted = item.findJsonBooleanProperty("muted") ?: false,
            paused = item.findJsonBooleanProperty("paused") ?: false,
            conversationHtml = item.findJsonBooleanProperty("conversation_html") ?: true,
            saveSentCopy = item.findJsonBooleanProperty("save_sent_copy"),
            rssSyncIntervalMinutes = item.findJsonLongProperty("rss_sync_interval_minutes")?.toInt() ?: 60,
            aliases =
                item
                    .findJsonArrayProperty("aliases")
                    ?.jsonArrayElements()
                    ?.mapNotNull { aliasJson ->
                        val email = aliasJson.findJsonStringProperty("email").orEmpty()
                        if (email.isBlank()) {
                            null
                        } else {
                            AccountAlias(
                                email = email,
                                name = aliasJson.findJsonStringProperty("name").orEmpty(),
                            )
                        }
                    }.orEmpty(),
            proxy = parseProxySpec(item.findJsonPropertyValue("proxy"), ProxySpec.followApp),
            signature = parseSignatureSpec(item.findJsonPropertyValue("signature")),
            chatWallpaperKind = wallpaperJson.findJsonStringProperty("kind").orEmpty(),
            chatWallpaperPresetId = wallpaperJson.findJsonStringProperty("presetId").orEmpty(),
            chatWallpaperUrl = wallpaperJson.findJsonStringProperty("url").orEmpty(),
        )
    }
}

/**
 * Read an account's signature override. Absent (or a mode this build does not
 * know) means the account follows the app-wide signature.
 */
private fun parseSignatureSpec(signatureJson: String?): SignatureSpec {
    val json = signatureJson?.takeIf { it.isNotBlank() && it != "null" } ?: return SignatureSpec.followApp
    val mode = json.findJsonStringProperty("mode").orEmpty()
    if (mode != "global" && mode != "none" && mode != "custom") return SignatureSpec.followApp
    return SignatureSpec(mode = mode, html = json.findJsonStringProperty("html").orEmpty())
}

/** The app-wide proxy from an `app.proxyGet` response; missing means none. */
fun parseProxyResponse(responseJson: String): ProxySpec = parseProxySpec(responseJson.findJsonPropertyValue("proxy"), ProxySpec.off)

/**
 * Read a proxy object, falling back to [fallback] when it is absent or carries
 * a mode this build does not know. Accounts stored before proxy support have no
 * such object at all, which is why the fallback is the caller's business.
 */
private fun parseProxySpec(
    proxyJson: String?,
    fallback: ProxySpec,
): ProxySpec {
    val json = proxyJson?.takeIf { it.isNotBlank() && it != "null" } ?: return fallback
    val mode = json.findJsonStringProperty("mode").orEmpty()
    if (mode.isBlank()) return fallback
    return ProxySpec(
        mode = mode,
        host = json.findJsonStringProperty("host").orEmpty(),
        port = json.findJsonLongProperty("port")?.toInt() ?: 0,
        username = json.findJsonStringProperty("username").orEmpty(),
        password = json.findJsonStringProperty("password").orEmpty(),
    )
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
            role = item.findJsonStringProperty("role") ?: "folder",
            // Absent for RSS folders and older cores; the wire name is then the
            // best label available.
            displayName = item.findJsonStringProperty("display_name")?.takeIf { it.isNotBlank() } ?: name,
            delimiter = item.findJsonStringProperty("delimiter").orEmpty(),
        )
    }
}

data class FolderDeleteResult(
    val removed: Set<String>,
    val warning: String?,
)

fun parseFolderDeleteResponse(responseJson: String): FolderDeleteResult {
    val removed =
        responseJson
            .findJsonArrayProperty("removed")
            ?.jsonArrayElements()
            ?.mapNotNull { item ->
                item
                    .takeIf { it.startsWith('"') }
                    ?.readJsonString(0)
                    ?.value
                    ?.takeIf { it.isNotBlank() }
            }?.toSet()
            .orEmpty()
    return FolderDeleteResult(
        removed = removed,
        warning = responseJson.findJsonStringProperty("warning")?.takeIf { it.isNotBlank() },
    )
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

/**
 * A server certificate the user is asked to trust. [fingerprint] is the hex
 * SHA-256 of the certificate, and the value pinned when they accept.
 */
data class ServerCertificate(
    val fingerprint: String,
    val subject: String,
    val issuer: String,
    val notBefore: String,
    val notAfter: String,
    val selfSigned: Boolean,
)

fun parseProbeCertResponse(responseJson: String): ServerCertificate? {
    val certJson = responseJson.findJsonObjectProperty("certificate") ?: return null
    val fingerprint = certJson.findJsonStringProperty("fingerprint").orEmpty()
    if (fingerprint.isBlank()) return null
    return ServerCertificate(
        fingerprint = fingerprint,
        subject = certJson.findJsonStringProperty("subject").orEmpty(),
        issuer = certJson.findJsonStringProperty("issuer").orEmpty(),
        notBefore = certJson.findJsonStringProperty("not_before").orEmpty(),
        notAfter = certJson.findJsonStringProperty("not_after").orEmpty(),
        selfSigned = certJson.findJsonBooleanProperty("self_signed") ?: false,
    )
}

data class ThreadListPage(
    val threads: List<ThreadSummary>,
    val nextCursor: String,
    val folderUnread: Int? = null,
    val folderSynced: Boolean? = null,
)

fun parseThreadListPage(responseJson: String): ThreadListPage {
    val threadsJson =
        responseJson.findJsonArrayProperty("threads")
            ?: return ThreadListPage(
                threads = emptyList(),
                nextCursor = "",
                folderUnread = responseJson.findJsonLongProperty("folder_unread")?.toInt(),
                folderSynced = responseJson.findJsonBooleanProperty("folder_synced"),
            )
    val threads =
        threadsJson.jsonArrayElements().mapNotNull { item ->
            val id = item.findJsonStringProperty("id").orEmpty()
            if (id.isBlank()) return@mapNotNull null
            ThreadSummary(
                id = id,
                accountId = item.findJsonStringProperty("account_id").orEmpty(),
                folder = item.findJsonStringProperty("folder_id") ?: item.findJsonStringProperty("folder").orEmpty(),
                folderRole = item.findJsonStringProperty("folder_role") ?: "folder",
                subject = item.findJsonStringProperty("subject").orEmpty(),
                // The envelope carries an empty from_name when the sender has
                // no display name, so fall through blanks to the address rather
                // than leaving the label empty for the row to fill in.
                sender =
                    item
                        .findJsonStringProperty("from_name")
                        .orEmpty()
                        .ifBlank { item.findJsonStringProperty("from_addr").orEmpty() }
                        .ifBlank { item.findJsonStringProperty("from").orEmpty() },
                preview = item.findJsonStringProperty("preview").orEmpty(),
                unread = item.findJsonBooleanProperty("unread") ?: false,
                unreadCount =
                    item.findJsonLongProperty("unread_count")?.toInt()
                        ?: if (item.findJsonBooleanProperty("unread") == true) 1 else 0,
                messageCount = item.findJsonLongProperty("message_count")?.toInt() ?: 0,
                starred = item.findJsonBooleanProperty("starred") ?: false,
                hasStarredItems = item.findJsonBooleanProperty("has_starred_items") ?: false,
                hasDraft = item.findJsonBooleanProperty("has_draft") ?: false,
                dateEpochSeconds = item.findJsonLongProperty("date") ?: item.findJsonLongProperty("date_epoch_seconds") ?: 0,
                feedUrl = item.findJsonStringProperty("feed_url").orEmpty(),
            )
        }
    return ThreadListPage(
        // Thread ids key the LazyColumn items; a duplicate id in one core
        // response (e.g. unified or search listings) would crash composition.
        threads = threads.distinctBy { it.id },
        nextCursor = responseJson.findJsonStringProperty("next_cursor").orEmpty(),
        folderUnread = responseJson.findJsonLongProperty("folder_unread")?.toInt(),
        folderSynced = responseJson.findJsonBooleanProperty("folder_synced"),
    )
}

fun parseThreadListResponse(responseJson: String): List<ThreadSummary> = parseThreadListPage(responseJson).threads

data class ThreadReadPage(
    val messages: List<MessageBody>,
    val nextCursor: String,
)

data class StarredItemsPage(
    val items: List<StarredItemSummary>,
    val nextCursor: String,
)

fun parseStarredItemsPage(responseJson: String): StarredItemsPage =
    StarredItemsPage(
        items = parseStarredItemsResponse(responseJson),
        nextCursor = responseJson.findJsonStringProperty("next_cursor").orEmpty(),
    )

fun parseStarredItemsResponse(responseJson: String): List<StarredItemSummary> {
    val itemsJson = responseJson.findJsonArrayProperty("items") ?: return emptyList()
    return itemsJson
        .jsonArrayElements()
        .mapNotNull { item ->
            val id = item.findJsonStringProperty("id").orEmpty()
            val threadId = item.findJsonStringProperty("thread_id").orEmpty()
            if (id.isBlank() || threadId.isBlank()) return@mapNotNull null
            StarredItemSummary(
                id = id,
                threadId = threadId,
                accountId = item.findJsonStringProperty("account_id").orEmpty(),
                folder = item.findJsonStringProperty("folder_id") ?: item.findJsonStringProperty("folder").orEmpty(),
                folderRole = item.findJsonStringProperty("folder_role") ?: "folder",
                subject = item.findJsonStringProperty("subject").orEmpty(),
                // The envelope carries an empty from_name when the sender has
                // no display name, so fall through blanks to the address rather
                // than leaving the label empty for the row to fill in.
                sender =
                    item
                        .findJsonStringProperty("from_name")
                        .orEmpty()
                        .ifBlank { item.findJsonStringProperty("from_addr").orEmpty() }
                        .ifBlank { item.findJsonStringProperty("from").orEmpty() },
                preview = item.findJsonStringProperty("preview").orEmpty(),
                unread = item.findJsonBooleanProperty("unread") ?: false,
                dateEpochSeconds = item.findJsonLongProperty("date") ?: item.findJsonLongProperty("date_epoch_seconds") ?: 0,
            )
        }.distinctBy { it.id }
}

fun parseThreadReadPage(responseJson: String): ThreadReadPage {
    val messagesJson =
        responseJson.findJsonArrayProperty("messages")
            ?: return ThreadReadPage(messages = emptyList(), nextCursor = "")
    val messages =
        messagesJson.jsonArrayElements().mapNotNull { item ->
            val id = item.findJsonStringProperty("id").orEmpty()
            if (id.isBlank()) return@mapNotNull null
            val fromName = item.findJsonStringProperty("from_name").orEmpty()
            val fromAddr = item.findJsonStringProperty("from_addr").orEmpty()
            // Read the reply recipients out of the message's *top-level* keys.
            // The scanning helpers below match the first occurrence anywhere in
            // the object, and the body — attacker-written text — is serialized
            // ahead of this key, so a mail whose body contains its own
            // "reply": {...} would otherwise choose who the user replies to.
            val replyObject = item.jsonObjectEntries().firstOrNull { it.first == "reply" }?.second
            MessageBody(
                id = id,
                folderId = item.findJsonStringProperty("folder_id") ?: item.findJsonStringProperty("folder").orEmpty(),
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
                inReplyTo = item.findJsonStringProperty("in_reply_to").orEmpty(),
                references = item.findJsonStringProperty("references").orEmpty(),
                unread = item.findJsonBooleanProperty("unread") ?: false,
                outgoing = item.findJsonBooleanProperty("outgoing") ?: false,
                starred = item.findJsonBooleanProperty("starred") ?: false,
                hasAttachments = item.findJsonBooleanProperty("has_attachments") ?: false,
                bodyMissing = item.findJsonBooleanProperty("body_missing") ?: false,
                reply =
                    replyObject?.takeIf { it.startsWith("{") }?.let { reply ->
                        MessageReply(
                            to = reply.findJsonStringProperty("to").orEmpty(),
                            cc = reply.findJsonStringProperty("cc").orEmpty(),
                            allTo = reply.findJsonStringProperty("all_to").orEmpty(),
                            allCc = reply.findJsonStringProperty("all_cc").orEmpty(),
                            allAddsRecipients = reply.findJsonBooleanProperty("all_adds_recipients") ?: false,
                        )
                    },
                attachments =
                    item
                        .findJsonArrayProperty("attachments")
                        ?.jsonArrayElements()
                        ?.mapNotNull { attachmentJson ->
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
        // Message ids key the conversation LazyColumn items; duplicates crash.
        messages = messages.distinctBy { it.id },
        nextCursor = responseJson.findJsonStringProperty("next_cursor").orEmpty(),
    )
}

fun parseThreadReadResponse(responseJson: String): List<MessageBody> = parseThreadReadPage(responseJson).messages

fun parseOpmlExportResponse(responseJson: String): String = responseJson.findJsonStringProperty("opml").orEmpty()

fun parseOpmlImportCountResponse(responseJson: String): Int = responseJson.findJsonLongProperty("imported")?.toInt() ?: 0

/** The serialized backup document from `backup.export`. */
fun parseBackupExportResponse(responseJson: String): String = responseJson.findJsonStringProperty("backup").orEmpty()

/**
 * What a restore did, or that the file is encrypted and needs a passphrase.
 * [needsPassphrase] is not an error: the host prompts and calls again.
 */
data class BackupImportResult(
    val needsPassphrase: Boolean = false,
    val accounts: Int = 0,
    /** Accounts already present locally, which a restore never overwrites. */
    val skipped: Int = 0,
    val feeds: Int = 0,
    val settings: Int = 0,
    val secrets: Int = 0,
)

fun parseBackupImportResponse(responseJson: String): BackupImportResult {
    if (responseJson.findJsonBooleanProperty("needs_passphrase") == true) {
        return BackupImportResult(needsPassphrase = true)
    }
    return BackupImportResult(
        accounts = responseJson.findJsonLongProperty("accounts")?.toInt() ?: 0,
        skipped = responseJson.findJsonLongProperty("skipped")?.toInt() ?: 0,
        feeds = responseJson.findJsonLongProperty("feeds")?.toInt() ?: 0,
        settings = responseJson.findJsonLongProperty("settings")?.toInt() ?: 0,
        secrets = responseJson.findJsonLongProperty("secrets")?.toInt() ?: 0,
    )
}

/**
 * Settings read from the core store (`app.prefsGet`). Values are the JSON
 * primitives the mobile pref cache holds — `String`, `Boolean`, `Long`, or
 * `List<String>`; anything else is skipped rather than guessed at.
 *
 * A key the store has never held is simply absent, which is what lets the host
 * keep its own cached value instead of resetting it to a type default.
 */
fun parseAppPrefsResponse(responseJson: String): Map<String, Any> {
    val objectJson = responseJson.findJsonObjectProperty("prefs") ?: return emptyMap()
    val out = LinkedHashMap<String, Any>()
    for ((key, raw) in objectJson.jsonObjectEntries()) {
        val value: Any =
            when {
                raw.startsWith('"') -> {
                    raw.readJsonString(0).value
                }

                raw == "true" -> {
                    true
                }

                raw == "false" -> {
                    false
                }

                raw.startsWith('[') -> {
                    raw
                        .jsonArrayElements()
                        .filter { it.startsWith('"') }
                        .map { it.readJsonString(0).value }
                }

                raw.toLongOrNull() != null -> {
                    raw.toLong()
                }

                // null, a nested object, or a float: nothing the pref store takes.
                else -> {
                    continue
                }
            }
        out[key] = value
    }
    return out
}

/** Encode one setting value for `AppPrefsSetParams`. */
fun encodeAppPrefValue(value: Any): String =
    when (value) {
        is String -> {
            value.jsonString()
        }

        is Boolean -> {
            value.toString()
        }

        is Int, is Long -> {
            value.toString()
        }

        is Collection<*> -> {
            value
                .filterIsInstance<String>()
                .joinToString(separator = ",", prefix = "[", postfix = "]") { it.jsonString() }
        }

        // Keep the payload well-formed rather than emitting a raw toString()
        // the reader would reject.
        else -> {
            value.toString().jsonString()
        }
    }

fun parseMediaFileUrlResponse(responseJson: String): String = responseJson.findJsonStringProperty("url").orEmpty()

fun parseAttachmentDataResponse(responseJson: String): String = responseJson.findJsonStringProperty("data").orEmpty()

fun parseChangelogResponse(responseJson: String): List<ChangelogRelease> {
    val releasesJson = responseJson.findJsonArrayProperty("releases") ?: return emptyList()
    return releasesJson.jsonArrayElements().mapNotNull { item ->
        val tag = item.findJsonStringProperty("tag").orEmpty()
        val version = item.findJsonStringProperty("version").orEmpty()
        if (tag.isBlank() && version.isBlank()) return@mapNotNull null
        val notes =
            item
                .findJsonArrayProperty("notes")
                ?.jsonArrayElements()
                ?.map { note -> if (note.startsWith('"')) note.readJsonString(0).value else note }
                ?.filter { it.isNotBlank() }
                .orEmpty()
        ChangelogRelease(
            version = version,
            tag = tag,
            date = item.findJsonStringProperty("date").orEmpty(),
            notes = notes,
        )
    }
}

fun parseStorageUsageResponse(responseJson: String): StorageUsage =
    StorageUsage(
        cacheBytes = responseJson.findJsonLongProperty("cacheBytes") ?: 0,
        dbBytes = responseJson.findJsonLongProperty("dbBytes") ?: 0,
    )

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

private fun String.findJsonObjectProperty(name: String): String? {
    val key = name.jsonString()
    var index = 0
    while (index < length) {
        val found = indexOf(key, startIndex = index)
        if (found < 0) return null
        var cursor = skipWhitespace(found + key.length)
        if (cursor < length && this[cursor] == ':') {
            cursor = skipWhitespace(cursor + 1)
            if (cursor < length && this[cursor] == '{') {
                return readBalancedJson(cursor, '{', '}').value
            }
        }
        index = found + key.length
    }
    return null
}

/** Top-level `"key": value` pairs of a JSON object, values left unparsed. */
private fun String.jsonObjectEntries(): List<Pair<String, String>> {
    val body = trim().removePrefix("{").removeSuffix("}")
    val out = mutableListOf<Pair<String, String>>()
    var index = 0
    while (index < body.length) {
        index = body.skipWhitespaceAndCommas(index)
        if (index >= body.length || body[index] != '"') break
        val key = body.readJsonString(index)
        index = body.skipWhitespace(key.nextIndex)
        if (index >= body.length || body[index] != ':') break
        index = body.skipWhitespace(index + 1)
        if (index >= body.length) break
        val value = body.readJsonValue(index)
        out += key.value to value.value
        index = value.nextIndex
    }
    return out
}

private fun String.findJsonStringProperty(name: String): String? {
    val value = findJsonPropertyValue(name) ?: return null
    if (!value.startsWith('"')) return null
    return value.readJsonString(0).value
}

private fun String.findJsonBooleanProperty(name: String): Boolean? =
    when (findJsonPropertyValue(name)) {
        "true" -> true
        "false" -> false
        else -> null
    }

private fun String.findJsonLongProperty(name: String): Long? = findJsonPropertyValue(name)?.toLongOrNull()

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
            '"' -> {
                return ResponseJsonSlice(out.toString(), index + 1)
            }

            '\\' -> {
                require(index + 1 < length) { "Dangling JSON escape" }
                when (val escaped = this[index + 1]) {
                    '"', '\\', '/' -> {
                        out.append(escaped)
                    }

                    'b' -> {
                        out.append('\b')
                    }

                    'f' -> {
                        out.append('\u000c')
                    }

                    'n' -> {
                        out.append('\n')
                    }

                    'r' -> {
                        out.append('\r')
                    }

                    't' -> {
                        out.append('\t')
                    }

                    'u' -> {
                        require(index + 5 < length) { "Short unicode escape" }
                        out.append(substring(index + 2, index + 6).toInt(16).toChar())
                        index += 4
                    }

                    else -> {
                        error("Bad JSON escape: \\$escaped")
                    }
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

        '{' -> {
            readBalancedJson(start, '{', '}')
        }

        '[' -> {
            readBalancedJson(start, '[', ']')
        }

        else -> {
            var index = start
            while (index < length && this[index] != ',' && this[index] != '}' && this[index] != ']') index += 1
            ResponseJsonSlice(substring(start, index).trim(), index)
        }
    }
}

private fun String.readBalancedJson(
    start: Int,
    open: Char,
    close: Char,
): ResponseJsonSlice {
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
                '"' -> {
                    inString = true
                }

                open -> {
                    depth += 1
                }

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
