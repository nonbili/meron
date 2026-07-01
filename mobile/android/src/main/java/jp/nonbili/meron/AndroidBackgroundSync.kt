package jp.nonbili.meron

import android.content.Context
import android.util.Log
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import androidx.work.workDataOf
import jp.nonbili.meron.shared.backgroundRefreshSummary
import jp.nonbili.meron.shared.backgroundRefreshUsesRssProtocol
import jp.nonbili.meron.shared.shouldBackgroundRefreshAccount
import org.json.JSONObject
import java.util.concurrent.TimeUnit

private const val KEY_MANUAL_RUN = "jp.nonbili.meron.background_sync.MANUAL_RUN"
private const val TAG = "MeronBgSync"

/** Email (or id) of an account for logs — never secrets. */
private fun accountLabel(account: JSONObject): String {
    val email = account.optString("email")
    return if (email.isNotBlank()) email else account.optString("id")
}

class AndroidBackgroundSyncWorker(
    appContext: Context,
    workerParams: WorkerParameters,
) : CoroutineWorker(appContext, workerParams) {
    override suspend fun doWork(): Result {
        if (
            !inputData.getBoolean(KEY_MANUAL_RUN, false) &&
            !loadAppBoolean(applicationContext, BACKGROUND_SYNC_ENABLED_PREF, true)
        ) {
            Log.i(TAG, "background refresh skipped because background sync is disabled")
            return Result.success()
        }
        if (!MeronCoreNative.isLoaded()) return Result.success()
        MeronCoreNative.initJson(applicationContext.filesDir.absolutePath, MeronDbKey.get(applicationContext))

        val listResponse = JSONObject(MeronCoreNative.invokeJson(requestJson(1, "account.list")))
        val accounts =
            listResponse.optJSONObject("result")?.optJSONArray("accounts")
                ?: return Result.success()

        var refreshed = 0
        var skipped = 0
        var failed = 0
        var hasTransientNetworkError = false
        for (index in 0 until accounts.length()) {
            val account = accounts.optJSONObject(index) ?: continue
            val syncRequest = androidRefreshSyncRequest(account, id = index + 2L)
            if (syncRequest == null) {
                skipped += 1
                continue
            }
            if (!refreshManagedGoogleToken(account, id = index + 2L)) {
                // Managed Gmail account whose token can no longer be minted; skip
                // the doomed sync and report it so the user can reconnect.
                Log.w(TAG, "${syncRequest.method} token refresh failed for account ${accountLabel(account)}")
                failed += 1
                continue
            }
            val syncResponse = JSONObject(MeronCoreNative.invokeJson(syncRequest.requestJson))
            if (syncResponse.has("error")) {
                val errorMessage = syncResponse.optJSONObject("error")?.optString("message") ?: ""
                // Log the error message (never the payload) so background failures
                // are diagnosable via Logcat; the app is closed during periodic runs
                // so core's own log events don't reach the platform logger.
                Log.w(
                    TAG,
                    "${syncRequest.method} failed for account ${accountLabel(account)}: $errorMessage",
                )
                if (isTransientNetworkError(errorMessage)) {
                    hasTransientNetworkError = true
                } else {
                    failed += 1
                }
            } else {
                refreshed += 1
            }
        }

        Log.i(TAG, "background refresh done: refreshed=$refreshed skipped=$skipped failed=$failed hasTransient=$hasTransientNetworkError")
        val body = backgroundRefreshSummary(refreshed = refreshed, skipped = skipped, failed = failed)
        if (
            shouldNotifyRefreshComplete(
                manualRun = inputData.getBoolean(KEY_MANUAL_RUN, false),
                failed = failed,
            )
        ) {
            AndroidNotificationService.notifyRefreshComplete(applicationContext, body)
        }
        return if (hasTransientNetworkError) {
            Log.i(TAG, "retrying background refresh due to transient network error")
            Result.retry()
        } else {
            Result.success()
        }
    }

    /**
     * For Gmail accounts backed by the on-device Google account, mint a fresh
     * access token via AccountManager (only when near expiry) and push it into
     * meron-core before sync. Returns false only when a managed account's token
     * could not be minted (user must reconnect); true otherwise, including for
     * browser-flow / non-managed accounts, which meron-core refreshes itself.
     */
    private suspend fun refreshManagedGoogleToken(
        account: JSONObject,
        id: Long,
    ): Boolean {
        val accountId = account.optString("id")
        if (accountId.isBlank()) return true
        return when (
            val refresh = GoogleAccountManagerAuth.mintIfNeeded(applicationContext, accountId)
        ) {
            is GoogleAccountManagerAuth.TokenRefresh.NotNeeded -> {
                true
            }

            is GoogleAccountManagerAuth.TokenRefresh.Refreshed -> {
                val params =
                    JSONObject()
                        .put("account_id", accountId)
                        .put("access_token", refresh.token)
                        .put("token_expires_at", refresh.expiresAt)
                val response =
                    JSONObject(
                        MeronCoreNative.invokeJson(
                            requestJson(id, "account.updateOAuthToken", params),
                        ),
                    )
                if (!response.has("error")) {
                    GoogleAccountManagerAuth.recordExpiry(applicationContext, accountId, refresh.expiresAt)
                }
                true
            }

            is GoogleAccountManagerAuth.TokenRefresh.Failed -> {
                false
            }
        }
    }
}

internal fun isTransientNetworkError(message: String): Boolean {
    val lower = message.lowercase()
    return lower.contains("tcp connect") ||
            lower.contains("dial tcp") ||
            lower.contains("timeout") ||
            lower.contains("timed out") ||
            lower.contains("network is unreachable") ||
            lower.contains("connection refused") ||
            lower.contains("connection reset") ||
            lower.contains("failed to lookup address") ||
            lower.contains("no address associated with hostname")
}

internal fun shouldNotifyRefreshComplete(
    manualRun: Boolean,
    failed: Int,
): Boolean = manualRun || failed > 0

internal data class AndroidRefreshSyncRequest(
    val method: String,
    val params: JSONObject,
    val requestJson: String,
)

internal fun androidRefreshSyncRequest(
    account: JSONObject,
    id: Long,
): AndroidRefreshSyncRequest? {
    val accountId = account.optString("id")
    if (!shouldBackgroundRefreshAccount(
            accountId = accountId,
            paused = account.optBoolean("paused"),
            needsReconnect = account.optBoolean("needs_reconnect"),
        )
    ) {
        return null
    }

    val usesRssProtocol =
        backgroundRefreshUsesRssProtocol(
            engine = account.optString("engine"),
            provider = account.optString("provider"),
            authType = account.optString("auth_type"),
        )
    val syncParams =
        if (usesRssProtocol) {
            JSONObject().put("account_id", accountId)
        } else {
            JSONObject()
                .put("account_id", accountId)
                .put("folder_id", "inbox")
                .put("limit", 50)
                .put("folders", true)
        }
    val syncMethod = if (usesRssProtocol) "rss.sync" else "mail.sync"
    return AndroidRefreshSyncRequest(
        method = syncMethod,
        params = syncParams,
        requestJson = requestJson(id, syncMethod, syncParams),
    )
}

object AndroidBackgroundSyncScheduler {
    private const val PERIODIC_WORK_NAME = "meron-background-sync"
    private const val ONCE_WORK_NAME = "meron-background-sync-once"

    fun sync(
        context: Context,
        enabled: Boolean,
    ) {
        if (enabled) {
            schedule(context)
        } else {
            cancel(context)
        }
    }

    fun schedule(context: Context) {
        val request =
            PeriodicWorkRequestBuilder<AndroidBackgroundSyncWorker>(15, TimeUnit.MINUTES)
                .setConstraints(networkConstraints())
                .build()
        WorkManager.getInstance(context).enqueueUniquePeriodicWork(
            PERIODIC_WORK_NAME,
            ExistingPeriodicWorkPolicy.UPDATE,
            request,
        )
    }

    fun cancel(context: Context) {
        WorkManager.getInstance(context).cancelUniqueWork(PERIODIC_WORK_NAME)
    }

    fun runOnce(context: Context) {
        val request =
            OneTimeWorkRequestBuilder<AndroidBackgroundSyncWorker>()
                .setConstraints(networkConstraints())
                .setInputData(workDataOf(KEY_MANUAL_RUN to true))
                .build()
        WorkManager.getInstance(context).enqueueUniqueWork(
            ONCE_WORK_NAME,
            ExistingWorkPolicy.REPLACE,
            request,
        )
    }

    private fun networkConstraints(): Constraints =
        Constraints
            .Builder()
            .setRequiredNetworkType(NetworkType.CONNECTED)
            .build()
}

private fun requestJson(
    id: Long,
    method: String,
    params: JSONObject = JSONObject(),
): String =
    JSONObject()
        .put("id", id)
        .put("method", method)
        .put("params", params)
        .toString()
