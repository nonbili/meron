package jp.nonbili.meron.shared

/**
 * Trust-on-first-use for mail servers whose certificate cannot be validated
 * against the public roots — a local Proton Mail Bridge, say, which serves a
 * self-signed CA certificate as its leaf. The core tags those failures; the app
 * fetches the certificate that was rejected and lets the user pin it.
 *
 * Mirrors desktop/frontend/src/components/dialog/certificateTrust.ts.
 *
 * The markers must match meron-core/src/tls.rs. The SMTP one contains the
 * general one, so an untrusted certificate matches whichever check runs first.
 */

private const val UNTRUSTED_CERT_MARKER = "untrusted-certificate"
private const val UNTRUSTED_SMTP_CERT_MARKER = "smtp-untrusted-certificate"

/** Which of an account's two servers refused; each carries its own pin. */
enum class CertificateProtocol(
    val wire: String,
) {
    IMAP("imap"),
    SMTP("smtp"),
}

fun isUntrustedCertificateError(message: String): Boolean = message.contains(UNTRUSTED_CERT_MARKER)

/** The server a failed connection was talking to when it refused the certificate. */
fun untrustedCertificateProtocol(message: String): CertificateProtocol? =
    when {
        message.contains(UNTRUSTED_SMTP_CERT_MARKER) -> CertificateProtocol.SMTP
        isUntrustedCertificateError(message) -> CertificateProtocol.IMAP
        else -> null
    }

/**
 * Hex SHA-256 as colon-separated byte pairs, the form every other tool
 * (openssl, browsers) prints, so the two can be compared.
 */
fun formatCertificateFingerprint(fingerprint: String): String =
    fingerprint
        .trim()
        .uppercase()
        .chunked(2)
        .joinToString(":")

/**
 * X.509 names arrive as an RDN string ("CN=127.0.0.1, O=Proton AG"). Show the
 * common name when there is one; the full sequence is noise in a dialog.
 */
fun certificateCommonName(name: String): String {
    val match = Regex("(?:^|,)\\s*CN=([^,]+)", RegexOption.IGNORE_CASE).find(name)
    return (match?.groupValues?.get(1) ?: name).trim()
}
