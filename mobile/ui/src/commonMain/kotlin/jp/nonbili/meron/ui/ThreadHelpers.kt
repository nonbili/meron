package jp.nonbili.meron.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.folderIsTrash

// One-line header subtitle. The bubbles already name their sender, so the
// header only carries the message count (or the feed's latest date for RSS).
internal fun threadHeaderSubtitle(
    messages: List<MessageBody>,
    isRss: Boolean,
): String {
    if (messages.isEmpty()) return ""
    if (isRss) return formatInboxTimestamp(messages.maxOf { it.dateEpochSeconds })
    val count = messages.size
    return if (count == 1) "" else "$count messages"
}

// How long after open the list keeps re-anchoring to the target message while
// asynchronously measured bubbles (HTML bodies in WebViews) settle.
internal const val THREAD_OPEN_ANCHOR_WINDOW_MS = 2_000L

// A message to hold at the top of the conversation viewport while item sizes
// settle, held by id because list indices shift when an older page loads.
// `animated` distinguishes a move the reader asked for by tapping a message
// open, which reads better as a scroll, from the silent positioning done when a
// thread opens.
internal data class ThreadListAnchor(
    val messageId: String,
    val animated: Boolean,
)

// List index to land on when a thread opens: the first unread message, or the
// newest message when everything is read. The subject header is always item 0,
// followed by the load-older row when there is an older page. Returns null when
// the default top position is already correct.
internal fun threadOpenScrollIndex(
    messages: List<MessageBody>,
    hasLoadOlderRow: Boolean,
): Int? {
    if (messages.isEmpty()) return null
    val firstUnread = messages.indexOfFirst { it.unread }
    val target = if (firstUnread >= 0) firstUnread else messages.lastIndex
    // Nothing to scroll past: staying at the top leaves the full subject on
    // screen above the message. With an older page still to load, the
    // load-older row sits between them, so scroll past both headers instead —
    // landing on the row would auto-load the older page straight away.
    if (target == 0 && !hasLoadOlderRow) return null
    return target + threadHeaderItemCount(hasLoadOlderRow)
}

// List items rendered above the messages: the subject header, plus the
// load-older row when an older page can still be loaded.
internal fun threadHeaderItemCount(hasLoadOlderRow: Boolean): Int = if (hasLoadOlderRow) 2 else 1

// Where the load-older row sits when there is one: directly below the subject
// header. Scrolling far enough to bring it into view is what asks for the older
// page, so the watcher fires on any first-visible index at or above it.
internal const val THREAD_LOAD_OLDER_ITEM_INDEX = 1

// Geometry of one visible LazyColumn item, decoupled from compose types so the
// scroll-driven read marking below is unit-testable.
internal data class ListItemGeometry(
    val index: Int,
    val offset: Int,
    val size: Int,
)

internal data class BottomMessageAnchor(
    val messageIndex: Int,
    val bottomGapPx: Int,
)

/** The lowest message currently visible, plus its distance from the viewport
 * bottom. Captured before the keyboard resizes the conversation so that same
 * message can remain in the same bottom-relative position. */
internal fun bottomVisibleMessageAnchor(
    visible: List<ListItemGeometry>,
    headerItemCount: Int,
    messageCount: Int,
    viewportStartOffset: Int,
    viewportEndOffset: Int,
): BottomMessageAnchor? =
    visible
        .filter { item ->
            item.index in headerItemCount until (headerItemCount + messageCount) &&
                item.offset < viewportEndOffset &&
                item.offset + item.size > viewportStartOffset
        }.maxByOrNull { it.offset + it.size }
        ?.let { item ->
            BottomMessageAnchor(
                messageIndex = item.index - headerItemCount,
                bottomGapPx = (viewportEndOffset - (item.offset + item.size)).coerceAtLeast(0),
            )
        }

internal data class ThreadScrollSnapshot(
    val firstVisibleIndex: Int,
    val visible: List<ListItemGeometry>,
    val totalItemCount: Int,
    val viewportEndOffset: Int,
)

// Ids of messages that just went from read to unread — the reader picking
// "Mark as unread" on a message that is very likely still on screen. Scroll
// marking has to leave those alone until they scroll away, or the action undoes
// itself on the next layout change. A message first seen already unread (no
// entry in `previousUnread`) is not one of these: it is simply an unread
// message the thread arrived with, and reading it marks it read as usual.
// Mirrors collectManualUnreadIds on desktop.
internal fun manualUnreadIds(
    messages: List<MessageBody>,
    previousUnread: Map<String, Boolean>,
): List<String> = messages.filter { it.unread && previousUnread[it.id] == false }.map { it.id }

// Indices (into the message list) of the messages that count as read from where
// the list sits: their top has passed above the viewport, so the reader scrolled
// through them — the only way a message taller than the screen ever counts — or
// their bottom came into view, so all of them has been on screen. A bubble
// merely peeking in from the bottom is the next message waiting its turn, not
// one that was read. Mirrors isMessageRead on desktop.
//
// `headerItemCount` counts the list items above the messages (see
// threadHeaderItemCount). `topSlackPx` is the grace on the top edge: unlike desktop, which lands
// its open anchor 24px below the edge, mobile scrolls the anchored item flush
// to it, so a pixel of rounding must not read as "scrolled past".
internal fun readMessageIndices(
    visible: List<ListItemGeometry>,
    firstVisibleIndex: Int,
    headerItemCount: Int,
    messageCount: Int,
    topSlackPx: Int,
    viewportEndOffset: Int,
): List<Int> {
    val read = mutableListOf<Int>()
    for (messageIndex in 0 until messageCount) {
        val itemIndex = messageIndex + headerItemCount
        val geometry = visible.firstOrNull { it.index == itemIndex }
        val isRead =
            if (geometry != null) {
                geometry.offset < -topSlackPx || geometry.offset + geometry.size <= viewportEndOffset
            } else {
                // Not in the visible window: above it means read, below it means
                // the reader has not reached it yet.
                itemIndex < firstVisibleIndex
            }
        if (isRead) read += messageIndex
    }
    return read
}

// Index (into the message list) of the message whose header the conversation
// should pin at the top of the viewport: the expanded message the reader is in
// the middle of, i.e. the one whose card straddles the top edge. Reading a
// message taller than the screen otherwise hides the header that collapses it.
// `minRemainingPx` keeps the pin off a card that is all but scrolled away —
// pinning a header over the message below it would only mislead.
internal fun pinnedHeaderMessageIndex(
    visible: List<ListItemGeometry>,
    headerItemCount: Int,
    messageCount: Int,
    viewportStartOffset: Int,
    minRemainingPx: Int,
    expanded: (Int) -> Boolean,
): Int? {
    for (geometry in visible) {
        val messageIndex = geometry.index - headerItemCount
        if (messageIndex < 0 || messageIndex >= messageCount) continue
        if (geometry.offset >= viewportStartOffset) continue
        if (geometry.offset + geometry.size < viewportStartOffset + minRemainingPx) continue
        return if (expanded(messageIndex)) messageIndex else null
    }
    return null
}

// True when the last list item is visible with its bottom within
// `bottomSlackPx` of the viewport end — desktop's "remaining <= 160" rule for
// marking the whole thread read.
internal fun listViewedToBottom(
    visible: List<ListItemGeometry>,
    totalItemCount: Int,
    viewportEndOffset: Int,
    bottomSlackPx: Int,
): Boolean {
    if (totalItemCount <= 0) return false
    val last = visible.lastOrNull() ?: return false
    if (last.index != totalItemCount - 1) return false
    return last.offset + last.size <= viewportEndOffset + bottomSlackPx
}

// Apply a partial scroll-driven read locally. Thread summaries carry the
// authoritative unread-message count, including messages on older pages that
// are not loaded in the conversation, so derive the remaining thread state
// from that count rather than only from the currently rendered messages.
internal fun threadAfterMessagesRead(
    thread: ThreadSummary,
    readCount: Int,
): ThreadSummary {
    if (!thread.unread || readCount <= 0) return thread
    val remaining = (thread.unreadCount.coerceAtLeast(1) - readCount).coerceAtLeast(0)
    return thread.copy(unread = remaining > 0, unreadCount = remaining)
}

internal fun threadMessageSearchText(message: MessageBody): String =
    listOf(
        message.subject,
        message.from,
        message.fromAddr,
        message.to,
        message.cc,
        message.body,
        message.bodyHtml.replace(Regex("<[^>]+>"), " "),
        message.attachments.joinToString(" ") { it.filename },
    ).joinToString(" ").lowercase()

internal fun threadDeleteActionLabel(
    folder: String,
    folderRole: String = "folder",
): String =
    when {
        folderRole == "drafts" || (folderRole == "folder" && folderIsDrafts(folder)) -> "Discard draft"
        folderRole == "trash" || (folderRole == "folder" && folderIsTrash(folder)) -> "Delete forever"
        else -> "Move to Trash"
    }

internal fun messagePlainText(message: MessageBody): String =
    message.body
        .ifBlank {
            message.bodyHtml
                .replace(Regex("<br\\s*/?>", RegexOption.IGNORE_CASE), "\n")
                .replace(Regex("</p>", RegexOption.IGNORE_CASE), "\n")
                .replace(Regex("<[^>]+>"), " ")
                .replace(Regex("[ \\t]+"), " ")
                .trim()
        }.ifBlank { "(no content)" }

internal fun conversationParticipants(
    messages: List<MessageBody>,
    ownEmail: String,
    isRss: Boolean,
): List<ConversationParticipant> {
    if (isRss) return emptyList()

    data class MutableParticipant(
        var name: String,
        val email: String,
        var count: Int,
        val isSelf: Boolean,
    )
    val own = ownEmail.trim().lowercase()
    val byEmail = linkedMapOf<String, MutableParticipant>()

    fun add(
        name: String,
        email: String,
    ) {
        val normalized = email.trim().trim('<', '>', ',', ';').lowercase()
        if (normalized.isBlank() || !normalized.contains("@")) return
        val existing = byEmail[normalized]
        if (existing != null) {
            existing.count += 1
            if ((existing.name.isBlank() || existing.name == existing.email) && name.isNotBlank() && name != email) {
                existing.name = name
            }
        } else {
            byEmail[normalized] =
                MutableParticipant(
                    name = name.takeIf { it.isNotBlank() && it != email } ?: normalized,
                    email = normalized,
                    count = 1,
                    isSelf = normalized == own,
                )
        }
    }

    messages.forEach { message ->
        add(message.from, message.fromAddr.ifBlank { message.from })
        parseAddressList(message.to).forEach { (name, email) -> add(name, email) }
        parseAddressList(message.cc).forEach { (name, email) -> add(name, email) }
    }
    return byEmail.values
        .map { ConversationParticipant(it.name, it.email, it.count, it.isSelf) }
        .sortedWith(compareBy<ConversationParticipant> { it.isSelf }.thenByDescending { it.count })
}

// One entry of an address header: the name a chip shows, the address shown
// under it (blank when the name already is the address), and the full
// `Name <addr>` text tapping it copies.
internal data class AddressChipItem(
    val display: String,
    val email: String,
    val full: String,
)

internal fun addressChipItems(rawList: String): List<AddressChipItem> =
    rawList.split(',', ';').mapNotNull { raw ->
        val entry = raw.trim()
        if (entry.isBlank()) return@mapNotNull null
        val bracket = Regex("""^(.*)<([^>]+)>$""").matchEntire(entry)
        if (bracket == null) return@mapNotNull AddressChipItem(entry, "", entry)
        val address = bracket.groupValues[2].trim()
        val name = bracket.groupValues[1].trim().trim('"')
        if (name.isBlank() || name.equals(address, ignoreCase = true)) {
            AddressChipItem(address, "", entry)
        } else {
            AddressChipItem(name, address, entry)
        }
    }

// The sender as a single address header, so it renders like To/Cc do.
internal fun fullFromAddress(message: MessageBody): String {
    val name = message.from.trim()
    val addr = message.fromAddr.trim()
    return when {
        addr.isBlank() -> name
        name.isBlank() || name.equals(addr, ignoreCase = true) -> addr
        name.contains('<') -> name
        else -> "$name <$addr>"
    }
}

internal fun parseAddressList(value: String): List<Pair<String, String>> {
    if (value.isBlank()) return emptyList()
    return value.split(',', ';').mapNotNull { raw ->
        val entry = raw.trim()
        if (entry.isBlank()) return@mapNotNull null
        val bracket = Regex("""^(.*)<([^>]+)>$""").matchEntire(entry)
        if (bracket != null) {
            val name = bracket.groupValues[1].trim().trim('"')
            val email = bracket.groupValues[2].trim()
            return@mapNotNull name to email
        }
        entry to entry
    }
}

internal fun highlightedMessageText(
    text: String,
    query: String,
    active: Boolean,
): AnnotatedString {
    if (query.isBlank()) return AnnotatedString(text)
    val lower = text.lowercase()
    val needle = query.lowercase()
    return buildAnnotatedString {
        var start = 0
        while (start < text.length) {
            val index = lower.indexOf(needle, start)
            if (index < 0) {
                append(text.substring(start))
                break
            }
            append(text.substring(start, index))
            withStyle(
                SpanStyle(
                    background = if (active) Color(0xFFFFD54F) else Color(0xFFFFECB3),
                    color = Color.Black,
                    fontWeight = FontWeight.SemiBold,
                ),
            ) {
                append(text.substring(index, index + needle.length))
            }
            start = index + needle.length
        }
    }
}
