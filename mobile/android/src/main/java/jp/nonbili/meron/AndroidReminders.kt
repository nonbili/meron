package jp.nonbili.meron

import android.app.AlarmManager
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.Build
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import org.json.JSONObject
import java.util.concurrent.Executors

/** One local alarm for the next reminder; independent of mail push and network.
 * Re-read the plan at delivery, so completed/deleted/rescheduled tasks cannot
 * produce stale notifications. Alarms are inexact and may be delayed by Android.
 */
object AndroidReminders {
    private const val CHANNEL = "meron_reminders"
    private val executor = Executors.newSingleThreadExecutor()

    fun refresh(context: Context) {
        val app = context.applicationContext
        executor.execute {
            runCatching { reconcile(app) }.onFailure {
                Log.w("MeronReminders", "Unable to refresh reminders", it)
                schedule(app, System.currentTimeMillis() / 1000 + 900)
            }
        }
    }

    fun receive(
        context: Context,
        finish: () -> Unit,
    ) {
        executor.execute {
            try {
                reconcile(context.applicationContext)
            } catch (error: Exception) {
                Log.w("MeronReminders", "Unable to deliver reminders", error)
                schedule(context, System.currentTimeMillis() / 1000 + 900)
            } finally {
                finish()
            }
        }
    }

    private fun invoke(
        method: String,
        params: JSONObject = JSONObject(),
    ): JSONObject {
        val request = JSONObject().put("id", 94).put("method", method).put("params", params)
        val response = JSONObject(MeronCoreNative.invokeJson(request.toString()))
        check(!response.has("error")) { "Reminder request failed" }
        return response.getJSONObject("result")
    }

    private fun reconcile(context: Context) {
        if (!MeronCoreNative.isLoaded()) return
        val init = JSONObject(MeronCoreNative.initJson(context.filesDir.absolutePath, MeronDbKey.get(context)))
        check(!init.has("error")) { "Reminder store unavailable" }
        val manager = NotificationManagerCompat.from(context)
        val local = localizedAppContext(context)
        val nativeManager = context.getSystemService(NotificationManager::class.java)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            nativeManager.createNotificationChannel(NotificationChannel(CHANNEL, local.getString(R.string.reminders_title), NotificationManager.IMPORTANCE_DEFAULT))
        }
        val channelEnabled =
            Build.VERSION.SDK_INT < Build.VERSION_CODES.O ||
                nativeManager.getNotificationChannel(CHANNEL)?.importance != NotificationManager.IMPORTANCE_NONE
        val enabled = AndroidNotificationService.canNotify(context) && manager.areNotificationsEnabled() && channelEnabled
        val prefs = context.getSharedPreferences("meron_reminders", Context.MODE_PRIVATE)
        if (enabled && !prefs.getBoolean("enabled", false)) {
            // Enabling notifications starts with upcoming reminders, without
            // replaying every item that fell due while notifications were off.
            invoke("reminders.skipPast")
        }
        prefs.edit().putBoolean("enabled", enabled).apply()
        val plan = invoke("reminders.list").getJSONArray("reminders")
        val now = System.currentTimeMillis() / 1000
        var delivered = 0
        if (enabled) {
            for (i in 0 until plan.length()) {
                val reminder = plan.getJSONObject(i)
                if (reminder.getLong("at") > now) break
                val claimed = invoke("reminders.claim", JSONObject().put("key", reminder.getString("key"))).optJSONObject("reminder") ?: continue
                val open = PendingIntent.getActivity(context, 0, Intent(context, MeronActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP), PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
                val kind = local.getString(if (claimed.getString("kind") == "tasks") R.string.tasks_title else R.string.calendar_title)
                val body = listOf(kind, claimed.optString("body")).filter { it.isNotBlank() }.joinToString(" · ")
                val notification =
                    NotificationCompat
                        .Builder(local, CHANNEL)
                        .setSmallIcon(R.drawable.ic_stat_mail)
                        .setContentTitle(claimed.getString("title"))
                        .setContentText(body)
                        .setContentIntent(open)
                        .setVisibility(NotificationCompat.VISIBILITY_PRIVATE)
                        .setAutoCancel(true)
                        .build()
                try {
                    // A tag preserves the full key and cannot collide with mail ids.
                    manager.notify(claimed.getString("key"), 0, notification)
                    delivered += 1
                } catch (_: SecurityException) {
                    // Permission may be revoked between checking and posting.
                }
            }
        }
        // Renew the rolling recurrence plan daily even when nothing is due.
        var next = now + if (delivered >= 64) 60 else 86_400
        for (i in 0 until plan.length()) {
            val at = plan.getJSONObject(i).getLong("at")
            if (at > now) next = minOf(next, at)
        }
        schedule(context, next)
    }

    private fun schedule(
        context: Context,
        next: Long,
    ) {
        val alarm = context.getSystemService(AlarmManager::class.java)
        val intent = PendingIntent.getBroadcast(context, 0, Intent(context, AndroidReminderReceiver::class.java), PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        alarm.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, next * 1000, intent)
    }
}

class AndroidReminderReceiver : BroadcastReceiver() {
    override fun onReceive(
        context: Context,
        intent: Intent,
    ) {
        val pending = goAsync()
        AndroidReminders.receive(context) { pending.finish() }
    }
}
