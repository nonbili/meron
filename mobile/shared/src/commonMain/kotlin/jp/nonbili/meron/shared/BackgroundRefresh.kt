package jp.nonbili.meron.shared

fun shouldBackgroundRefreshAccount(
    accountId: String,
    paused: Boolean,
    needsReconnect: Boolean,
): Boolean = accountId.isNotBlank() && !paused && !needsReconnect

fun backgroundRefreshUsesRssProtocol(
    engine: String,
    provider: String,
    authType: String,
): Boolean = engine == "rss" || provider == "rss" || authType == "rss"

/** Poll-interval choices (minutes) offered on platforms without true background
 *  push (iOS). `0` means "Off": no foreground timer, background stays best-effort. */
val pollIntervalOptionsMinutes = listOf(0, 5, 15, 30, 60)

/** Snap a stored interval to a known option, defaulting to 15 minutes. */
fun coercePollIntervalMinutes(value: Int): Int = if (value in pollIntervalOptionsMinutes) value else 15

/** The next option in [pollIntervalOptionsMinutes], wrapping around. */
fun nextPollIntervalMinutes(current: Int): Int {
    val index = pollIntervalOptionsMinutes.indexOf(coercePollIntervalMinutes(current))
    return pollIntervalOptionsMinutes[(index + 1) % pollIntervalOptionsMinutes.size]
}

/** Seconds to wait before the next best-effort background refresh. "Off" (0) and
 *  any sub-15-minute choice still floor at 15 min, the practical iOS minimum. */
fun backgroundRefreshDelaySeconds(pollIntervalMinutes: Int): Double {
    val minutes = if (pollIntervalMinutes <= 0) 15 else pollIntervalMinutes
    return maxOf(minutes, 15) * 60.0
}

fun backgroundRefreshSummary(
    refreshed: Int,
    skipped: Int,
    failed: Int,
): String =
    when {
        refreshed > 0 && failed > 0 -> "$refreshed account(s) refreshed, $failed failed"
        refreshed > 0 -> "$refreshed account(s) refreshed"
        failed > 0 -> "$failed account(s) failed to refresh"
        skipped > 0 -> "No accounts refreshed; $skipped skipped"
        else -> "No accounts configured"
    }
