package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.DpOffset

internal data class BareUrl(
    val start: Int,
    val end: Int,
    val text: String,
    val href: String,
)

// The same rules as desktop's lib/linkify.ts and the core's reader-side pass:
// `http(s)://` and `www.` only, never a bare domain.
private val bareUrl = Regex("""(^|[^\p{L}\p{N}./@_-])((?:https?://|www\.)[^\s<>"'`]+)""", RegexOption.IGNORE_CASE)
private val closers = mapOf(')' to '(', ']' to '[', '}' to '{')
private val scheme = Regex("""^(https?://|www\.)""", RegexOption.IGNORE_CASE)
private val wwwHost = Regex("""^[^./?#]+\.[^/?#]""")

/** The URL without the sentence punctuation or unmatched closing bracket after it. */
private fun trimUrl(url: String): String {
    val balance = mutableMapOf('(' to 0, '[' to 0, '{' to 0)
    for (c in url) {
        if (c in balance) {
            balance[c] = balance.getValue(c) + 1
        } else {
            closers[c]?.let { balance[it] = balance.getValue(it) - 1 }
        }
    }
    var end = url.length
    while (end > 0) {
        val last = url[end - 1]
        val opener = closers[last]
        if (last in ".,;:!?*") {
            end--
        } else if (opener != null && balance.getValue(opener) < 0) {
            balance[opener] = balance.getValue(opener) + 1
            end--
        } else {
            break
        }
    }
    return url.substring(0, end)
}

/** The bare URLs in a run of plain text, in order. */
internal fun findBareUrls(text: String): List<BareUrl> =
    bareUrl
        .findAll(text)
        .mapNotNull { match ->
            val start = match.range.first + match.groupValues[1].length
            val url = trimUrl(match.groupValues[2])
            val www = url.startsWith("www.", ignoreCase = true)
            val host = url.replace(scheme, "")
            if (host.isEmpty() || (www && !wwwHost.containsMatchIn(host))) {
                null
            } else {
                BareUrl(start, start + url.length, url, if (www) "https://$url" else url)
            }
        }.toList()

/**
 * Plain text with its bare URLs as links: a tap opens one, a long press offers
 * to open or copy it, as in a message. A press elsewhere in the text falls
 * through to whatever the text sits in.
 */
@Composable
internal fun LinkifiedText(
    text: String,
    style: TextStyle,
    modifier: Modifier = Modifier,
    color: Color = Color.Unspecified,
    minLines: Int = 1,
    maxLines: Int = Int.MAX_VALUE,
    overflow: TextOverflow = TextOverflow.Clip,
) {
    val uriHandler = LocalUriHandler.current
    val onOpenUrl: (String) -> Unit = { url -> runCatching { uriHandler.openUri(url) } }
    val linkColor = MaterialTheme.colorScheme.primary
    val links = remember(text) { findBareUrls(text).map { InlineMessageLink(it.start, it.end, it.href) } }
    val annotated =
        remember(text, links, linkColor) {
            buildAnnotatedString {
                append(text)
                links.forEach { link ->
                    addLink(
                        LinkAnnotation.Url(
                            url = link.url,
                            styles = TextLinkStyles(style = SpanStyle(color = linkColor, textDecoration = TextDecoration.Underline)),
                            linkInteractionListener = { onOpenUrl(link.url) },
                        ),
                        start = link.start,
                        end = link.end,
                    )
                }
            }
        }
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    var menuTarget by remember { mutableStateOf<MessageLinkMenuTarget?>(null) }
    val density = LocalDensity.current
    Box(modifier) {
        Text(
            text = annotated,
            style = style,
            color = color,
            minLines = minLines,
            maxLines = maxLines,
            overflow = overflow,
            onTextLayout = { layout = it },
            modifier =
                Modifier.pointerInput(links) {
                    detectMessageLinkGestures(
                        linkAt = { position -> layout?.let { messageLinkAtPosition(it, links, position) } },
                        onTap = { link -> onOpenUrl(link.url) },
                        onLongPress = { link, position ->
                            menuTarget =
                                MessageLinkMenuTarget(
                                    url = link.url,
                                    offset = with(density) { DpOffset(position.x.toDp(), position.y.toDp()) },
                                )
                        },
                    )
                },
        )
        MessageLinkContextMenu(target = menuTarget, onDismiss = { menuTarget = null }, onOpenUrl = onOpenUrl)
    }
}
