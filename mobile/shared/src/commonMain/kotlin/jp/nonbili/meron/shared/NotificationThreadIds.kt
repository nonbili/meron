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

/** True when a thread key survives being moved between folders.
 *
 *  Header-derived keys are stable, so undoing an archive can address the thread
 *  in its new folder. A uid-derived key cannot: IMAP assigns fresh uids in the
 *  target mailbox, so the old uid names nothing there. */
fun notificationThreadKeyIsStableAcrossMove(threadKey: String): Boolean = threadKey.isNotBlank() && !threadKey.startsWith("uid:")
