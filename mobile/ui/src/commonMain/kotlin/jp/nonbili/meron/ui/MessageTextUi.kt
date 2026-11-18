package jp.nonbili.meron.ui

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Link
import androidx.compose.material.icons.filled.OpenInBrowser
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.AwaitPointerEventScope
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.PointerInputChange
import androidx.compose.ui.input.pointer.PointerInputScope
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.DpOffset

internal data class InlineMessageLink(
    val start: Int,
    val end: Int,
    val url: String,
)

internal data class ParsedInlineMessageText(
    val text: String,
    val links: List<InlineMessageLink>,
)

private val inlineMessageToken =
    Regex("""(\[[^\]]+]\([^)]+\)|(?:https?://|www\.)[^\s<>"']+)""", RegexOption.IGNORE_CASE)
private val markdownMessageLink = Regex("""^\[([^\]]+)]\(([^)]+)\)$""")
private val schemelessMessageUrl = Regex("""^[\w.-]+\.[a-z]{2,}(/|$)""", RegexOption.IGNORE_CASE)

internal fun parseInlineMessageText(text: String): ParsedInlineMessageText {
    if (text.isEmpty()) return ParsedInlineMessageText("", emptyList())

    val rendered = StringBuilder()
    val links = mutableListOf<InlineMessageLink>()
    var sourceOffset = 0
    inlineMessageToken.findAll(text).forEach { match ->
        rendered.append(text, sourceOffset, match.range.first)
        val token = match.value
        val markdown = markdownMessageLink.matchEntire(token)
        val label = markdown?.groupValues?.get(1) ?: token
        val rawUrl = markdown?.groupValues?.get(2) ?: token
        val url = normalizedInlineMessageUrl(rawUrl)
        if (url == null) {
            rendered.append(token)
            sourceOffset = match.range.last + 1
            return@forEach
        }
        val start = rendered.length
        rendered.append(label)
        links += InlineMessageLink(start, rendered.length, url)
        sourceOffset = match.range.last + 1
    }
    rendered.append(text, sourceOffset, text.length)
    return ParsedInlineMessageText(rendered.toString(), links)
}

private fun normalizedInlineMessageUrl(rawUrl: String): String? =
    when {
        rawUrl.startsWith("https://", ignoreCase = true) ||
            rawUrl.startsWith("http://", ignoreCase = true) ||
            rawUrl.startsWith("mailto:", ignoreCase = true) ||
            rawUrl.startsWith("tel:", ignoreCase = true) -> rawUrl

        schemelessMessageUrl.containsMatchIn(rawUrl) -> "https://$rawUrl"

        else -> null
    }

@Composable
internal fun SelectableMessageText(
    text: String,
    onOpenUrl: (String) -> Unit,
    style: TextStyle,
    color: Color = Color.Unspecified,
    searchQuery: String = "",
    activeSearchMatch: Boolean = false,
) {
    val parsed = parseInlineMessageText(text)
    val linkColor = MaterialTheme.colorScheme.primary
    val annotated =
        buildAnnotatedString {
            append(parsed.text)
            parsed.links.forEach { link ->
                addLink(
                    LinkAnnotation.Url(
                        url = link.url,
                        styles =
                            TextLinkStyles(
                                style =
                                    SpanStyle(
                                        color = linkColor,
                                        textDecoration = TextDecoration.Underline,
                                    ),
                            ),
                        linkInteractionListener = { onOpenUrl(link.url) },
                    ),
                    start = link.start,
                    end = link.end,
                )
            }
            addSearchHighlights(parsed.text, searchQuery, activeSearchMatch)
        }
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    var menuTarget by remember { mutableStateOf<MessageLinkMenuTarget?>(null) }
    val density = LocalDensity.current
    Box {
        SelectionContainer {
            Text(
                text = annotated,
                style = style,
                color = color,
                onTextLayout = { layout = it },
                modifier =
                    Modifier.pointerInput(parsed.links) {
                        detectMessageLinkGestures(
                            linkAt = { position ->
                                layout?.let { messageLinkAtPosition(it, parsed.links, position) }
                            },
                            onTap = { link -> onOpenUrl(link.url) },
                            onLongPress = { link, position ->
                                menuTarget =
                                    MessageLinkMenuTarget(
                                        url = link.url,
                                        offset =
                                            with(density) {
                                                DpOffset(position.x.toDp(), position.y.toDp())
                                            },
                                    )
                            },
                        )
                    },
            )
        }
        MessageLinkContextMenu(
            target = menuTarget,
            onDismiss = { menuTarget = null },
            onOpenUrl = onOpenUrl,
        )
    }
}

/** A long-pressed link, plus where the press landed relative to the composable
 *  showing the menu, so the menu opens at the finger. */
internal data class MessageLinkMenuTarget(
    val url: String,
    val offset: DpOffset,
)

@Composable
internal fun MessageLinkContextMenu(
    target: MessageLinkMenuTarget?,
    onDismiss: () -> Unit,
    onOpenUrl: (String) -> Unit,
) {
    if (target == null) return
    val clipboardManager = LocalClipboardManager.current
    DropdownMenu(expanded = true, onDismissRequest = onDismiss, offset = target.offset) {
        DropdownMenuItem(
            text = { Text(tr("chat.actions.openLink")) },
            leadingIcon = { Icon(Icons.Filled.OpenInBrowser, contentDescription = null) },
            onClick = {
                onDismiss()
                onOpenUrl(target.url)
            },
        )
        DropdownMenuItem(
            text = { Text(tr("chat.actions.copyLinkAddress")) },
            leadingIcon = { Icon(Icons.Filled.Link, contentDescription = null) },
            onClick = {
                onDismiss()
                clipboardManager.setText(AnnotatedString(target.url))
            },
        )
    }
}

internal fun messageLinkAtCharOffset(
    links: List<InlineMessageLink>,
    charOffset: Int,
): InlineMessageLink? = links.firstOrNull { charOffset >= it.start && charOffset < it.end }

/** Hit-tests [position] against the rendered link spans. [TextLayoutResult.getOffsetForPosition]
 *  snaps to the nearest character, so the character box is checked too — otherwise a press in the
 *  blank space past the end of a line would count as a press on its last link. */
internal fun messageLinkAtPosition(
    layout: TextLayoutResult,
    links: List<InlineMessageLink>,
    position: Offset,
): InlineMessageLink? {
    if (links.isEmpty()) return null
    val charOffset = layout.getOffsetForPosition(position)
    val link = messageLinkAtCharOffset(links, charOffset) ?: return null
    val charIndex = charOffset.coerceIn(link.start, link.end - 1)
    val box = layout.getBoundingBox(charIndex)
    val line = layout.getLineForOffset(charIndex)
    val onGlyph =
        position.x >= box.left &&
            position.x <= box.right &&
            position.y >= layout.getLineTop(line) &&
            position.y <= layout.getLineBottom(line)
    return link.takeIf { onGlyph }
}

/** Taps and long presses on inline links, handled ahead of the enclosing
 *  [SelectionContainer] (whose own long press starts text selection) by claiming the
 *  press in the initial pass — but only when it lands on a link, so selecting plain
 *  text and scrolling the bubble keep working. */
internal suspend fun PointerInputScope.detectMessageLinkGestures(
    linkAt: (Offset) -> InlineMessageLink?,
    onTap: (InlineMessageLink) -> Unit,
    onLongPress: (InlineMessageLink, Offset) -> Unit,
) {
    awaitEachGesture {
        val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
        val link = linkAt(down.position) ?: return@awaitEachGesture
        down.consume()
        val released =
            withTimeoutOrNull(viewConfiguration.longPressTimeoutMillis) {
                awaitLinkRelease(down)
            }
        when (released) {
            // Released within the long-press timeout without dragging away.
            true -> {
                onTap(link)
            }

            // Dragged past touch slop: the scrolling parent owns the gesture now.
            false -> {
                Unit
            }

            // Timed out: still pressed on the link.
            null -> {
                onLongPress(link, down.position)
                consumeUntilUp(down)
            }
        }
    }
}

private suspend fun AwaitPointerEventScope.awaitLinkRelease(down: PointerInputChange): Boolean {
    while (true) {
        val event = awaitPointerEvent(PointerEventPass.Initial)
        val change = event.changes.firstOrNull { it.id == down.id } ?: return false
        if (!change.pressed) {
            change.consume()
            return true
        }
        if ((change.position - down.position).getDistance() > viewConfiguration.touchSlop) {
            return false
        }
        change.consume()
    }
}

private suspend fun AwaitPointerEventScope.consumeUntilUp(down: PointerInputChange) {
    while (true) {
        val event = awaitPointerEvent(PointerEventPass.Initial)
        event.changes.forEach { it.consume() }
        val change = event.changes.firstOrNull { it.id == down.id } ?: return
        if (!change.pressed) return
    }
}

private fun AnnotatedString.Builder.addSearchHighlights(
    text: String,
    query: String,
    active: Boolean,
) {
    if (query.isBlank()) return
    val lower = text.lowercase()
    val needle = query.lowercase()
    var start = 0
    while (start < text.length) {
        val index = lower.indexOf(needle, start)
        if (index < 0) return
        addStyle(
            SpanStyle(
                background = if (active) Color(0xFFFFD54F) else Color(0xFFFFECB3),
                color = Color.Black,
                fontWeight = FontWeight.SemiBold,
            ),
            start = index,
            end = index + needle.length,
        )
        start = index + needle.length
    }
}
