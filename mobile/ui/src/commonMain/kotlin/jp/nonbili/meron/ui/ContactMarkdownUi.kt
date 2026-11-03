package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

internal enum class ContactMarkdownKind { Paragraph, Heading, ListItem, Quote, Code }

internal enum class ContactMarkdownStyle { Bold, Emphasis, Code }

internal data class ContactMarkdownSpan(
    val start: Int,
    val end: Int,
    val style: ContactMarkdownStyle? = null,
    val url: String? = null,
)

internal data class ContactMarkdownInline(
    val text: String,
    val spans: List<ContactMarkdownSpan> = emptyList(),
)

internal data class ContactMarkdownBlock(
    val kind: ContactMarkdownKind,
    val content: ContactMarkdownInline,
    val level: Int = 0,
    val marker: String = "",
)

/** Deliberately small Markdown subset: no HTML, automatic links, or image loading. */
@Composable
internal fun ContactMarkdown(text: String) {
    val blocks = remember(text) { parseContactMarkdown(text) }
    val uriHandler = LocalUriHandler.current
    val colors = MaterialTheme.colorScheme
    SelectionContainer {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            blocks.forEach { block ->
                val annotated =
                    remember(block.content, colors.primary, colors.surfaceContainerHigh) {
                        buildContactMarkdownAnnotatedString(
                            inline = block.content,
                            codeBackground = colors.surfaceContainerHigh,
                            linkColor = colors.primary,
                            onOpenUrl = { url ->
                                // Only an explicit tap can leave the app. Unsupported handlers
                                // should not crash the notes screen.
                                if (safeContactMarkdownUrl(url) != null) {
                                    runCatching { uriHandler.openUri(url) }
                                }
                            },
                        )
                    }
                when (block.kind) {
                    ContactMarkdownKind.Heading -> {
                        val headingStyle =
                            when (block.level) {
                                1 -> MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold, color = colors.onSurface)
                                2 -> MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold, color = colors.onSurface)
                                else -> MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.SemiBold, color = colors.onSurface)
                            }
                        Text(text = annotated, style = headingStyle)
                    }

                    ContactMarkdownKind.Code -> {
                        val shape = RoundedCornerShape(8.dp)
                        Box(
                            Modifier
                                .fillMaxWidth()
                                .clip(shape)
                                .background(colors.surfaceContainerHigh)
                                .border(1.dp, colors.outlineVariant.copy(alpha = 0.5f), shape)
                                .padding(horizontal = 12.dp, vertical = 10.dp),
                        ) {
                            Text(
                                text = annotated,
                                style =
                                    MaterialTheme.typography.bodySmall.copy(
                                        fontFamily = FontFamily.Monospace,
                                        color = colors.onSurface,
                                        lineHeight = 18.sp,
                                    ),
                            )
                        }
                    }

                    ContactMarkdownKind.Quote -> {
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .height(IntrinsicSize.Min),
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            Box(
                                Modifier
                                    .width(3.dp)
                                    .fillMaxHeight()
                                    .clip(RoundedCornerShape(2.dp))
                                    .background(colors.outlineVariant),
                            )
                            Text(
                                text = annotated,
                                style = MaterialTheme.typography.bodyMedium.copy(color = colors.onSurfaceVariant),
                                modifier = Modifier.weight(1f).padding(vertical = 2.dp),
                            )
                        }
                    }

                    ContactMarkdownKind.ListItem -> {
                        Row(
                            Modifier.fillMaxWidth().padding(start = 4.dp),
                            horizontalArrangement = Arrangement.spacedBy(6.dp),
                        ) {
                            Text(
                                text = block.marker.trimEnd(),
                                style = MaterialTheme.typography.bodyMedium.copy(color = colors.onSurfaceVariant),
                            )
                            Text(
                                text = annotated,
                                style = MaterialTheme.typography.bodyMedium.copy(color = colors.onSurface),
                                modifier = Modifier.weight(1f),
                            )
                        }
                    }

                    ContactMarkdownKind.Paragraph -> {
                        Text(
                            text = annotated,
                            style = MaterialTheme.typography.bodyMedium.copy(color = colors.onSurface),
                        )
                    }
                }
            }
        }
    }
}

private fun buildContactMarkdownAnnotatedString(
    inline: ContactMarkdownInline,
    codeBackground: androidx.compose.ui.graphics.Color,
    linkColor: androidx.compose.ui.graphics.Color,
    onOpenUrl: (String) -> Unit,
): AnnotatedString =
    buildAnnotatedString {
        append(inline.text)
        inline.spans.forEach { span ->
            val style =
                when (span.style) {
                    ContactMarkdownStyle.Bold -> {
                        SpanStyle(fontWeight = FontWeight.SemiBold)
                    }

                    ContactMarkdownStyle.Emphasis -> {
                        SpanStyle(fontStyle = FontStyle.Italic)
                    }

                    ContactMarkdownStyle.Code -> {
                        SpanStyle(
                            fontFamily = FontFamily.Monospace,
                            fontSize = 12.sp,
                            background = codeBackground,
                        )
                    }

                    null -> {
                        null
                    }
                }
            if (style != null) addStyle(style, span.start, span.end)
            span.url?.let { url ->
                addLink(
                    LinkAnnotation.Url(
                        url = url,
                        styles =
                            TextLinkStyles(
                                style =
                                    SpanStyle(
                                        color = linkColor,
                                        textDecoration = TextDecoration.Underline,
                                    ),
                            ),
                        linkInteractionListener = { onOpenUrl(url) },
                    ),
                    span.start,
                    span.end,
                )
            }
        }
    }

/** No normalization into a different scheme, HTML entity decoding, or relative URLs. */
internal fun safeContactMarkdownUrl(url: String): String? {
    if (url.any { it.isWhitespace() || it.code < 32 || it.code == 127 || it == '\\' }) return null
    val scheme = url.substringBefore(':', "").lowercase()
    return when (scheme) {
        "http", "https" -> {
            val prefix = "$scheme://"
            if (!url.startsWith(prefix, ignoreCase = true)) return null
            val authority =
                url
                    .substring(prefix.length)
                    .substringBefore('/')
                    .substringBefore('?')
                    .substringBefore('#')
            url.takeIf { authority.isNotEmpty() && '@' !in authority && '<' !in it && '>' !in it }
        }

        "mailto" -> {
            url.takeIf { it.substringAfter(':').isNotEmpty() && '<' !in it && '>' !in it }
        }

        else -> {
            null
        }
    }
}

private val contactHeading = Regex("^(#{1,6})\\s+(.+)$")
private val contactListItem = Regex("^\\s*(?:([-+*])|([0-9]+)[.)])\\s+(.+)$")
private val contactFence = Regex("^\\s{0,3}(`{3,}|~{3,}).*$")

internal fun parseContactMarkdown(text: String): List<ContactMarkdownBlock> {
    val lines = text.replace("\r\n", "\n").replace('\r', '\n').split('\n')
    val blocks = mutableListOf<ContactMarkdownBlock>()
    var index = 0
    while (index < lines.size) {
        val line = lines[index]
        if (line.isBlank()) {
            index++
            continue
        }
        val fence = contactFence.matchEntire(line)?.groupValues?.get(1)
        if (fence != null) {
            val code = mutableListOf<String>()
            index++
            while (index < lines.size) {
                val candidate = lines[index].trim()
                if (candidate.length >= fence.length && candidate.all { it == fence[0] }) break
                code += lines[index++]
            }
            if (index < lines.size) index++
            blocks += ContactMarkdownBlock(ContactMarkdownKind.Code, ContactMarkdownInline(code.joinToString("\n")))
            continue
        }
        val heading = contactHeading.matchEntire(line)
        val item = contactListItem.matchEntire(line)
        when {
            heading != null -> {
                blocks +=
                    ContactMarkdownBlock(
                        ContactMarkdownKind.Heading,
                        parseContactMarkdownInline(heading.groupValues[2]),
                        level = heading.groupValues[1].length,
                    )
            }

            item != null -> {
                blocks +=
                    ContactMarkdownBlock(
                        ContactMarkdownKind.ListItem,
                        parseContactMarkdownInline(item.groupValues[3]),
                        marker = item.groupValues[2].let { if (it.isEmpty()) "• " else "$it. " },
                    )
            }

            line.trimStart().startsWith('>') -> {
                blocks +=
                    ContactMarkdownBlock(
                        ContactMarkdownKind.Quote,
                        parseContactMarkdownInline(line.trimStart().drop(1).removePrefix(" ")),
                    )
            }

            else -> {
                val paragraph = mutableListOf(line)
                while (index + 1 < lines.size) {
                    val next = lines[index + 1]
                    if (next.isBlank() || contactFence.matches(next) || contactHeading.matches(next) ||
                        contactListItem.matches(next) || next.trimStart().startsWith('>')
                    ) {
                        break
                    }
                    paragraph += next
                    index++
                }
                blocks +=
                    ContactMarkdownBlock(
                        ContactMarkdownKind.Paragraph,
                        parseContactMarkdownInline(paragraph.joinToString("\n")),
                    )
            }
        }
        index++
    }
    return blocks
}

internal fun parseContactMarkdownInline(source: String): ContactMarkdownInline = parseContactMarkdownInline(source, allowLinks = true, depth = 0)

private fun parseContactMarkdownInline(
    source: String,
    allowLinks: Boolean,
    depth: Int,
): ContactMarkdownInline {
    if (depth >= 16) return ContactMarkdownInline(source)
    val text = StringBuilder()
    val spans = mutableListOf<ContactMarkdownSpan>()
    var index = 0

    fun appendInline(
        value: ContactMarkdownInline,
        style: ContactMarkdownStyle? = null,
        url: String? = null,
    ) {
        val start = text.length
        text.append(value.text)
        spans += value.spans.map { it.copy(start = it.start + start, end = it.end + start) }
        if (text.length > start && (style != null || url != null)) {
            spans += ContactMarkdownSpan(start, text.length, style, url)
        }
    }
    while (index < source.length) {
        if (source[index] == '\\' && index + 1 < source.length && source[index + 1] in "\\`*_[]!()") {
            text.append(source[index + 1])
            index += 2
            continue
        }
        val image = source.startsWith("![", index)
        if (image || source[index] == '[') {
            val labelStart = index + if (image) 2 else 1
            val labelEnd = source.indexOf("](", labelStart)
            if (labelEnd >= 0) {
                val destinationStart = labelEnd + 2
                var end = destinationStart
                var nesting = 1
                while (end < source.length) {
                    if (source[end] == '(') nesting++
                    if (source[end] == ')') nesting--
                    if (nesting == 0) break
                    end++
                }
                if (nesting == 0) {
                    val label = source.substring(labelStart, labelEnd)
                    if (image) {
                        // Alt text is literal: it cannot contain clickable links or fetch anything.
                        text.append(label)
                    } else {
                        appendInline(
                            parseContactMarkdownInline(label, allowLinks = false, depth = depth + 1),
                            url = if (allowLinks) safeContactMarkdownUrl(source.substring(destinationStart, end)) else null,
                        )
                    }
                    index = end + 1
                    continue
                }
            }
        }
        val delimiter =
            when {
                source[index] == '`' -> "`".repeat(source.substring(index).takeWhile { it == '`' }.length)
                source.startsWith("**", index) -> "**"
                source.startsWith("__", index) -> "__"
                source[index] == '*' -> "*"
                source[index] == '_' && (index == 0 || !source[index - 1].isLetterOrDigit()) -> "_"
                else -> null
            }
        if (delimiter != null) {
            val end = source.indexOf(delimiter, index + delimiter.length)
            if (end > index + delimiter.length) {
                val content = source.substring(index + delimiter.length, end)
                val code = delimiter[0] == '`'
                appendInline(
                    if (code) {
                        ContactMarkdownInline(content)
                    } else {
                        parseContactMarkdownInline(content, allowLinks, depth + 1)
                    },
                    style =
                        when {
                            code -> ContactMarkdownStyle.Code
                            delimiter.length == 2 -> ContactMarkdownStyle.Bold
                            else -> ContactMarkdownStyle.Emphasis
                        },
                )
                index = end + delimiter.length
                continue
            }
        }
        text.append(source[index++])
    }
    return ContactMarkdownInline(text.toString(), spans)
}
