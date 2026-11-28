package jp.nonbili.meron.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.mayQuoteOriginal

/**
 * A passage the user selected in a message, quoted for the reply written under
 * it ("Quote in reply"): `> `-prefixed lines, as the whole original is quoted.
 * Blank lines around the passage are dropped; empty when nothing but whitespace
 * was selected.
 */
internal fun quotedSelectionText(selection: String): String {
    val lines =
        selection
            .replace("\r\n", "\n")
            .replace('\r', '\n')
            .replace(' ', ' ')
            .split('\n')
            .map { it.trimEnd() }
            .dropWhile { it.isEmpty() }
            .dropLastWhile { it.isEmpty() }
    return lines.joinToString("\n") { if (it.isEmpty()) ">" else "> $it" }
}

/** Quotes text selected in a message into a reply to it. */
internal fun interface SelectionQuoter {
    fun quote(
        message: MessageBody,
        selection: String,
        fromReader: Boolean,
    )
}

/** The open conversation's [SelectionQuoter]; null where nothing can be replied to. */
internal val LocalSelectionQuoter = staticCompositionLocalOf<SelectionQuoter?> { null }

/** Whether the bodies below sit in the full-screen reader rather than the conversation. */
internal val LocalInMessageReader = staticCompositionLocalOf { false }

/** What a message body's text selection offers as "Quote in reply", for the
 *  message it belongs to; null where it offers nothing. */
internal val LocalQuoteSelection = compositionLocalOf<((String) -> Unit)?> { null }

/** The reply bar's latest request to take focus, with the caret at [caret];
 *  a new [sequence] for each, zero for none yet. Null for the bar of a
 *  conversation page that isn't the open one. */
internal data class QuickReplyFocusRequest(
    val sequence: Int,
    val caret: Int,
)

internal val LocalQuickReplyFocusRequest = compositionLocalOf<QuickReplyFocusRequest?> { null }

/** Lets the bodies in [content] quote their selected text into a reply to
 *  [message], where it is one that can be answered and quoted. */
@Composable
internal fun ProvideQuoteSelection(
    message: MessageBody,
    isRss: Boolean,
    content: @Composable () -> Unit,
) {
    val quoter = LocalSelectionQuoter.current
    val fromReader = LocalInMessageReader.current
    val quotable =
        quoter != null &&
            !isRss &&
            !message.isDraft() &&
            message.mayQuoteOriginal()
    val onQuote =
        remember(quoter, message, fromReader, quotable) {
            if (quotable) { selection: String -> quoter.quote(message, selection, fromReader) } else null
        }
    CompositionLocalProvider(LocalQuoteSelection provides onQuote, content = content)
}

private fun MessageBody.isDraft(): Boolean = folderRole == "drafts" || folderIsDrafts(folderId)

/** A [androidx.compose.foundation.text.selection.SelectionContainer] whose text
 *  selection menu also offers "Quote in reply" when [onQuote] is set, handing it
 *  the selected text. Only Android adds the item. */
@Composable
internal expect fun QuotableSelectionContainer(
    onQuote: ((String) -> Unit)?,
    content: @Composable () -> Unit,
)
