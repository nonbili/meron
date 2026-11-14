package jp.nonbili.meron

import android.content.Context
import java.io.File
import java.io.RandomAccessFile
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

    // One runaway entry (a huge stack trace or server reply) shouldn't make
    // every later append and read rewrite megabytes.
    private const val MAX_ENTRY_CHARS = 8_000
    internal const val MAX_READ_BYTES = 64 * 1024

    private val timestampFormat = SimpleDateFormat("yyyy-MM-dd HH:mm:ss", Locale.US)

    @Synchronized
    fun append(
        context: Context,
        message: String,
    ) {
        appendEntries(logFile(context), listOf(message), timestampFormat.format(Date()))
    }

    /** Write a group with one file rewrite, retaining the per-entry size limit. */
    @Synchronized
    internal fun appendEntries(
        file: File,
        messages: List<String>,
        timestamp: String,
    ) {
        if (messages.isEmpty()) return
        val entries =
            messages.map { message ->
                val entry = if (message.length > MAX_ENTRY_CHARS) message.take(MAX_ENTRY_CHARS) + "…" else message
                "$timestamp $entry"
            }
        val lines = (if (file.exists()) file.readLines() else emptyList()) + entries
        file.writeText(lines.takeLast(MAX_LINES).joinToString("\n") + "\n")
    }

    @Synchronized
    fun appendRedactedBatch(
        context: Context,
        messages: List<String>,
    ) {
        if (messages.isEmpty()) return
        appendEntries(logFile(context), messages.map(::redactMessage), timestampFormat.format(Date()))
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

    /** The newest [MAX_READ_BYTES] of the log, which is all the viewer and
     *  the share sheet need. */
    @Synchronized
    fun read(context: Context): String = readTail(logFile(context))

    internal fun readTail(
        file: File,
        maxBytes: Int = MAX_READ_BYTES,
    ): String {
        if (!file.exists()) return ""
        RandomAccessFile(file, "r").use { raf ->
            val length = raf.length()
            if (length <= maxBytes) {
                val bytes = ByteArray(length.toInt())
                raf.readFully(bytes)
                return bytes.decodeToString()
            }
            raf.seek(length - maxBytes)
            val bytes = ByteArray(maxBytes)
            raf.readFully(bytes)
            // Start on a line after the cut, so the first entry isn't a torn
            // fragment (or a split UTF-8 sequence).
            val newline = bytes.indexOf('\n'.code.toByte())
            val start = if (newline >= 0) newline + 1 else bytes.indexOfFirst { (it.toInt() and 0xC0) != 0x80 }.coerceAtLeast(0)
            return bytes.decodeToString(start, bytes.size)
        }
    }

    private fun logFile(context: Context): File = File(context.filesDir, FILE_NAME)
}
