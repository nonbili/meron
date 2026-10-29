package jp.nonbili.meron.shared

const val MERON_OAUTH_REDIRECT_URI = "jp.nonbili.meron.oauth://oauth"
const val MERON_OAUTH_CALLBACK_SCHEME = "jp.nonbili.meron.oauth"

fun isMeronOAuthCallbackScheme(scheme: String?): Boolean {
    val normalized = scheme?.trim()?.lowercase().orEmpty()
    return normalized == MERON_OAUTH_CALLBACK_SCHEME ||
        normalized == "msauth" ||
        normalized.startsWith("com.googleusercontent.apps.")
}

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
    require(expectedState.isNotBlank()) {
        "No OAuth flow is in progress"
    }
    val trimmed = rawUrl.trim()
    val query =
        trimmed
            .substringAfter('?', missingDelimiterValue = "")
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
