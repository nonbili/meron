package jp.nonbili.meron.shared

const val MERON_OAUTH_REDIRECT_URI = "jp.nonbili.meron.oauth://oauth"
const val MERON_OAUTH_CALLBACK_SCHEME = "jp.nonbili.meron.oauth"

data class OAuthAuthorizationRequest(
    val provider: String,
    val clientId: String,
    val redirectUri: String,
    val state: String,
    val codeChallenge: String,
    val loginHint: String = "",
)

data class OAuthCallbackResult(
    val code: String,
    val state: String,
)

fun defaultOAuthRedirectUri(): String = MERON_OAUTH_REDIRECT_URI

fun isOAuthCallbackUrl(rawUrl: String): Boolean = rawUrl.trim().startsWith("$MERON_OAUTH_CALLBACK_SCHEME:", ignoreCase = true)

fun isOAuthCallbackUrl(
    rawUrl: String,
    redirectUri: String,
): Boolean {
    val trimmed = rawUrl.trim()
    val expected = redirectUri.trim()
    if (expected.isBlank()) return isOAuthCallbackUrl(trimmed)
    val expectedWithSlash = if (expected.endsWith('/')) expected else "$expected/"
    return trimmed == expected ||
        trimmed.startsWith("$expected?") ||
        trimmed.startsWith("$expected#") ||
        trimmed.startsWith("$expectedWithSlash?") ||
        trimmed.startsWith("$expectedWithSlash#")
}

fun isPotentialOAuthCallbackUrl(rawUrl: String): Boolean {
    val trimmed = rawUrl.trim()
    val hasCodeOrError =
        trimmed.contains("?code=") ||
            trimmed.contains("&code=") ||
            trimmed.contains("?error=") ||
            trimmed.contains("&error=")
    val hasState = trimmed.contains("?state=") || trimmed.contains("&state=")
    return hasCodeOrError && (hasState || isOAuthCallbackUrl(trimmed))
}

fun buildOAuthAuthorizationUrl(request: OAuthAuthorizationRequest): String {
    val provider = request.provider.lowercase()
    val (endpoint, scope) =
        when (provider) {
            "gmail" -> {
                "https://accounts.google.com/o/oauth2/v2/auth" to
                    "openid email profile https://mail.google.com/"
            }

            "outlook" -> {
                "https://login.microsoftonline.com/common/oauth2/v2.0/authorize" to
                    "offline_access openid email profile https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send"
            }

            else -> {
                error("Unsupported OAuth provider: ${request.provider}")
            }
        }

    return endpoint + "?" +
        listOf(
            "client_id" to request.clientId,
            "redirect_uri" to request.redirectUri,
            "response_type" to "code",
            "scope" to scope,
            "state" to request.state,
            "code_challenge" to request.codeChallenge,
            "code_challenge_method" to "S256",
            "access_type" to "offline".takeIf { provider == "gmail" },
            "prompt" to "consent".takeIf { provider == "gmail" },
            "login_hint" to request.loginHint.takeIf { it.isNotBlank() },
        ).filter { it.second != null }
            .joinToString("&") { (key, value) ->
                "${urlEncode(key)}=${urlEncode(value.orEmpty())}"
            }
}

fun parseOAuthCallbackUrl(
    rawUrl: String,
    expectedState: String,
): OAuthCallbackResult? {
    val trimmed = rawUrl.trim()
    if (!isOAuthCallbackUrl(trimmed)) return null
    return parseOAuthCallbackFields(trimmed, expectedState)
}

fun parseOAuthCallbackUrlForRedirect(
    rawUrl: String,
    expectedState: String,
    redirectUri: String,
): OAuthCallbackResult? {
    val trimmed = rawUrl.trim()
    if (!isOAuthCallbackUrl(trimmed, redirectUri)) return null
    return parseOAuthCallbackFields(trimmed, expectedState)
}

private fun parseOAuthCallbackFields(
    rawUrl: String,
    expectedState: String,
): OAuthCallbackResult {
    val trimmed = rawUrl.trim()
    val query =
        trimmed.substringAfter('?', missingDelimiterValue = "")
            .ifBlank { trimmed.substringAfter('#', missingDelimiterValue = "") }
    val fields = parseUrlQuery(query)
    val error = fields["error"]
    require(error.isNullOrBlank()) {
        fields["error_description"] ?: error.orEmpty()
    }
    val state = fields["state"].orEmpty()
    require(state == expectedState) {
        "OAuth state mismatch"
    }
    val code = fields["code"].orEmpty()
    require(code.isNotBlank()) {
        "OAuth callback missing code"
    }
    return OAuthCallbackResult(code = code, state = state)
}

fun parseOAuthCallbackUrlForRedirectOrNull(
    rawUrl: String,
    expectedState: String,
    redirectUri: String,
): OAuthCallbackResult? =
    runCatching {
        parseOAuthCallbackUrlForRedirect(rawUrl, expectedState, redirectUri)
    }.getOrNull()

fun parseOAuthCallbackUrlOrNull(
    rawUrl: String,
    expectedState: String,
): OAuthCallbackResult? =
    runCatching {
        parseOAuthCallbackUrl(rawUrl, expectedState)
    }.getOrNull()

fun oauthCallbackValidationErrorForRedirect(
    rawUrl: String,
    expectedState: String,
    redirectUri: String,
): String? =
    runCatching {
        parseOAuthCallbackUrlForRedirect(rawUrl, expectedState, redirectUri)
        null
    }.getOrElse { err ->
        err.message ?: "OAuth callback failed"
    }

fun oauthCallbackValidationError(
    rawUrl: String,
    expectedState: String,
): String? =
    runCatching {
        parseOAuthCallbackUrl(rawUrl, expectedState)
        null
    }.getOrElse { err ->
        err.message ?: "OAuth callback failed"
    }

private fun parseUrlQuery(query: String): Map<String, String> {
    if (query.isBlank()) return emptyMap()
    return query
        .split('&')
        .filter { it.isNotBlank() }
        .associate { pair ->
            val key = pair.substringBefore('=')
            val value = pair.substringAfter('=', missingDelimiterValue = "")
            oauthPercentDecode(key, plusAsSpace = true) to oauthPercentDecode(value, plusAsSpace = true)
        }
}

private fun urlEncode(value: String): String {
    val out = StringBuilder()
    value.encodeToByteArray().forEach { byte ->
        val unsigned = byte.toInt() and 0xff
        val ch = unsigned.toChar()
        if (ch in 'A'..'Z' || ch in 'a'..'z' || ch in '0'..'9' || ch == '-' || ch == '_' || ch == '.' || ch == '~') {
            out.append(ch)
        } else {
            out.append('%')
            out.append(unsigned.toString(16).uppercase().padStart(2, '0'))
        }
    }
    return out.toString()
}

private fun oauthPercentDecode(
    value: String,
    plusAsSpace: Boolean,
): String {
    val bytes = mutableListOf<Byte>()
    var index = 0
    while (index < value.length) {
        val ch = value[index]
        when {
            ch == '%' && index + 2 < value.length -> {
                val hex = value.substring(index + 1, index + 3)
                val decoded = hex.toIntOrNull(16)
                if (decoded == null) {
                    bytes.add(ch.code.toByte())
                } else {
                    bytes.add(decoded.toByte())
                    index += 2
                }
            }

            ch == '+' && plusAsSpace -> {
                bytes.add(' '.code.toByte())
            }

            else -> {
                ch.toString().encodeToByteArray().forEach(bytes::add)
            }
        }
        index += 1
    }
    return bytes.toByteArray().decodeToString()
}
