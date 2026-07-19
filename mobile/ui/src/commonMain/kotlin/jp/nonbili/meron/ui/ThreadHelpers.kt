package jp.nonbili.meron.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.folderIsTrash

// One-line header subtitle: who's in the conversation plus message count, so the
// header carries context the way a mail client's thread view does.
internal fun threadHeaderSubtitle(
    messages: List<MessageBody>,
    ownEmail: String,
    isRss: Boolean,
): String {
    if (messages.isEmpty()) return ""
    val count = messages.size
    val countLabel = if (count == 1) "1 message" else "$count messages"
    if (isRss) return formatInboxTimestamp(messages.maxOf { it.dateEpochSeconds })
    val others =
        messages
            .filterNot { isOutgoing(it, ownEmail) }
            .map { it.from.ifBlank { it.fromAddr } }
            .filter { it.isNotBlank() }
            .distinct()
    val people = others.take(2).joinToString(", ")
    return when {
        people.isBlank() -> countLabel
        count == 1 -> people
        else -> "$people · $countLabel"
    }
}

// How long after open the list keeps re-anchoring to the target message while
// asynchronously measured bubbles (HTML bodies in WebViews) settle.
internal const val THREAD_OPEN_ANCHOR_WINDOW_MS = 2_000L

// List index to land on when a thread opens: the first unread message, or the
// newest message when everything is read. `headerItemCount` counts the list
// items rendered above the messages (the load-older row). Returns null when
// the default top position is already correct.
internal fun threadOpenScrollIndex(
    messages: List<MessageBody>,
    headerItemCount: Int,
): Int? {
    if (messages.isEmpty()) return null
    val firstUnread = messages.indexOfFirst { it.unread }
    val target = if (firstUnread >= 0) firstUnread else messages.lastIndex
    return (target + headerItemCount).takeIf { it > 0 }
}

// Geometry of one visible LazyColumn item, decoupled from compose types so the
// scroll-driven read marking below is unit-testable.
internal data class ListItemGeometry(
    val index: Int,
    val offset: Int,
    val size: Int,
)

internal data class ThreadScrollSnapshot(
    val firstVisibleIndex: Int,
    val visible: List<ListItemGeometry>,
    val totalItemCount: Int,
    val viewportEndOffset: Int,
)

// Indices (into the message list) of the messages whose bubbles have fully
// scrolled past the top of the viewport. `headerItemCount` counts the list
// items above the messages (the load-older row); `topSlackPx` mirrors
// desktop's 24px grace so a bubble only counts once it is clearly out of view.
internal fun scrolledPastMessageIndices(
    visible: List<ListItemGeometry>,
    firstVisibleIndex: Int,
    headerItemCount: Int,
    messageCount: Int,
    topSlackPx: Int,
): List<Int> {
    val passed = mutableListOf<Int>()
    for (messageIndex in 0 until messageCount) {
        val itemIndex = messageIndex + headerItemCount
        val geometry = visible.firstOrNull { it.index == itemIndex }
        val isPast =
            if (geometry != null) {
                geometry.offset + geometry.size < topSlackPx
            } else {
                itemIndex < firstVisibleIndex
            }
        if (!isPast) break
        passed += messageIndex
    }
    return passed
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
