package jp.nonbili.meron.shared

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class OAuthFlowTest {
    @Test
    fun authorizeUrlResponseYieldsTheUrlOrFails() {
        assertEquals(
            "https://accounts.google.com/o/oauth2/v2/auth?client_id=c",
            parseOAuthAuthorizeUrlResponse("""{"url":"https://accounts.google.com/o/oauth2/v2/auth?client_id=c"}"""),
        )
        assertFailsWith<RuntimeException> {
            parseOAuthAuthorizeUrlResponse("""{"error":{"message":"unsupported oauth provider"}}""")
        }
        assertFailsWith<IllegalStateException> { parseOAuthAuthorizeUrlResponse("""{"url":""}""") }
    }

    @Test
    fun callbackParserRequiresSchemeCodeAndMatchingState() {
        val parsed =
            parseOAuthCallbackUrl(
                "${defaultOAuthRedirectUri()}?code=abc%20123&state=expected",
                expectedState = "expected",
            )

        assertNotNull(parsed)
        assertEquals("abc 123", parsed.code)
        assertEquals("expected", parsed.state)
        assertNull(parseOAuthCallbackUrl("mailto:me@example.com", expectedState = "expected"))
        assertFailsWith<IllegalArgumentException> {
            parseOAuthCallbackUrl("${defaultOAuthRedirectUri()}?code=abc&state=wrong", expectedState = "expected")
        }
        assertFailsWith<IllegalArgumentException> {
            parseOAuthCallbackUrl("${defaultOAuthRedirectUri()}?error=access_denied&state=expected", expectedState = "expected")
        }
        assertFailsWith<IllegalArgumentException> {
            parseOAuthCallbackUrl("${defaultOAuthRedirectUri()}?code=abc", expectedState = "")
        }
    }

    @Test
    fun callbackHelpersExposeSharedRedirectDefaults() {
        assertEquals("jp.nonbili.meron.oauth://oauth", defaultOAuthRedirectUri())
        assertTrue(isOAuthCallbackUrl("${defaultOAuthRedirectUri()}?code=abc"))
        assertTrue(isOAuthCallbackUrl("JP.NONBILI.MERON.OAUTH://oauth?code=abc"))
        assertNull(parseOAuthCallbackUrl("https://example.com/oauth?code=abc", expectedState = "state"))
        assertTrue(isMeronOAuthCallbackScheme("jp.nonbili.meron.oauth"))
        assertTrue(isMeronOAuthCallbackScheme("msauth"))
        assertTrue(isMeronOAuthCallbackScheme("com.googleusercontent.apps.client"))
        assertTrue(!isMeronOAuthCallbackScheme("https"))
    }

    @Test
    fun callbackParserAcceptsProviderNormalizedRedirectForms() {
        val redirectUri = "msauth.jp.nonbili.meron://auth"

        assertTrue(isOAuthCallbackUrl("$redirectUri/?code=abc&state=expected", redirectUri))
        assertEquals(
            "abc",
            parseOAuthCallbackUrlForRedirect(
                rawUrl = "$redirectUri/?code=abc&state=expected",
                expectedState = "expected",
                redirectUri = redirectUri,
            )?.code,
        )
        assertEquals(
            "fragment-code",
            parseOAuthCallbackUrlForRedirect(
                rawUrl = "$redirectUri#code=fragment-code&state=expected",
                expectedState = "expected",
                redirectUri = redirectUri,
            )?.code,
        )
    }

    @Test
    fun callbackParserCanValidateConfiguredHttpsRedirectUri() {
        val redirectUri = "https://mail.example.com/oauth/android"
        val parsed =
            parseOAuthCallbackUrlForRedirect(
                "$redirectUri?code=app-link-code&state=expected",
                expectedState = "expected",
                redirectUri = redirectUri,
            )

        assertNotNull(parsed)
        assertEquals("app-link-code", parsed.code)
        assertTrue(isOAuthCallbackUrl("$redirectUri?code=abc&state=expected", redirectUri))
        assertNull(
            parseOAuthCallbackUrlForRedirect(
                "https://other.example.com/oauth/android?code=abc&state=expected",
                expectedState = "expected",
                redirectUri = redirectUri,
            ),
        )
        assertNull(
            parseOAuthCallbackUrlForRedirect(
                "${defaultOAuthRedirectUri()}?code=abc&state=expected",
                expectedState = "expected",
                redirectUri = redirectUri,
            ),
        )
    }

    @Test
    fun callbackValidationErrorReportsFailureWithoutThrowing() {
        assertEquals(
            "OAuth state mismatch",
            oauthCallbackValidationError("${defaultOAuthRedirectUri()}?code=abc&state=wrong", expectedState = "expected"),
        )
        assertEquals(
            "OAuth callback missing code",
            oauthCallbackValidationError("${defaultOAuthRedirectUri()}?state=expected", expectedState = "expected"),
        )
        assertNull(
            oauthCallbackValidationError("${defaultOAuthRedirectUri()}?code=abc&state=expected", expectedState = "expected"),
        )
    }
}
