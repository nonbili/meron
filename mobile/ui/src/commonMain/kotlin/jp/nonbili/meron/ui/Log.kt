package jp.nonbili.meron.ui

enum class LogLevel { DEBUG, INFO, WARN, ERROR }

/**
 * Platform log sink: Android Logcat, iOS NSLog. Prefer the [Log] helpers over
 * raw `println`, so device logs are tagged consistently and carry a level.
 */
expect fun writeLog(
    level: LogLevel,
    tag: String,
    message: String,
    error: Throwable?,
)

object Log {
    fun d(
        tag: String,
        message: String,
    ) = writeLog(LogLevel.DEBUG, tag, message, null)

    fun i(
        tag: String,
        message: String,
    ) = writeLog(LogLevel.INFO, tag, message, null)

    fun w(
        tag: String,
        message: String,
        error: Throwable? = null,
    ) = writeLog(LogLevel.WARN, tag, message, error)

    fun e(
        tag: String,
        message: String,
        error: Throwable? = null,
    ) = writeLog(LogLevel.ERROR, tag, message, error)
}
