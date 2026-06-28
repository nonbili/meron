package jp.nonbili.meron.ui

import android.util.Log as AndroidLog

actual fun writeLog(
    level: LogLevel,
    tag: String,
    message: String,
    error: Throwable?,
) {
    val androidTag = "Meron.$tag"
    when (level) {
        LogLevel.DEBUG -> AndroidLog.d(androidTag, message, error)
        LogLevel.INFO -> AndroidLog.i(androidTag, message, error)
        LogLevel.WARN -> AndroidLog.w(androidTag, message, error)
        LogLevel.ERROR -> AndroidLog.e(androidTag, message, error)
    }
}
