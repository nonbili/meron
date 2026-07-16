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
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.parseAccountListResponse
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
            Log.w(TAG, "live mail push unavailable: ${e.message}")
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

    override fun onCoreEventJson(eventJson: String) {
        val envelope = JSONObject(eventJson)
        when (envelope.optString("event")) {
            "mail.newMessages" -> {
                val detail = envelope.optJSONObject("detail") ?: return
                AndroidNotificationService.notifyNewMail(this, detail)
            }
        }
    }

    private suspend fun refreshWatches() {
        val response = MeronCoreNative.invokeJson("""{"id":1,"method":"account.list"}""")
        val accounts = parseAccountListResponse(response)
        val active = accounts.filterNot { accountSummaryIsRss(it) || it.paused || it.needsReconnect }
        val wanted = active.map { watchKey(it.id, INBOX_FOLDER) }.toSet()
        watched
            .filterNot { it in wanted }
            .forEach { key ->
                val (account, folder) = key.split("\n", limit = 2).let { it[0] to it.getOrElse(1) { INBOX_FOLDER } }
                stopWatch(account, folder)
                watched.remove(key)
            }
        active.forEach { account ->
            val key = watchKey(account.id, INBOX_FOLDER)
            if (key in watched) return@forEach
            // Push a fresh AccountManager token into core first: the stored one
            // may be expired, and core has no refresh token for managed accounts.
            val refresh = GoogleAccountManagerAuth.mintAndPushToken(this, account.id)
            if (refresh == GoogleAccountManagerAuth.TokenRefresh.Failed) {
                Log.w(TAG, "not watching ${account.id}: silent token mint failed, reconnect needed")
                return@forEach
            }
            watched.add(key)
            startWatch(account.id, INBOX_FOLDER)
        }
    }

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
                    watched.toList().forEach { key ->
                        val accountId = key.substringBefore('\n')
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
                Log.w(TAG, "cannot start live mail push: ${e.message}")
            }
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, AndroidMailPushService::class.java))
        }

        fun sync(context: Context) {
            if (isEnabled(context)) start(context) else stop(context)
        }

        fun isEnabled(context: Context): Boolean = loadAppBoolean(context, LIVE_MAIL_PUSH_PREF, false)

        private fun ensureChannel(context: Context) {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
            val manager = context.getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_ID,
                    "Live mail push",
                    NotificationManager.IMPORTANCE_LOW,
                ).apply {
                    description = "Keeps IMAP IDLE connected for new mail alerts"
                    setShowBadge(false)
                },
            )
        }

        private fun foregroundNotification(context: Context): Notification =
            NotificationCompat
                .Builder(context, CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_stat_mail)
                .setContentTitle("Meron live mail push")
                .setContentText("Watching mail accounts for new messages")
                .setContentIntent(AndroidNotificationService.openAppIntent(context))
                .setPriority(NotificationCompat.PRIORITY_LOW)
                .setOngoing(true)
                .build()

        private fun watchKey(
            account: String,
            folder: String,
        ): String = "$account\n$folder"

        private fun startWatch(
            account: String,
            folder: String,
        ) {
            MeronCoreNative.invokeJson(watchJson("watch.start", account, folder))
        }

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
