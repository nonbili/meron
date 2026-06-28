package jp.nonbili.meron.ui

import platform.Foundation.NSLog

actual fun writeLog(
    level: LogLevel,
    tag: String,
    message: String,
    error: Throwable?,
) {
    val detail = error?.let { " | ${it.stackTraceToString()}" }.orEmpty()
    // Pass the line as a single %@ argument so a '%' in the message can't be
    // interpreted as an NSLog format specifier.
    NSLog("Meron.%@ [%@] %@", tag, level.name, message + detail)
}
