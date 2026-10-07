package jp.nonbili.meron

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.coreErrorMessage
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseFolderListResponse
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.util.concurrent.TimeUnit

class AndroidMailPushService :
    Service(),
    MeronCoreNative.CoreEventListener {
    private val watched = mutableSetOf<String>()

    // Main-thread scope so `watched` is only touched from one thread; the
    // AccountManager calls inside mintAndPushToken hop to IO themselves.
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var tokenRemintJob: Job? = null
    private var foregroundActive = false

    override fun onCreate() {
        super.onCreate()
        ensureChannel(this)
        try {
            ServiceCompat.startForeground(
                this,
                NOTIFICATION_ID,
                foregroundNotification(this),
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                    ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE
                } else {
                    0
                },
            )
        } catch (e: Exception) {
            // startForeground can still be rejected (background-start
            // restrictions, battery saver states); fail quietly instead of
            // crashing service creation.
            logWarn(this, "live mail push unavailable: ${e.message}")
            stopSelf()
            return
        }
        foregroundActive = true
        if (!MeronCoreNative.isLoaded()) {
            stopSelf()
            return
        }
        MeronCoreNative.initJson(filesDir.absolutePath, MeronDbKey.get(this))
        MeronCoreNative.addCoreEventListener(this)
    }

    override fun onStartCommand(
        intent: Intent?,
        flags: Int,
        startId: Int,
    ): Int {
        if (!foregroundActive || !isEnabled(this)) {
            stopSelf()
            return START_NOT_STICKY
        }
        // The ongoing notification is built once in onCreate, but the service
        // outlives a language change: the Activity it recreates starts this
        // service again, which lands here rather than in onCreate. Rebuild the
        // channel and the row so they follow the new language instead of
        // sitting in the old one until the service is restarted.
        refreshForegroundNotification()
        scope.launch { refreshWatches() }
        ensureTokenRemintLoop()
        return START_STICKY
    }

    // specialUse has no time budget, but if the system ever delivers a
    // timeout (e.g. the type changes again) the app crashes unless the
    // service stops promptly.
    override fun onTimeout(
        startId: Int,
        fgsType: Int,
    ) {
        stopSelf()
    }

    override fun onDestroy() {
        scope.cancel()
        stopWatches()
        MeronCoreNative.removeCoreEventListener(this)
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun refreshForegroundNotification() {
        ensureChannel(this)
        try {
            NotificationManagerCompat.from(this).notify(NOTIFICATION_ID, foregroundNotification(this))
        } catch (_: SecurityException) {
            // Notification permission can be revoked while the service runs; the
            // foreground row the system is already showing stays as it is.
        }
    }

    override fun onCoreEventJson(eventJson: String) {
        val envelope = JSONObject(eventJson)
        when (envelope.optString("event")) {
            "mail.newMessages" -> {
                val detail = envelope.optJSONObject("detail") ?: return
                AndroidNotificationService.notifyNewMail(this, detail)
                AndroidUnreadWidget.refreshAsync(this)
            }

            // A sync with no arrivals — mail read or removed on another device —
            // still moves the inbox unread count.
            "mail.synced" -> {
                AndroidUnreadWidget.refreshAsync(this)
            }

            "error" -> {
                val message = envelope.optJSONObject("detail")?.optString("message").orEmpty()
                logWarn(this, "core watcher error: $message")
            }
        }
    }

    private suspend fun refreshWatches() {
        val response = MeronCoreNative.invokeJson("""{"id":1,"method":"account.list"}""")
        val accounts = parseAccountListResponse(response)
        val active = accounts.filterNot { accountSummaryIsRss(it) || it.paused || it.needsReconnect }
        // INBOX, plus the opted-in folders that hold an IDLE slot: IDLE watches
        // one mailbox per connection, so core polls the rest of the opted-in
        // folders for as long as the account is watched here.
        val foldersByAccount = active.associate { it.id to listOf(INBOX_FOLDER) + liveNotifyFolders(it.id) }
        val wanted = foldersByAccount.flatMap { (id, folders) -> folders.map { watchKey(id, it) } }.toSet()
        watched
            .filterNot { it in wanted }
            .forEach { key ->
                val (account, folder) = key.split("\n", limit = 2).let { it[0] to it.getOrElse(1) { INBOX_FOLDER } }
                stopWatch(account, folder)
                watched.remove(key)
            }
        active.forEach { account ->
            val missing = foldersByAccount[account.id].orEmpty().filterNot { watchKey(account.id, it) in watched }
            if (missing.isEmpty()) return@forEach
            // Push a fresh AccountManager token into core first: the stored one
            // may be expired, and core has no refresh token for managed accounts.
            val refresh = GoogleAccountManagerAuth.mintAndPushToken(this, account.id)
            if (refresh == GoogleAccountManagerAuth.TokenRefresh.Failed) {
                logWarn(this, "not watching ${account.id}: silent token mint failed, reconnect needed")
                return@forEach
            }
            missing.forEach { folder ->
                val startError = coreErrorMessage(startWatch(account.id, folder))
                if (startError != null) {
                    logWarn(this, "not watching ${account.id}: $startError")
                } else {
                    watched.add(watchKey(account.id, folder))
                }
            }
        }
    }

    /** Opted-in folders besides INBOX that get a live watch (cache-only read). */
    private fun liveNotifyFolders(accountId: String): List<String> =
        parseFolderListResponse(
            MeronCoreNative.invokeJson(
                JSONObject()
                    .put("id", 1)
                    .put("method", "mail.folderList")
                    .put("params", JSONObject().put("account_id", accountId))
                    .toString(),
            ),
        ).filter { it.notifyLive && !it.name.equals(INBOX_FOLDER, ignoreCase = true) }.map { it.name }

    /**
     * While watches run, periodically re-mint managed accounts' access tokens
     * so an IDLE reconnect after the ~1h token lifetime still authenticates.
     */
    private fun ensureTokenRemintLoop() {
        if (tokenRemintJob?.isActive == true) return
        tokenRemintJob =
            scope.launch {
                while (true) {
                    delay(TOKEN_REMINT_INTERVAL_MS)
                    watched.map { it.substringBefore('\n') }.distinct().forEach { accountId ->
                        GoogleAccountManagerAuth.mintAndPushToken(this@AndroidMailPushService, accountId)
                    }
                }
            }
    }

    private fun stopWatches() {
        watched.toList().forEach { key ->
            val (account, folder) = key.split("\n", limit = 2).let { it[0] to it.getOrElse(1) { INBOX_FOLDER } }
            stopWatch(account, folder)
        }
        watched.clear()
    }

    companion object {
        private const val TAG = "MeronMailPush"
        private const val CHANNEL_ID = "meron_live_mail_status"
        private const val NOTIFICATION_ID = 2001

        /** Re-mint well inside the ~1h token lifetime. */
        private val TOKEN_REMINT_INTERVAL_MS =
            TimeUnit.SECONDS.toMillis(GoogleAccountManagerAuth.TOKEN_LIFETIME_SECONDS) * 3 / 4

        fun start(context: Context) {
            if (!isEnabled(context)) return
            val intent = Intent(context, AndroidMailPushService::class.java)
            try {
                ContextCompat.startForegroundService(context, intent)
            } catch (e: Exception) {
                // Background-start restrictions (Android 12+) can reject the
                // request when nothing foreground is behind it.
                logWarn(context, "cannot start live mail push: ${e.message}")
            }
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, AndroidMailPushService::class.java))
        }

        fun sync(context: Context) {
            if (isEnabled(context)) start(context) else stop(context)
        }

        fun isEnabled(context: Context): Boolean = loadAppBoolean(context, LIVE_MAIL_PUSH_PREF, false)

        private fun logWarn(
            context: Context,
            message: String,
        ) {
            Log.w(TAG, message)
            AndroidSyncDiagnosticLog.appendRedacted(context, "$TAG: $message")
        }

        private fun ensureChannel(base: Context) {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
            // Service context: below Android 13 it does not carry the in-app
            // language, so apply it here as the mail notifications do.
            val context = localizedAppContext(base)
            val manager = context.getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_ID,
                    context.getString(R.string.mobile_android_push_channel_name),
                    NotificationManager.IMPORTANCE_LOW,
                ).apply {
                    description = context.getString(R.string.mobile_android_push_channel_desc)
                    setShowBadge(false)
                },
            )
        }

        private fun foregroundNotification(base: Context): Notification {
            val context = localizedAppContext(base)
            return NotificationCompat
                .Builder(context, CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_stat_mail)
                .setContentTitle(context.getString(R.string.mobile_android_push_notification_title))
                .setContentText(context.getString(R.string.mobile_android_push_notification_body))
                .setContentIntent(AndroidNotificationService.openAppIntent(context))
                .setPriority(NotificationCompat.PRIORITY_LOW)
                .setOngoing(true)
                .build()
        }

        private fun watchKey(
            account: String,
            folder: String,
        ): String = "$account\n$folder"

        private fun startWatch(
            account: String,
            folder: String,
        ): String = MeronCoreNative.invokeJson(watchJson("watch.start", account, folder))

        private fun stopWatch(
            account: String,
            folder: String,
        ) {
            MeronCoreNative.invokeJson(watchJson("watch.stop", account, folder))
        }

        private fun watchJson(
            method: String,
            account: String,
            folder: String,
        ): String =
            JSONObject()
                .put("id", 1)
                .put("method", method)
                .put(
                    "params",
                    JSONObject()
                        .put("account_id", account)
                        .put("folder", folder),
                ).toString()
    }
}
