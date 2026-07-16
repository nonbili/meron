package jp.nonbili.meron.ui

enum class LogLevel { DEBUG, INFO, WARN, ERROR }

internal expect val isDebugLogBuild: Boolean

private fun shouldWriteLog(level: LogLevel): Boolean = isDebugLogBuild || level == LogLevel.WARN || level == LogLevel.ERROR

/**
 * Platform log sink: Android Logcat, iOS NSLog. Prefer the [Log] helpers over
 * raw `println`, so device logs are tagged consistently and carry a level.
 */
internal expect fun writeLog(
    level: LogLevel,
    tag: String,
    message: String,
    error: Throwable?,
)

object Log {
    /** Host-installed sink that mirrors WARN/ERROR lines into the on-device
     *  diagnostic log (viewable and shareable from Settings), so problems
     *  beyond background sync are captured for debugging. Null where no
     *  diagnostic log is kept. */
    var diagnosticSink: ((line: String) -> Unit)? = null

    private fun mirrorToDiagnosticLog(
        level: LogLevel,
        tag: String,
        message: String,
        error: Throwable?,
    ) {
        val sink = diagnosticSink ?: return
        val suffix = error?.let { ": ${it.message ?: it::class.simpleName}" }.orEmpty()
        sink("${level.name[0]} $tag $message$suffix")
    }

    fun d(
        tag: String,
        message: String,
    ) {
        if (shouldWriteLog(LogLevel.DEBUG)) writeLog(LogLevel.DEBUG, tag, message, null)
    }

    fun i(
        tag: String,
        message: String,
    ) {
        if (shouldWriteLog(LogLevel.INFO)) writeLog(LogLevel.INFO, tag, message, null)
    }

    fun w(
        tag: String,
        message: String,
        error: Throwable? = null,
    ) {
        if (shouldWriteLog(LogLevel.WARN)) writeLog(LogLevel.WARN, tag, message, error)
        mirrorToDiagnosticLog(LogLevel.WARN, tag, message, error)
    }

    fun e(
        tag: String,
        message: String,
        error: Throwable? = null,
    ) {
        if (shouldWriteLog(LogLevel.ERROR)) writeLog(LogLevel.ERROR, tag, message, error)
        mirrorToDiagnosticLog(LogLevel.ERROR, tag, message, error)
    }
}
