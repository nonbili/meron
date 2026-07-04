package jp.nonbili.meron

import android.content.Context
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/** Rolling on-disk log of background sync events, so a user seeing repeated
 *  "N failed" notifications can share what happened without needing `adb
 *  logcat` (the app process isn't running to view logs during periodic
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

    @Synchronized
    fun read(context: Context): String {
        val file = logFile(context)
        return if (file.exists()) file.readText() else ""
    }

    private fun logFile(context: Context): File = File(context.filesDir, FILE_NAME)
}
