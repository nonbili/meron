package jp.nonbili.meron.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.BodyLink

// Folding a message's quoted tail — the conversation a reply pastes under its
// new text — behind a "•••" toggle, the way Gmail does (and the desktop app,
// see desktop/frontend/src/components/chat/quoteFold.ts). The core finds the
// quote: HTML bodies arrive with it marked by `data-meron-quote`, plain bodies
// with the UTF-16 offset it starts at (`body_quote_start`), which is also how
// Kotlin indexes strings.

/** A plain body split at the core's quote offset; [quote] is empty when there
 *  is nothing to fold. */
internal data class QuotedBody(
    val reply: String,
    val quote: String,
    val replyLinks: List<BodyLink> = emptyList(),
    val quoteLinks: List<BodyLink> = emptyList(),
)

/** [body] split at the core's quote offset, each part with the [links] inside
 *  it, moved to offsets into that part. */
internal fun splitQuotedBody(
    body: String,
    quoteStart: Int?,
    links: List<BodyLink> = emptyList(),
): QuotedBody {
    fun slice(
        from: Int,
        text: String,
    ) = links
        .filter { it.start >= from && it.end <= from + text.length }
        .map { it.copy(start = it.start - from, end = it.end - from) }
    return if (quoteStart == null || quoteStart <= 0 || quoteStart >= body.length) {
        QuotedBody(body, "", slice(0, body))
    } else {
        val reply = body.substring(0, quoteStart).trimEnd()
        val quote = body.substring(quoteStart).trimEnd()
        QuotedBody(reply, quote, slice(0, reply), slice(quoteStart, quote))
    }
}

/** Which messages' quotes the reader opened this session, so a bubble that
 *  scrolls out of the list and back — or whose web view is rebuilt — comes back
 *  the way it was left. Touched from the main thread only. */
internal object QuoteFoldMemory {
    private val open = mutableSetOf<String>()

    fun isOpen(key: String): Boolean = key in open

    fun setOpen(
        key: String,
        value: Boolean,
    ) {
        if (value) open.add(key) else open.remove(key)
    }
}

/** Whether an in-thread search should open the quote: it matches inside it,
 *  by the same case-insensitive rule the highlighter marks with. */
internal fun quoteMatchesSearch(
    quote: String,
    searchQuery: String,
): Boolean {
    val query = searchQuery.trim()
    return quote.isNotEmpty() && query.isNotEmpty() && quote.lowercase().contains(query.lowercase())
}

/** The attribute the core marks an HTML body's quoted tail with. */
internal const val HTML_QUOTE_ATTR = "data-meron-quote"

/** A JavaScript string literal for [value], safe to splice into an inline
 *  script: `<` is escaped so the text can never close the script element. */
internal fun jsStringLiteral(value: String): String =
    buildString {
        append('"')
        value.forEach { char ->
            when (char) {
                '\\' -> append("\\\\")
                '"' -> append("\\\"")
                '\n' -> append("\\n")
                '\r' -> append("\\r")
                '<' -> append("\\u003c")
                '\u2028' -> append("\\u2028")
                '\u2029' -> append("\\u2029")
                else -> append(char)
            }
        }
        append('"')
    }

/** The "•••" chip between a plain body's reply and its quoted tail. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun QuoteToggle(
    open: Boolean,
    color: Color,
    onToggle: () -> Unit,
    onLongClick: (() -> Unit)? = null,
) {
    val label = if (open) tr("chat.hideQuotedText") else tr("chat.showQuotedText")
    val shape = RoundedCornerShape(50)
    Box(
        Modifier
            .padding(vertical = 6.dp)
            .clip(shape)
            .border(1.dp, color.copy(alpha = if (open) 0.55f else 0.25f), shape)
            .background(color.copy(alpha = if (open) 0.16f else 0.06f))
            .then(
                if (onLongClick == null) {
                    Modifier.clickable(onClickLabel = label, onClick = onToggle)
                } else {
                    Modifier.combinedClickable(onClickLabel = label, onClick = onToggle, onLongClick = onLongClick)
                },
            ).semantics { contentDescription = label }
            .padding(horizontal = 10.dp, vertical = 2.dp),
    ) {
        Text(
            text = "•••",
            color = color.copy(alpha = if (open) 0.95f else 0.65f),
            fontSize = 11.sp,
            lineHeight = 12.sp,
            fontWeight = FontWeight.Bold,
            letterSpacing = 1.sp,
        )
    }
}
