package jp.nonbili.meron.ui

import androidx.compose.runtime.staticCompositionLocalOf

/** Platform services for the current composition, provided by the root. */
val LocalPlatformServices = staticCompositionLocalOf<PlatformServices> { error("PlatformServices not provided") }

/** A file chosen by the user through a platform picker. */
data class PickedFile(
    val name: String,
    val bytes: ByteArray,
    val mimeType: String,
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is PickedFile) return false
        return name == other.name && mimeType == other.mimeType && bytes.contentEquals(other.bytes)
    }

    override fun hashCode(): Int {
        var result = name.hashCode()
        result = 31 * result + mimeType.hashCode()
        result = 31 * result + bytes.contentHashCode()
        return result
    }
}

/** Platform actions the shared UI triggers but cannot perform itself. Provided by
 *  each host (Android Activity, iOS UIViewController). */
interface PlatformServices {
    fun openUrl(url: String)

    fun openOAuthUrl(
        url: String,
        callbackScheme: String,
        onCallback: (String) -> Unit,
        onFailure: (String) -> Unit,
    ) {
        runCatching { openUrl(url) }.onFailure { onFailure(it.message ?: "OAuth browser launch failed") }
    }

    fun copyText(
        label: String,
        value: String,
    )

    fun copyImage(
        bytes: ByteArray,
        mimeType: String,
        label: String,
    )

    fun shareFile(
        bytes: ByteArray,
        fileName: String,
        mimeType: String,
    )

    fun saveFile(
        bytes: ByteArray,
        fileName: String,
        mimeType: String,
    )

    fun pickFile(
        mimeTypes: List<String>,
        onPicked: (PickedFile?) -> Unit,
    )

    fun pickImage(onPicked: (PickedFile?) -> Unit)
}

/** Controls the in-app UI language. */
interface LocaleController {
    fun currentLanguageTag(): String

    fun apply(tag: String)

    fun displayName(tag: String): String
}
