package jp.nonbili.meron

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.parseAccountListResponse
import org.json.JSONObject

class AndroidMailPushService : Service(), MeronCoreNative.CoreEventListener {
    private val watched = mutableSetOf<String>()

    override fun onCreate() {
        super.onCreate()
        ensureChannel(this)
        startForeground(NOTIFICATION_ID, foregroundNotification(this))
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
        if (!isEnabled(this)) {
            stopSelf()
            return START_NOT_STICKY
        }
        refreshWatches()
        return START_STICKY
    }

    override fun onDestroy() {
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
                if (!detail.optBoolean("muted")) {
                    AndroidNotificationService.notifyNewMail(
                        context = this,
                        accountName = detail.optString("accountName"),
                        from = detail.optString("from"),
                        subject = detail.optString("subject"),
                        count = detail.optInt("count", 1),
                        accountId = detail.optString("account"),
                        folder = detail.optString("folder"),
                        threadKey = detail.optString("threadKey"),
                    )
                }
            }
        }
    }

    private fun refreshWatches() {
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
            if (watched.add(key)) {
                startWatch(account.id, INBOX_FOLDER)
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
        private const val CHANNEL_ID = "meron_live_mail_status"
        private const val NOTIFICATION_ID = 2001

        fun start(context: Context) {
            if (!isEnabled(context)) return
            val intent = Intent(context, AndroidMailPushService::class.java)
            ContextCompat.startForegroundService(context, intent)
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
                .setSmallIcon(android.R.drawable.stat_notify_sync)
                .setContentTitle("Meron live mail push")
                .setContentText("Watching mail accounts for new messages")
                .setContentIntent(AndroidNotificationService.openAppIntent(context))
                .setPriority(NotificationCompat.PRIORITY_LOW)
                .setOngoing(true)
                .build()

        private fun watchKey(account: String, folder: String): String = "$account\n$folder"

        private fun startWatch(account: String, folder: String) {
            MeronCoreNative.invokeJson(watchJson("watch.start", account, folder))
        }

        private fun stopWatch(account: String, folder: String) {
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
