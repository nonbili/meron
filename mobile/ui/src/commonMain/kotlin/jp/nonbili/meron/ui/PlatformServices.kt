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

enum class AttachmentOpenResult {
    Opened,
    Unsupported,
    Blocked,
}

/** Platform actions the shared UI triggers but cannot perform itself. Provided by
 *  each host (Android Activity, iOS UIViewController). */
interface PlatformServices {
    fun openUrl(url: String)

    fun tryOpenUrl(url: String): Boolean = runCatching { openUrl(url) }.isSuccess

    fun openOAuthUrl(
        url: String,
        callbackScheme: String,
        onCallback: (String) -> Unit,
        onFailure: (String) -> Unit,
    ) {
        if (!tryOpenUrl(url)) onFailure("OAuth browser launch failed")
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

    fun openFile(
        bytes: ByteArray,
        fileName: String,
        mimeType: String,
    ): AttachmentOpenResult {
        shareFile(bytes, fileName, mimeType)
        return AttachmentOpenResult.Opened
    }

    fun saveFile(
        bytes: ByteArray,
        fileName: String,
        mimeType: String,
    )

    fun saveFile(
        bytes: ByteArray,
        fileName: String,
        mimeType: String,
        onComplete: (Result<Boolean>) -> Unit,
    ) {
        runCatching { saveFile(bytes, fileName, mimeType) }
            .onSuccess { onComplete(Result.success(true)) }
            .onFailure { onComplete(Result.failure(it)) }
    }

    fun pickFile(
        mimeTypes: List<String>,
        onPicked: (PickedFile?) -> Unit,
    )

    fun pickImage(onPicked: (PickedFile?) -> Unit)
}

/**
 * Controls the in-app UI language: the platform half of it.
 *
 * Persistence deliberately does *not* live here. These implementations are built
 * by the hosts, before the write-through preference store exists, so anything
 * they stored would bypass it and never reach the authoritative `settings` table.
 * The stored tag is owned by the shared UI instead; this only talks to the OS.
 */
interface LocaleController {
    /**
     * The language the OS has assigned this app. Three distinct answers:
     *
     *  * `null` — the platform has no per-app language concept (Android below 13,
     *    iOS), so the stored tag decides.
     *  * `""` — the platform *does* own the setting and the user chose "system
     *    default" there. That is an authoritative answer, not an absent one: it
     *    has to clear any language previously stored, or resetting to system in
     *    Android's settings would appear to do nothing.
     *  * a tag — the user picked that specific language at the OS level.
     */
    fun systemLanguageTag(): String?

    /** Push a language to the OS so its own resources follow it. */
    fun applySystem(tag: String)

    /**
     * The device's own language, as a BCP-47 tag such as `fr-FR`. Consulted only
     * when no language is chosen for this app — "system default" should mean the
     * device's language, not English.
     */
    fun deviceLanguageTag(): String

    fun displayName(tag: String): String
}
