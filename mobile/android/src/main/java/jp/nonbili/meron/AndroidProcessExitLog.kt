package jp.nonbili.meron

import android.app.ActivityManager
import android.app.Application
import android.app.ApplicationExitInfo
import android.content.Context
import android.os.Build

internal data class AndroidProcessExit(
    val timestamp: Long,
    val processName: String,
    val pid: Int,
    val reason: Int,
    val status: Int,
    val importance: Int,
    val description: String?,
) {
    val key: String
        get() = "$timestamp:$pid:$processName"

    val isCrash: Boolean
        get() = reason == ApplicationExitInfo.REASON_CRASH || reason == ApplicationExitInfo.REASON_CRASH_NATIVE || reason == ApplicationExitInfo.REASON_ANR

    fun summary(): String {
        val reasonName =
            when (reason) {
                ApplicationExitInfo.REASON_CRASH -> "CRASH"
                ApplicationExitInfo.REASON_CRASH_NATIVE -> "CRASH_NATIVE"
                ApplicationExitInfo.REASON_ANR -> "ANR"
                ApplicationExitInfo.REASON_LOW_MEMORY -> "LOW_MEMORY"
                ApplicationExitInfo.REASON_SIGNALED -> "SIGNALED"
                ApplicationExitInfo.REASON_USER_REQUESTED -> "USER_REQUESTED"
                ApplicationExitInfo.REASON_USER_STOPPED -> "USER_STOPPED"
                ApplicationExitInfo.REASON_EXIT_SELF -> "EXIT_SELF"
                else -> reason.toString()
            }
        return "process=$processName pid=$pid exitTime=$timestamp reason=$reasonName status=$status importance=$importance " +
            redactMessage(description.orEmpty())
                .replace('\n', ' ')
                .replace('\r', ' ')
                .take(1000)
    }
}

/** Use identities rather than a wall-clock watermark: the clock can move backwards. */
internal fun unrecordedProcessExits(
    exits: List<AndroidProcessExit>,
    recordedKeys: Set<String>,
): List<AndroidProcessExit> = exits.filter { it.key !in recordedKeys }.sortedBy { it.timestamp }

internal fun latestAppCrash(
    exits: List<AndroidProcessExit>,
    processName: String,
): AndroidProcessExit? = exits.filter { it.processName == processName }.maxByOrNull { it.timestamp }?.takeIf { it.isCrash }

/** Captures native faults that never reach the JVM exception or Rust panic handlers. */
object AndroidProcessExitLog {
    fun install(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return
        val appContext = context.applicationContext
        // Read only bounded metadata, never tombstone streams. Finish before
        // the first screen reads the pending crash marker for its report prompt.
        runCatching {
            val prefs = appContext.getSharedPreferences("process-exit-log", Context.MODE_PRIVATE)
            val manager = appContext.getSystemService(ActivityManager::class.java) ?: return@runCatching
            val exits =
                manager.getHistoricalProcessExitReasons(appContext.packageName, 0, 16).map {
                    AndroidProcessExit(it.timestamp, it.processName, it.pid, it.reason, it.status, it.importance, it.description)
                }
            val fresh = unrecordedProcessExits(exits, prefs.getStringSet("recorded-exits", emptySet()).orEmpty())
            AndroidSyncDiagnosticLog.appendRedactedBatch(appContext, fresh.map { "Previous process exit: ${it.summary()}" })
            // A renderer/service exit must not be presented as a crash of
            // the app. Keep a more specific JVM/Rust report when one exists.
            val crash = latestAppCrash(fresh, Application.getProcessName())
            if (crash != null && AndroidCrashLog.pending(appContext).isEmpty()) {
                AndroidCrashLog.markPending(appContext, "Previous process exit: ${crash.summary()}")
            }
            if (exits.isNotEmpty()) {
                // Keep only this bounded history snapshot. apply() updates the
                // in-memory keys now and persists them without a main-thread fsync.
                prefs.edit().putStringSet("recorded-exits", exits.map { it.key }.toSet()).apply()
            }
        }.onFailure {
            runCatching { AndroidSyncDiagnosticLog.appendRedacted(appContext, "Cannot read previous process exits: $it") }
        }
    }
}
