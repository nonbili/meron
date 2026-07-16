package jp.nonbili.meron

import android.content.Context
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

private val emailRegex = Regex("[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}")

/** Mask the local part of an email so the diagnostic log (which a user may
 *  share with support) keeps the domain for context but never the full
 *  address, e.g. "j***@gmail.com". */
internal fun redactEmail(email: String): String {
    val at = email.indexOf('@')
    if (at <= 0) return "***"
    return "${email[0]}***${email.substring(at)}"
}

/** Mask any email addresses that leak into a message before it's written to
 *  the shareable diagnostic log. */
internal fun redactMessage(message: String): String = emailRegex.replace(message) { redactEmail(it.value) }

/** Rolling on-disk log of background sync events and app warnings/errors, so
 *  a user hitting a problem can view or share what happened without needing
 *  `adb logcat` (the app process isn't running to view logs during periodic
 *  background runs anyway). */
object AndroidSyncDiagnosticLog {
    private const val FILE_NAME = "sync-diagnostic.log"
    private const val MAX_LINES = 500

    private val timestampFormat = SimpleDateFormat("yyyy-MM-dd HH:mm:ss", Locale.US)

    @Synchronized
    fun append(
        context: Context,
        message: String,
    ) {
        val file = logFile(context)
        val timestamp = timestampFormat.format(Date())
        val lines = (if (file.exists()) file.readLines() else emptyList()) + "$timestamp $message"
        file.writeText(lines.takeLast(MAX_LINES).joinToString("\n") + "\n")
    }

    /** Append with any email addresses masked so the log stays shareable. */
    fun appendRedacted(
        context: Context,
        message: String,
    ) {
        append(context, redactMessage(message))
    }

    /** Mirror shared-code WARN/ERROR logs into this diagnostic log so app
     *  problems beyond background sync (mail actions, OAuth, push) are
     *  captured too. */
    fun installUiLogSink(context: Context) {
        val appContext = context.applicationContext
        jp.nonbili.meron.ui.Log.diagnosticSink = { line -> appendRedacted(appContext, line) }
    }

    @Synchronized
    fun read(context: Context): String {
        val file = logFile(context)
        return if (file.exists()) file.readText() else ""
    }

    private fun logFile(context: Context): File = File(context.filesDir, FILE_NAME)
}
