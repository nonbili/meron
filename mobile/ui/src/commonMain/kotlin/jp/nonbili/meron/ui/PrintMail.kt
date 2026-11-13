package jp.nonbili.meron.ui

import androidx.compose.runtime.Composable
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.ThreadReadPage
import jp.nonbili.meron.shared.applyRemoteContentPolicy
import jp.nonbili.meron.shared.mailBodyCsp
import jp.nonbili.meron.shared.standaloneAttachments

@Composable
internal expect fun rememberMailPrinter(errorText: String): (String, String) -> Unit

internal expect val supportsHtmlMailPrinting: Boolean

// HTML bodies arrive sanitized by the core. Printing never enables email scripts.
internal fun mailPrintHtml(
    message: MessageBody,
    fromLabel: String,
    toLabel: String,
    ccLabel: String,
    bccLabel: String,
    replyToLabel: String,
    attachmentsLabel: String,
    noSubject: String,
    preferHtml: Boolean = true,
    allowRemote: Boolean = false,
): String = printHtmlSections(listOf(mailPrintSection(message, fromLabel, toLabel, ccLabel, bccLabel, replyToLabel, attachmentsLabel, noSubject, preferHtml, allowRemote)), allowRemote)

private fun escapePrintText(text: String): String = text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")

private fun mailPrintSection(
    message: MessageBody,
    fromLabel: String,
    toLabel: String,
    ccLabel: String,
    bccLabel: String,
    replyToLabel: String,
    attachmentsLabel: String,
    noSubject: String,
    preferHtml: Boolean,
    allowRemote: Boolean,
): String {
    if (!preferHtml || message.bodyHtml.isBlank() || message.bodyMissing) {
        return "<pre>" + escapePrintText(mailPrintText(message, fromLabel, toLabel, ccLabel, bccLabel, replyToLabel, attachmentsLabel, noSubject)) + "</pre>"
    }
    val header = mailPrintText(message.copy(body = " ", bodyHtml = "", attachments = emptyList()), fromLabel, toLabel, ccLabel, bccLabel, replyToLabel, attachmentsLabel, noSubject).trimEnd()
    val body =
        applyRemoteContentPolicy(message.bodyHtml, allowRemote)
            .replace(Regex("<(script|iframe|object|embed)\\b[^>]*>[\\s\\S]*?</\\1\\s*>", RegexOption.IGNORE_CASE), "")
            .replace(Regex("<!doctype[^>]*>|</?(html|head|body)\\b[^>]*>|<meta\\b[^>]*>", RegexOption.IGNORE_CASE), "")
    val attachments = standaloneAttachments(message)
    val footer = if (attachments.isEmpty()) "" else "<pre>" + escapePrintText("$attachmentsLabel:\n" + attachments.joinToString("\n") { it.filename }) + "</pre>"
    return "<pre>${escapePrintText(header)}</pre><div>$body</div>$footer"
}

private fun mailPrintText(
    message: MessageBody,
    fromLabel: String,
    toLabel: String,
    ccLabel: String,
    bccLabel: String,
    replyToLabel: String,
    attachmentsLabel: String,
    noSubject: String,
): String {
    val text =
        buildString {
            appendLine(message.subject.ifBlank { noSubject })
            appendLine("$fromLabel: ${fullFromAddress(message)}")
            if (message.to.isNotBlank()) appendLine("$toLabel: ${message.to}")
            if (message.cc.isNotBlank()) appendLine("$ccLabel: ${message.cc}")
            if (message.bcc.isNotBlank()) appendLine("$bccLabel: ${message.bcc}")
            if (message.replyTo.isNotBlank()) appendLine("$replyToLabel: ${message.replyTo}")
            if (message.dateEpochSeconds != 0L) appendLine(formatMessageFullTimestamp(message.dateEpochSeconds))
            appendLine()
            append(printMessagePlainText(message))
            val attachments = message.attachments
            if (attachments.isNotEmpty()) {
                appendLine()
                appendLine()
                appendLine("$attachmentsLabel:")
                attachments.forEach { appendLine(it.filename) }
            }
        }
    return text
}

internal fun printMessagePlainText(message: MessageBody): String =
    messagePlainText(
        message.copy(
            bodyHtml =
                message.bodyHtml
                    .replace(Regex("<!--[\\s\\S]*?-->"), "")
                    .replace(Regex("<(script|style)\\b[^>]*>[\\s\\S]*?</\\1\\s*>", RegexOption.IGNORE_CASE), ""),
        ),
    )

private fun printHtmlSections(
    messages: List<String>,
    allowRemote: Boolean,
): String {
    val sections = messages.joinToString("\n") { "<section>$it</section>" }
    val csp = mailBodyCsp(allowRemote, "print-disabled").replace("script-src 'nonce-print-disabled'", "script-src 'none'")
    return """<!doctype html><html><head><meta charset="utf-8">
        <meta http-equiv="Content-Security-Policy" content="$csp">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <style>body { color: black; background: white; }</style>
        </head><body>$sections
        <style>@page { margin: 15mm; }
        html, body { height: auto !important; overflow: visible !important; }
        body > section + section { break-before: page; page-break-before: always; }
        img, table { max-width: 100% !important; } img { height: auto !important; }
        section > pre { white-space: pre-wrap; overflow-wrap: anywhere; font: 11pt/1.5 sans-serif; color: black; }
        * { -webkit-print-color-adjust: exact; print-color-adjust: exact; }</style></body></html>"""
}

@Composable
internal fun rememberPrintMessage(
    preferHtml: Boolean = true,
    allowRemote: Boolean = false,
): (MessageBody) -> Unit {
    val print = rememberMailPrinter(tr("chat.couldNotPrintMessage"))
    val from = tr("composer.fields.from")
    val to = tr("composer.fields.to")
    val cc = tr("composer.fields.cc")
    val bcc = tr("composer.fields.bcc")
    val replyTo = tr("chat.replyTo")
    val attachments = tr("chat.attachments")
    val noSubject = tr("threads.noSubject")
    return { message ->
        print(message.subject.ifBlank { noSubject }, mailPrintHtml(message, from, to, cc, bcc, replyTo, attachments, noSubject, preferHtml && supportsHtmlMailPrinting, allowRemote))
    }
}

internal suspend fun loadPrintThread(fetchPage: suspend (String?) -> ThreadReadPage): List<MessageBody> {
    val messages = linkedMapOf<String, MessageBody>()
    val cursors = mutableSetOf<String?>()
    var cursor: String? = null
    do {
        check(cursors.add(cursor)) { "Repeated thread cursor" }
        val page = fetchPage(cursor)
        page.messages.forEach { message ->
            if (message.id !in messages) messages[message.id] = message
        }
        cursor = page.nextCursor.takeIf { it.isNotBlank() }
    } while (cursor != null)
    check(messages.isNotEmpty()) { "Thread is empty" }
    return messages.values.sortedBy { it.dateEpochSeconds }
}

@Composable
internal fun rememberPrintThread(preferHtml: Boolean): (String, List<MessageBody>) -> Unit {
    val print = rememberMailPrinter(tr("chat.couldNotPrintThread"))
    val unavailableBody = tr("chat.couldNotPrintMessage")
    val from = tr("composer.fields.from")
    val to = tr("composer.fields.to")
    val cc = tr("composer.fields.cc")
    val bcc = tr("composer.fields.bcc")
    val replyTo = tr("chat.replyTo")
    val attachments = tr("chat.attachments")
    val noSubject = tr("threads.noSubject")
    return { subject, messages ->
        print(
            subject.ifBlank { noSubject },
            printHtmlSections(
                messages.map {
                    mailPrintSection(if (it.bodyMissing) it.copy(body = unavailableBody) else it, from, to, cc, bcc, replyTo, attachments, noSubject, preferHtml && supportsHtmlMailPrinting, false)
                },
                false,
            ),
        )
    }
}
