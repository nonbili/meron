package jp.nonbili.meron.shared

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class OAuthFlowTest {
    @Test
    fun gmailAuthorizationUrlUsesPkceAndOfflineAccess() {
        val url =
            buildOAuthAuthorizationUrl(
                OAuthAuthorizationRequest(
                    provider = "gmail",
                    clientId = "client id",
                    redirectUri = defaultOAuthRedirectUri(),
                    state = "state",
                    codeChallenge = "challenge",
                    loginHint = "me@example.com",
                ),
            )

        assertTrue(url.startsWith("https://accounts.google.com/o/oauth2/v2/auth?"))
        assertTrue(url.contains("client_id=client%20id"))
        assertTrue(url.contains("redirect_uri=jp.nonbili.meron.oauth%3A%2F%2Foauth"))
        assertTrue(url.contains("response_type=code"))
        assertTrue(url.contains("scope=openid%20email%20profile%20https%3A%2F%2Fmail.google.com%2F"))
        assertTrue(url.contains("state=state"))
        assertTrue(url.contains("code_challenge=challenge"))
        assertTrue(url.contains("code_challenge_method=S256"))
        assertTrue(url.contains("access_type=offline"))
        assertTrue(url.contains("prompt=consent"))
        assertTrue(url.contains("login_hint=me%40example.com"))
    }

    @Test
    fun outlookAuthorizationUrlUsesMicrosoftEndpointAndScopes() {
        val url =
            buildOAuthAuthorizationUrl(
                OAuthAuthorizationRequest(
                    provider = "outlook",
                    clientId = "client",
                    redirectUri = defaultOAuthRedirectUri(),
                    state = "state",
                    codeChallenge = "challenge",
                ),
            )

        assertTrue(url.startsWith("https://login.microsoftonline.com/common/oauth2/v2.0/authorize?"))
        assertTrue(url.contains("offline_access"))
        assertTrue(url.contains("profile"))
        assertTrue(url.contains("IMAP.AccessAsUser.All"))
        assertTrue(url.contains("SMTP.Send"))
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
    }

    @Test
    fun callbackHelpersExposeSharedRedirectDefaults() {
        assertEquals("jp.nonbili.meron.oauth://oauth", defaultOAuthRedirectUri())
        assertTrue(isOAuthCallbackUrl("${defaultOAuthRedirectUri()}?code=abc"))
        assertTrue(isOAuthCallbackUrl("JP.NONBILI.MERON.OAUTH://oauth?code=abc"))
        assertNull(parseOAuthCallbackUrl("https://example.com/oauth?code=abc", expectedState = "state"))
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
