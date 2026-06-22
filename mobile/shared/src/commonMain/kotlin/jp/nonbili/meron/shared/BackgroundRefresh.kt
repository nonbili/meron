package jp.nonbili.meron.shared

fun shouldBackgroundRefreshAccount(
    accountId: String,
    paused: Boolean,
    needsReconnect: Boolean,
): Boolean {
    return accountId.isNotBlank() && !paused && !needsReconnect
}

fun backgroundRefreshUsesRssProtocol(
    engine: String,
    provider: String,
    authType: String,
): Boolean {
    return engine == "rss" || provider == "rss" || authType == "rss"
}

fun backgroundRefreshSummary(
    refreshed: Int,
    skipped: Int,
    failed: Int,
): String {
    return when {
        refreshed > 0 && failed > 0 -> "$refreshed account(s) refreshed, $failed failed"
        refreshed > 0 -> "$refreshed account(s) refreshed"
        skipped > 0 -> "No accounts refreshed; $skipped skipped"
        else -> "No accounts configured"
    }
}
