package jp.nonbili.meron.shared

import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

/** The folder name core stores the inbox under; the UI's own folder ids use a
 *  lowercase "inbox" that IMAP would not recognize. */
private const val CORE_INBOX_FOLDER = "INBOX"

/** Compose the `thread_id` core expects from the fields a notification carries.
 *
 *  Notification payloads carry the account, folder and bare thread key
 *  separately, but every `mail.*` thread method keys on the composite
 *  `account#folder#key` (see `parse_thread_id` in meron-core). Keys minted from
 *  message headers are base64url-encoded behind a `t.` prefix; uid-derived keys
 *  are written as the bare uid.
 *
 *  Lives here rather than in the UI so notification *actions* — which run in a
 *  background receiver with no UI state — build the same id the tap-through
 *  does. */
@OptIn(ExperimentalEncodingApi::class)
fun notificationThreadId(
    accountId: String,
    folder: String,
    threadKey: String,
): String {
    // A feed is a thread, keyed by its subscription rather than by folder: the
    // same "<account>#rss#<subscription>" id `rss.recent` mints for the feed row
    // and every `mail.*` method routes on. Folder plays no part.
    if (accountIdIsRss(accountId)) {
        return "$accountId#rss#$threadKey"
    }
    val coreFolder = if (folder.equals(CORE_INBOX_FOLDER, ignoreCase = true)) CORE_INBOX_FOLDER else folder
    threadKey.removePrefix("uid:").takeIf { threadKey.startsWith("uid:") }?.let { uid ->
        return "$accountId#$coreFolder#$uid"
    }
    val encoded = Base64.UrlSafe.encode(threadKey.encodeToByteArray()).trimEnd('=')
    return "$accountId#$coreFolder#t.$encoded"
}

/** The account, folder and thread key a composite `thread_id` was built from. */
data class ParsedThreadId(
    val accountId: String,
    val folder: String,
    val threadKey: String,
)

/**
 * The inverse of [notificationThreadId]: split a stored `thread_id` back into
 * the fields a thread-open needs.
 *
 * A Tasks entry keeps only the composite id — that is all the core hands the
 * clients — but reopening the thread on mobile needs the folder to load and the
 * key to match against, so the id has to come apart again.
 *
 * Returns null for anything that isn't the three-part shape, including a feed
 * id, whose middle segment is the literal `rss` rather than a folder.
 */
@OptIn(ExperimentalEncodingApi::class)
fun parseNotificationThreadId(threadId: String): ParsedThreadId? {
    val separator = threadId.lastIndexOf('#')
    if (separator <= 0) return null
    val head = threadId.substring(0, separator)
    val key = threadId.substring(separator + 1)
    val folderSeparator = head.lastIndexOf('#')
    if (folderSeparator <= 0) return null
    val accountId = head.substring(0, folderSeparator)
    val folder = head.substring(folderSeparator + 1)
    if (accountId.isBlank() || folder.isBlank() || key.isBlank()) return null
    val threadKey =
        if (key.startsWith("t.")) {
            val padded = key.substring(2).padEnd((key.length - 2 + 3) / 4 * 4, '=')
            runCatching { Base64.UrlSafe.decode(padded).decodeToString() }.getOrNull() ?: return null
        } else {
            "uid:$key"
        }
    return ParsedThreadId(accountId, folder, threadKey)
}
