package jp.nonbili.meron.shared

data class AddRssAccountParams(
    val feedUrl: String,
    val displayName: String = "",
) {
    fun toJson(): String =
        jsonObject(
            "feed_url" to feedUrl.jsonString(),
            "display_name" to displayName.jsonString(),
        )
}

data class AddPasswordAccountParams(
    val email: String,
    val displayName: String = "",
    val senderName: String = "",
    val imapHost: String,
    val imapPort: Int = 993,
    val smtpHost: String,
    val smtpPort: Int = 465,
    val username: String,
    // Null omits the field, which the core reads as "keep the stored password".
    // Editing an existing account's servers must not require retyping it — the
    // UI never holds one. A blank string would *clear* the credential, so the
    // distinction matters.
    val password: String?,
    val tls: Boolean = true,
    val starttls: Boolean? = null,
    val smtpTls: Boolean? = null,
    val smtpStarttls: Boolean? = null,
    // Certificates the user accepted for a server webpki rejects. Null omits
    // the field, which keeps whatever the stored account already trusts; a new
    // account has nothing to keep, so the pin has to travel with the request
    // that creates it.
    val certPin: String? = null,
    val smtpCertPin: String? = null,
) {
    fun toJson(): String =
        jsonObject(
            "email" to email.jsonString(),
            "display_name" to displayName.jsonString(),
            "sender_name" to senderName.jsonString(),
            "imap_host" to imapHost.jsonString(),
            "imap_port" to imapPort.toString(),
            "smtp_host" to smtpHost.jsonString(),
            "smtp_port" to smtpPort.toString(),
            "username" to username.jsonString(),
            "password" to password?.jsonString(),
            "tls" to tls.toString(),
            "starttls" to starttls?.toString(),
            "smtp_tls" to smtpTls?.toString(),
            "smtp_starttls" to smtpStarttls?.toString(),
            "cert_pin" to certPin?.jsonString(),
            "smtp_cert_pin" to smtpCertPin?.jsonString(),
        )
}

data class AutodiscoverAccountParams(
    val email: String,
) {
    fun toJson(): String = jsonObject("email" to email.jsonString())
}

data class AddOAuthAccountParams(
    val email: String,
    val provider: String,
    val displayName: String = "",
    val senderName: String = "",
    val username: String = "",
    val avatarUrl: String = "",
    val accessToken: String = "",
    // Empty for platform-managed accounts (e.g. Android Gmail via AccountManager),
    // where the OS holds the long-lived credential and re-mints access tokens.
    val refreshToken: String = "",
    val tokenExpiresAt: Long = 0,
    val imapHost: String = "",
    val imapPort: Int? = null,
    val smtpHost: String = "",
    val smtpPort: Int? = null,
) {
    fun toJson(): String =
        jsonObject(
            "email" to email.jsonString(),
            "provider" to provider.jsonString(),
            "display_name" to displayName.jsonString(),
            "sender_name" to senderName.jsonString(),
            "username" to username.takeIf { it.isNotBlank() }?.jsonString(),
            "avatar_url" to avatarUrl.takeIf { it.isNotBlank() }?.jsonString(),
            "access_token" to accessToken.takeIf { it.isNotBlank() }?.jsonString(),
            "refresh_token" to refreshToken.jsonString(),
            "token_expires_at" to tokenExpiresAt.toString(),
            "imap_host" to imapHost.takeIf { it.isNotBlank() }?.jsonString(),
            "imap_port" to imapPort?.toString(),
            "smtp_host" to smtpHost.takeIf { it.isNotBlank() }?.jsonString(),
            "smtp_port" to smtpPort?.toString(),
        )
}

data class UpdateOAuthTokenParams(
    val accountId: String,
    val accessToken: String,
    val tokenExpiresAt: Long = 0,
) {
    fun toJson(): String =
        jsonObject(
            "account_id" to accountId.jsonString(),
            "access_token" to accessToken.jsonString(),
            "token_expires_at" to tokenExpiresAt.toString(),
        )
}

data class ExchangeOAuthCodeParams(
    val email: String,
    val provider: String,
    val displayName: String = "",
    val senderName: String = "",
    val code: String,
    val clientId: String,
    val clientSecret: String = "",
    val redirectUri: String,
    val codeVerifier: String,
    val tokenUrl: String = "",
) {
    fun toJson(): String =
        jsonObject(
            "email" to email.jsonString(),
            "provider" to provider.jsonString(),
            "display_name" to displayName.jsonString(),
            "sender_name" to senderName.jsonString(),
            "code" to code.jsonString(),
            "client_id" to clientId.jsonString(),
            "client_secret" to clientSecret.takeIf { it.isNotBlank() }?.jsonString(),
            "redirect_uri" to redirectUri.takeIf { it.isNotBlank() }?.jsonString(),
            "code_verifier" to codeVerifier.takeIf { it.isNotBlank() }?.jsonString(),
            "token_url" to tokenUrl.takeIf { it.isNotBlank() }?.jsonString(),
        )
}

data class OAuthAuthorizeUrlParams(
    val provider: String,
    val clientId: String,
    val redirectUri: String,
    val state: String,
    val codeChallenge: String,
    val loginHint: String = "",
) {
    fun toJson(): String =
        jsonObject(
            "provider" to provider.jsonString(),
            "client_id" to clientId.jsonString(),
            "redirect_uri" to redirectUri.jsonString(),
            "state" to state.jsonString(),
            "code_challenge" to codeChallenge.jsonString(),
            "login_hint" to loginHint.takeIf { it.isNotBlank() }?.jsonString(),
        )
}

data class AddRssFeedParams(
    val accountId: String,
    val feedUrl: String,
) {
    fun toJson(): String =
        jsonObject(
            "account" to accountId.jsonString(),
            "feed_url" to feedUrl.jsonString(),
        )
}

data class RemoveRssFeedParams(
    val threadId: String,
) {
    fun toJson(): String = jsonObject("thread_id" to threadId.jsonString())
}

/** [title] empty restores the feed's own title. */
data class RenameRssFeedParams(
    val threadId: String,
    val title: String,
) {
    fun toJson(): String =
        jsonObject(
            "thread_id" to threadId.jsonString(),
            "title" to title.jsonString(),
        )
}

data class MoveRssFeedParams(
    val threadId: String,
    val targetAccountId: String,
) {
    fun toJson(): String =
        jsonObject(
            "thread_id" to threadId.jsonString(),
            "target_account" to targetAccountId.jsonString(),
        )
}

data class ExportOpmlParams(
    val accountId: String,
) {
    fun toJson(): String = jsonObject("account" to accountId.jsonString())
}

data class ImportOpmlParams(
    val accountId: String,
    val opml: String,
) {
    fun toJson(): String =
        jsonObject(
            "account" to accountId.jsonString(),
            "opml" to opml.jsonString(),
        )
}

/**
 * Backup export options. A passphrase encrypts the document; [includeSecrets]
 * additionally puts account passwords and OAuth tokens in it, which the core
 * refuses to do without a passphrase.
 *
 * [appVersion] is the host bundle's version name and [platform] is "android"
 * or "ios"; both are recorded in the backup envelope. The core knows neither:
 * its crate version is not the app's, and desktop and mobile are versioned on
 * separate tracks, so the number only identifies a build alongside the
 * platform that wrote it.
 */
data class ExportBackupParams(
    val includeSecrets: Boolean = false,
    val passphrase: String = "",
    val appVersion: String = "",
    val platform: String = "",
) {
    fun toJson(): String =
        jsonObject(
            "include_secrets" to includeSecrets.toString(),
            "passphrase" to passphrase.jsonString(),
            "app_version" to appVersion.jsonString(),
            "platform" to platform.jsonString(),
        )
}

data class ImportBackupParams(
    val backup: String,
    val passphrase: String = "",
) {
    fun toJson(): String =
        jsonObject(
            "backup" to backup.jsonString(),
            "passphrase" to passphrase.jsonString(),
        )
}

/**
 * Settings to read from the core store. The `settings` table is authoritative
 * for mobile preferences; the platform store in front of it is only a cache so
 * the first frame can paint before the keyed DB is open.
 */
data class AppPrefsGetParams(
    val keys: List<String>,
) {
    fun toJson(): String = jsonObject("keys" to keys.jsonStringArray())
}

/**
 * One setting to persist. [valueJson] is already-encoded JSON so a preference
 * can be any of the shapes the store holds (string, bool, number, array).
 */
data class AppPrefsSetParams(
    val key: String,
    val valueJson: String,
) {
    fun toJson(): String =
        jsonObject(
            "key" to key.jsonString(),
            "value" to valueJson.ifBlank { "null" },
        )
}

data class AccountIdParams(
    val accountId: String,
) {
    fun toJson(): String = jsonObject("id" to accountId.jsonString())
}

data class AccountNameParams(
    val accountId: String,
    val name: String,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "name" to name.jsonString(),
        )
}

/** An account colour as `#rrggbb`; blank clears it. */
data class AccountColorParams(
    val accountId: String,
    val color: String,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "color" to color.jsonString(),
        )
}

data class AccountAvatarParams(
    val accountId: String,
    val avatarUrl: String,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "avatar_url" to avatarUrl.jsonString(),
        )
}

data class AccountMediaFileParams(
    val accountId: String,
    val filename: String,
    val mime: String,
    val data: String,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "filename" to filename.jsonString(),
            "mime" to mime.jsonString(),
            "data" to data.jsonString(),
        )
}

data class AccountChatWallpaperParams(
    val accountId: String,
    val presetId: String = "",
    val customUrl: String = "",
) {
    fun toJson(): String {
        val wallpaper =
            when {
                presetId.isNotBlank() -> {
                    jsonObject(
                        "kind" to "preset".jsonString(),
                        "presetId" to presetId.jsonString(),
                    )
                }

                customUrl.isNotBlank() -> {
                    jsonObject(
                        "kind" to "custom".jsonString(),
                        "url" to customUrl.jsonString(),
                    )
                }

                else -> {
                    "null"
                }
            }
        return jsonObject(
            "id" to accountId.jsonString(),
            "wallpaper" to wallpaper,
        )
    }
}

data class AccountFlagParams(
    val accountId: String,
    val enabled: Boolean,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "enabled" to enabled.toString(),
        )
}

data class AccountSaveSentCopyParams(
    val accountId: String,
    val value: Boolean?,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "value" to (value?.toString() ?: "null"),
        )
}

data class AccountRssSyncIntervalParams(
    val accountId: String,
    val minutes: Int,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "minutes" to minutes.toString(),
        )
}

data class AccountAliasParams(
    val email: String,
    val name: String = "",
) {
    fun toJson(): String =
        jsonObject(
            "email" to email.jsonString(),
            "name" to name.jsonString(),
        )
}

data class AccountAliasesParams(
    val accountId: String,
    val aliases: List<AccountAliasParams>,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "aliases" to aliases.joinToString(separator = ",", prefix = "[", postfix = "]") { it.toJson() },
        )
}

/** Serializes a [ProxySpec] the way the core's proxy parser expects it. */
private fun ProxySpec.toJson(): String =
    jsonObject(
        "mode" to mode.jsonString(),
        "host" to host.jsonString(),
        "port" to port.toString(),
        "username" to username.jsonString(),
        "password" to password.jsonString(),
    )

data class ProxyParams(
    val proxy: ProxySpec,
) {
    fun toJson(): String = jsonObject("proxy" to proxy.toJson())
}

data class AccountProxyParams(
    val accountId: String,
    val proxy: ProxySpec,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "proxy" to proxy.toJson(),
        )
}

/**
 * The server to fetch a certificate from, for a user who has to decide whether
 * to trust it. [protocol] is "imap" or "smtp" — the two can be different
 * daemons — and [starttls] picks the cleartext-then-upgrade form.
 *
 * [proxy] is the account's own proxy choice, so the probe takes the same route
 * as the connection that failed; null follows the app-wide proxy, as a
 * connection with no account override does.
 */
data class ProbeCertParams(
    val host: String,
    val port: Int,
    val protocol: String,
    val starttls: Boolean,
    val proxy: ProxySpec? = null,
) {
    fun toJson(): String =
        jsonObject(
            "host" to host.jsonString(),
            "port" to port.toString(),
            "protocol" to protocol.jsonString(),
            "starttls" to starttls.toString(),
            "proxy" to proxy?.toJson(),
        )
}

/**
 * Pins for the certificates the user accepted on an existing account. A null
 * pin leaves that server's stored pin alone; the core clears one only when the
 * key is sent explicitly as null, which nothing does yet.
 */
data class AccountCertPinParams(
    val accountId: String,
    val certPin: String? = null,
    val smtpCertPin: String? = null,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "cert_pin" to certPin?.jsonString(),
            "smtp_cert_pin" to smtpCertPin?.jsonString(),
        )
}

data class AccountSignatureParams(
    val accountId: String,
    val signature: SignatureSpec?,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            // A null signature clears the override, so the account follows the
            // app-wide signature again.
            "signature" to
                (
                    signature?.let {
                        jsonObject(
                            "mode" to it.mode.jsonString(),
                            "html" to it.html.jsonString(),
                        )
                    } ?: "null"
                ),
        )
}

data class AccountReorderParams(
    val accountIds: List<String>,
) {
    fun toJson(): String =
        jsonObject(
            "accounts" to accountIds.jsonStringArray(),
        )
}
