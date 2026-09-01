package jp.nonbili.meron.ui

import android.webkit.WebView
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

@Suppress("DEPRECATION")
class MailWebViewLinkTest {
    @Test
    fun localMediaMimeTypesIncludeWebp() {
        assertEquals("image/webp", mailMediaMimeType("inline.WEBP"))
        assertEquals("image/jpeg", mailMediaMimeType("photo.jpeg"))
        assertEquals("application/octet-stream", mailMediaMimeType("file.bin"))
    }

    @Test
    fun rejectsSyntheticOriginLinks() {
        assertNull(externalMailUrl("https://appassets.androidplatform.net/page.html"))
        assertNull(externalMailUrl("HTTPS://APPASSETS.ANDROIDPLATFORM.NET/media/image.webp"))
        assertNull(externalMailUrl("http://appassets.androidplatform.net/page.html"))
        assertNull(externalMailUrl("https://appassets.androidplatform.net:443/page.html"))
        assertNull(externalMailUrl("/relative/path"))
        assertEquals("whatsapp://send?text=hello", externalMailUrl("whatsapp://send?text=hello"))
        assertNull(externalMailUrl("intent://example/#Intent;scheme=https;end"))
        assertNull(externalMailUrl("javascript:alert(1)"))
        assertNull(externalMailUrl("file:///tmp/message"))
        assertNull(externalMailUrl("jp.nonbili.meron.oauth://oauth?code=abc"))
        assertNull(externalMailUrl("msauth://jp.nonbili.meron/callback?code=abc"))
        assertNull(externalMailUrl("com.googleusercontent.apps.client:/oauth2redirect?code=abc"))
        assertEquals("https://example.com", externalMailUrl(" https://example.com "))
        assertEquals("mailto:user@example.com", externalMailUrl("mailto:user@example.com"))
        assertEquals("tel:+123456789", externalMailUrl("tel:+123456789"))
        assertEquals("zoommtg://zoom.us/join", externalMailUrl("zoommtg://zoom.us/join"))
    }

    @Test
    fun normalizesSyntheticOriginImageRefs() {
        assertEquals("/media/acct/image.webp", mailImageRef("https://appassets.androidplatform.net/media/acct/image.webp"))
        assertNull(mailImageRef("https://appassets.androidplatform.net/not-media/image.webp"))
        assertEquals("https://example.com/image.webp", mailImageRef("https://example.com/image.webp"))
        assertNull(mailImageRef("/broken-relative-image"))
        assertNull(mailImageRef("intent://image/#Intent;scheme=https;end"))
        assertEquals("https://cdn.example.com/my image.png", mailImageRef("https://cdn.example.com/my image.png"))
        assertEquals("https://cdn.example.com/image|track.png", mailImageRef("https://cdn.example.com/image|track.png"))
    }

    @Test
    fun acceptsAnchorHitTestResults() {
        assertEquals(
            "https://example.com",
            webViewLinkUrl(WebView.HitTestResult.ANCHOR_TYPE, " https://example.com "),
        )
        assertEquals(
            "https://example.com/image",
            webViewLinkUrl(WebView.HitTestResult.SRC_ANCHOR_TYPE, "https://example.com/image"),
        )
    }

    @Test
    fun rejectsNonLinksAndBlankTargets() {
        assertNull(webViewLinkUrl(WebView.HitTestResult.IMAGE_TYPE, "https://example.com/image.png"))
        assertNull(webViewLinkUrl(WebView.HitTestResult.ANCHOR_TYPE, "  "))
        assertNull(webViewLinkUrl(WebView.HitTestResult.ANCHOR_TYPE, null))
    }
}
