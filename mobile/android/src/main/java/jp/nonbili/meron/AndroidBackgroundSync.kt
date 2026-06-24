package jp.nonbili.meron

import android.content.Context
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import jp.nonbili.meron.shared.backgroundRefreshSummary
import jp.nonbili.meron.shared.backgroundRefreshUsesRssProtocol
import jp.nonbili.meron.shared.shouldBackgroundRefreshAccount
import org.json.JSONObject
import java.util.concurrent.TimeUnit

class AndroidBackgroundSyncWorker(
    appContext: Context,
    workerParams: WorkerParameters,
) : CoroutineWorker(appContext, workerParams) {
    override suspend fun doWork(): Result {
        if (!MeronCoreNative.isLoaded()) return Result.success()
        MeronCoreNative.initJson(applicationContext.filesDir.absolutePath, MeronDbKey.get(applicationContext))

        val listResponse = JSONObject(MeronCoreNative.invokeJson(requestJson(1, "account.list")))
        val accounts =
            listResponse.optJSONObject("result")?.optJSONArray("accounts")
                ?: return Result.success()

        var refreshed = 0
        var skipped = 0
        var failed = 0
        for (index in 0 until accounts.length()) {
            val account = accounts.optJSONObject(index) ?: continue
            val syncRequest = androidRefreshSyncRequest(account, id = index + 2L)
            if (syncRequest == null) {
                skipped += 1
                continue
            }
            val syncResponse = JSONObject(MeronCoreNative.invokeJson(syncRequest.requestJson))
            if (syncResponse.has("error")) {
                failed += 1
            } else {
                refreshed += 1
            }
        }

        val body = backgroundRefreshSummary(refreshed = refreshed, skipped = skipped, failed = failed)
        AndroidNotificationService.notifyRefreshComplete(applicationContext, body)
        return Result.success()
    }
}

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

    fun runOnce(context: Context) {
        val request =
            OneTimeWorkRequestBuilder<AndroidBackgroundSyncWorker>()
                .setConstraints(networkConstraints())
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
