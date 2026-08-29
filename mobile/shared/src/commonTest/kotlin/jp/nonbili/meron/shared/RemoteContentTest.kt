package jp.nonbili.meron.shared

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class RemoteContentTest {
    @Test
    fun normalizesSenderTheWayTheCoreDoes() {
        assertEquals("news@example.com", normalizeSenderAddr("  News <News@Example.com> "))
        assertEquals("news@example.com", normalizeSenderAddr("NEWS@example.com"))
        // Nothing to unwrap: a stray bracket must not swallow the address.
        assertEquals("a<b@example.com", normalizeSenderAddr("a<b@example.com"))
        assertEquals("", normalizeSenderAddr("   "))
    }

    @Test
    fun policyFollowsAccountToggleThenAllowlist() {
        val blocked = RemoteContentPolicy(accountAllows = false, allowedSenders = listOf("news@example.com"))
        assertTrue(blocked.allows("News <news@example.com>"))
        assertFalse(blocked.allows("other@example.com"))
        // An empty From is never a member, even beside an allowlist entry that
        // normalized down to nothing.
        assertFalse(RemoteContentPolicy(allowedSenders = listOf("")).allows(""))
        assertTrue(RemoteContentPolicy(accountAllows = true).allows("other@example.com"))
    }

    @Test
    fun allowlistEditsStayNormalizedAndDeduped() {
        val once = withRemoteSender(emptyList(), "News <News@Example.com>", allowed = true)
        assertEquals(listOf("news@example.com"), once)
        // Allowing the same sender twice must not add a second row.
        assertEquals(once, withRemoteSender(once, "news@example.com", allowed = true))
        assertEquals(emptyList(), withRemoteSender(once, "NEWS@example.com", allowed = false))
        // An address that normalizes to nothing leaves the list alone, so the
        // caller can skip the write.
        assertEquals(once, withRemoteSender(once, "  ", allowed = true))
        assertEquals(
            listOf("news@example.com", "b@example.com"),
            sanitizeRemoteSenders(listOf("News <news@example.com>", "NEWS@EXAMPLE.COM", "", "b@example.com")),
        )
    }

    @Test
    fun onlyNetworkBackedAttachmentsAreGated() {
        val cached = MessageAttachment(filename = "a.png", mimeType = "image/png", key = "acct/inbox/1/0.png")
        val dataUrl = MessageAttachment(filename = "b.png", mimeType = "image/png", url = "data:image/png;base64,AAAA")
        val remote = MessageAttachment(filename = "c.png", mimeType = "image/png", url = "https://tracker.example/c.png")
        val images = listOf(cached, dataUrl, remote)

        assertEquals(images, visibleImageAttachments(images, allowRemote = true))
        assertEquals(listOf(cached, dataUrl), visibleImageAttachments(images, allowRemote = false))
    }

    @Test
    fun detectsRemoteMediaInABody() {
        assertTrue(htmlHasRemoteMedia("""<img src="https://tracker.example/pixel.gif">"""))
        assertTrue(htmlHasRemoteMedia("""<img src="//cdn.example/x.png">"""))
        assertTrue(htmlHasRemoteMedia("""<div style="background:url(https://cdn.example/bg.png)">hi</div>"""))
        assertTrue(htmlHasRemoteMedia("""<video poster="https://cdn.example/p.jpg"></video>"""))
        // Inline references are local, and a plain link is not media.
        assertFalse(htmlHasRemoteMedia("""<img src="/media/acct/inbox/1/0.png">"""))
        assertFalse(htmlHasRemoteMedia("""<img src="data:image/png;base64,AAAA">"""))
        assertFalse(htmlHasRemoteMedia("""<a href="https://example.com">ok</a>"""))
    }

    @Test
    fun viewerCspGatesRemoteContentAndAdmitsOnlyOurScript() {
        val blocked = mailBodyCsp(allowRemote = false, scriptNonce = "abc123")
        assertTrue(blocked.contains("img-src 'self' data:;"))
        assertTrue(blocked.contains("media-src 'self' data: blob:;"))
        assertFalse(blocked.contains("https:"))

        val allowed = mailBodyCsp(allowRemote = true, scriptNonce = "abc123")
        assertTrue(allowed.contains("img-src 'self' data: http: https:;"))
        assertTrue(allowed.contains("media-src 'self' data: blob: http: https:;"))
        // The mail's own scripts stay blocked either way: only the nonce runs.
        assertTrue(allowed.contains("script-src 'nonce-abc123';"))
        assertTrue(allowed.contains("default-src 'none';"))
    }

    @Test
    fun rewritesTheBakedPolicyInBothDirections() {
        val blockedBody =
            """<html><head><meta http-equiv="Content-Security-Policy" content="default-src 'none'; """ +
                """img-src 'self' data:; media-src 'self' data: blob:; style-src 'unsafe-inline';"></head><body>hi</body></html>"""

        val revealed = applyRemoteContentPolicy(blockedBody, allowRemote = true)
        assertTrue(revealed.contains("img-src 'self' data: http: https:;"))
        assertTrue(revealed.contains("media-src 'self' data: blob: http: https:;"))
        // Everything else is left exactly as the core wrote it.
        assertTrue(revealed.contains("default-src 'none';"))
        assertTrue(revealed.contains("style-src 'unsafe-inline';"))

        // ...and withdrawing the trust tightens a body that was baked permissive.
        val reblocked = applyRemoteContentPolicy(revealed, allowRemote = false)
        assertEquals(blockedBody, reblocked)
    }

    @Test
    fun blockingNeverLeavesADirectiveWithoutSources() {
        val body = """<meta http-equiv="Content-Security-Policy" content="img-src http: https:; media-src *;">"""
        val blocked = applyRemoteContentPolicy(body, allowRemote = false)
        assertTrue(blocked.contains("img-src 'none';"))
        assertTrue(blocked.contains("media-src 'none'"))
    }

    @Test
    fun allowingIsIdempotent() {
        val body = """<meta http-equiv="Content-Security-Policy" content="img-src 'self' data: http: https:;">"""
        assertEquals(body, applyRemoteContentPolicy(body, allowRemote = true))
    }

    @Test
    fun bodiesWithoutABakedPolicyAreLeftAlone() {
        val body = "<p>plain enough</p>"
        assertEquals(body, applyRemoteContentPolicy(body, allowRemote = false))
    }
}
