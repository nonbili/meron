package jp.nonbili.meron.shared

// Remote content (images, video, web fonts a mail pulls from the network) is
// gated the same way it is on desktop: an account-wide "load remote images"
// toggle, plus an app-wide allowlist of senders that always load, plus a
// per-message reveal the reader can make while a thread is open.
//
// The core resolves the first two when it reads a message and bakes the answer
// into the body's CSP (`prepare_html`). This file exists because the reader
// still has to decide twice more:
//
//   * that baked `<meta>` sits in the *mail's* head, and the viewer splices the
//     mail into a document of its own (see HtmlMessageBody) — a CSP meta that is
//     not a child of the document's head is ignored, so the viewer has to state
//     the policy in its own head for it to be enforced at all;
//   * a reveal, or a sender allowed while the thread is on screen, changes the
//     answer after the body was baked, and re-reading the thread to get a
//     differently baked body would be a round trip for a decision already made.

/**
 * A sender address as the allowlist stores and compares it: trimmed, unwrapped
 * from any `Name <addr>` form, lowercased. Mirrors `normalize_sender` in the
 * core and `normalizeSenderAddr` on desktop, so all three agree on membership.
 */
fun normalizeSenderAddr(addr: String): String {
    val trimmed = addr.trim()
    val open = trimmed.lastIndexOf('<')
    val close = trimmed.lastIndexOf('>')
    val bare = if (open >= 0 && close > open) trimmed.substring(open + 1, close) else trimmed
    return bare.trim().lowercase()
}

/**
 * The effective remote-content rule for reading a conversation: the account's
 * own toggle plus the app-wide sender allowlist. Mirrors `RemoteImagePolicy` in
 * the core.
 */
data class RemoteContentPolicy(
    /** The account-wide "load remote images" toggle. */
    val accountAllows: Boolean = false,
    /** App-wide allowed senders, normalized by [normalizeSenderAddr]. */
    val allowedSenders: List<String> = emptyList(),
) {
    /** Whether a message from [fromAddr] may load its remote content. */
    fun allows(fromAddr: String): Boolean {
        if (accountAllows) return true
        val sender = normalizeSenderAddr(fromAddr)
        return sender.isNotEmpty() && allowedSenders.contains(sender)
    }
}

/** Add [addr] to (or drop it from) an allowlist, keeping it normalized and
 *  duplicate-free. Returns the list unchanged when there is nothing to do, so a
 *  caller can skip the write. */
fun withRemoteSender(
    senders: List<String>,
    addr: String,
    allowed: Boolean,
): List<String> {
    val address = normalizeSenderAddr(addr)
    if (address.isEmpty()) return senders
    val without = senders.filterNot { it == address }
    return if (allowed) without + address else without
}

/** Normalize and dedupe an allowlist read back from the core: the row is shared
 *  with desktop, and a value written by an older build may hold `Name <addr>`
 *  forms or repeats. */
fun sanitizeRemoteSenders(senders: List<String>): List<String> {
    val out = mutableListOf<String>()
    for (raw in senders) {
        val address = normalizeSenderAddr(raw)
        if (address.isNotEmpty() && address !in out) out.add(address)
    }
    return out
}

// ---- What the policy holds back --------------------------------------------

/**
 * Whether an attachment renders without a remote fetch: a cached `/media/<key>`
 * file, or a `data:` URL. Anything else is pulled from the network when the
 * image grid loads it, so the remote-content policy has to gate it — those
 * images are decoded natively, outside the web view, where no CSP reaches.
 */
fun isInlineMedia(attachment: MessageAttachment): Boolean = attachment.key.trim().isNotEmpty() || attachment.url.trim().startsWith("data:", ignoreCase = true)

/** The attachment images to show beside a body, with the remote ones held back
 *  while remote content is blocked. */
fun visibleImageAttachments(
    images: List<MessageAttachment>,
    allowRemote: Boolean,
): List<MessageAttachment> = if (allowRemote) images else images.filter(::isInlineMedia)

// Remote references the CSP holds back when a message is not trusted: an
// absolute or protocol-relative `src`/`srcset`/`background`/`poster` (a video
// poster is fetched under `img-src`, so it is blocked like any image), and CSS
// `url(...)` references. Inline images are `/media/<key>` or `data:`, so they
// never match.
private val REMOTE_MEDIA_REF =
    Regex(
        """<[^>]+\s(?:src|srcset|background|poster)\s*=\s*["']?\s*(?:https?:)?//|url\(\s*["']?\s*(?:https?:)?//""",
        RegexOption.IGNORE_CASE,
    )

/** Whether an HTML body references remote media the policy is holding back. A
 *  newsletter keeps its images in the body rather than in the attachment list,
 *  so this is what makes the reveal affordance appear for the common case. */
fun htmlHasRemoteMedia(html: String): Boolean = html.isNotEmpty() && REMOTE_MEDIA_REF.containsMatchIn(html)

// ---- The policy the viewer enforces ----------------------------------------

/**
 * The `Content-Security-Policy` the mail viewer puts in its own head, which is
 * where a meta policy has to sit to be honoured at all.
 *
 * It mirrors `prepare_html` in the core, except for `script-src`: the viewer's
 * height-reporting bridge is an inline script of ours, so it is admitted by
 * [scriptNonce] — a value the mail cannot know — while the mail's own scripts
 * stay blocked, behind the core having already stripped them.
 */
fun mailBodyCsp(
    allowRemote: Boolean,
    scriptNonce: String,
): String {
    val img = if (allowRemote) "'self' data: http: https:" else "'self' data:"
    val media = if (allowRemote) "'self' data: blob: http: https:" else "'self' data: blob:"
    return "default-src 'none'; script-src 'nonce-$scriptNonce'; object-src 'none'; frame-src 'none'; " +
        "base-uri 'none'; form-action 'none'; " +
        "img-src $img; media-src $media; style-src 'unsafe-inline'; font-src 'self' data:;"
}

private val CSP_META =
    Regex("""(<meta\s+http-equiv="Content-Security-Policy"\s+content=")([^"]*)(")""", RegexOption.IGNORE_CASE)

private val REMOTE_SOURCES = listOf("http:", "https:")

/** Source-list tokens that let remote content through. */
private val REMOTE_TOKENS = setOf("http:", "https:", "*", "http://*", "https://*")

/**
 * Rewrite the CSP the core baked into a body to the caller's current decision.
 *
 * The viewer's own head policy is what the browser enforces, but the two
 * intersect wherever a web view does honour the baked one: leaving a body that
 * was read while its sender was blocked at "no remote content" would then keep
 * a reveal from taking effect, and leaving a body read while the sender was
 * allowed permissive would keep loading remote content after that allowance was
 * withdrawn. So both directions are applied.
 */
fun applyRemoteContentPolicy(
    html: String,
    allowRemote: Boolean,
): String =
    CSP_META.replace(html) { match ->
        match.groupValues[1] + rewriteCsp(match.groupValues[2], allowRemote) + match.groupValues[3]
    }

private fun rewriteCsp(
    csp: String,
    allowRemote: Boolean,
): String =
    csp.split(';').joinToString(";") { directive ->
        val parts = directive.trim().split(Regex("""\s+""")).filter { it.isNotEmpty() }
        val name = parts.firstOrNull()?.lowercase()
        if (name != "img-src" && name != "media-src") {
            directive
        } else if (allowRemote) {
            val missing = REMOTE_SOURCES.filterNot { source -> parts.any { it.equals(source, ignoreCase = true) } }
            if (missing.isEmpty()) directive else "$directive ${missing.joinToString(" ")}"
        } else {
            val kept = parts.drop(1).filterNot { it.lowercase() in REMOTE_TOKENS }
            // Keep the directive's own spacing, and never leave it source-less:
            // an empty source list is invalid, where `'none'` is the block we
            // want.
            val lead = directive.takeWhile { it.isWhitespace() }
            "$lead$name ${if (kept.isEmpty()) "'none'" else kept.joinToString(" ")}"
        }
    }
