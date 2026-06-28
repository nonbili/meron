package jp.nonbili.meron.ui

import platform.Foundation.NSLog
import kotlin.experimental.ExperimentalNativeApi
import kotlin.native.Platform

@OptIn(ExperimentalNativeApi::class)
internal actual val isDebugLogBuild: Boolean = Platform.isDebugBinary

internal actual fun writeLog(
    level: LogLevel,
    tag: String,
    message: String,
    error: Throwable?,
) {
    val detail = error?.let { " | ${it.stackTraceToString()}" }.orEmpty()
    // Build the whole line and pass it as the format string with NO varargs.
    // Kotlin/Native's bridge for NSLog's C *object* varargs (%@) passes a bad
    // pointer and crashes (EXC_BAD_ACCESS in CFStringAppendFormat), so we avoid
    // varargs entirely. Escape '%' so the message can't be read as a format spec.
    val line = "Meron.$tag [${level.name}] $message$detail".replace("%", "%%")
    NSLog(line)
}
