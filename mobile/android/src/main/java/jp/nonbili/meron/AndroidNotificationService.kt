package jp.nonbili.meron

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import org.json.JSONObject

object AndroidNotificationService {
    private const val CHANNEL_ID = "meron_sync"
    private const val NOTIFICATION_ID = 1001
    const val EXTRA_ACCOUNT_ID = "jp.nonbili.meron.extra.ACCOUNT_ID"
    const val EXTRA_FOLDER = "jp.nonbili.meron.extra.FOLDER"
    const val EXTRA_THREAD_KEY = "jp.nonbili.meron.extra.THREAD_KEY"

    fun refreshChannelIdForTesting(): String = CHANNEL_ID

    fun ensureChannels(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = context.getSystemService(NotificationManager::class.java)
        val channel =
            NotificationChannel(
                CHANNEL_ID,
                "Mail sync",
                NotificationManager.IMPORTANCE_DEFAULT,
            ).apply {
                description = "Background mail refresh status"
            }
        manager.createNotificationChannel(channel)
    }

    fun canNotify(context: Context): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) ==
            PackageManager.PERMISSION_GRANTED

    fun notifyRefreshComplete(
        context: Context,
        body: String,
    ) {
        if (!canNotify(context)) return
        ensureChannels(context)
        val notification =
            NotificationCompat
                .Builder(context, CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_stat_mail)
                .setContentTitle("Refresh complete")
                .setContentText(body)
                .setStyle(NotificationCompat.BigTextStyle().bigText(body))
                .setPriority(NotificationCompat.PRIORITY_DEFAULT)
                .setAutoCancel(true)
                .build()
        try {
            NotificationManagerCompat.from(context).notify(NOTIFICATION_ID, notification)
        } catch (_: SecurityException) {
            // Notification permission can change after canNotify() checks it.
        }
    }

    /** Posts a new-mail notification from a `mail.newMessages`-shaped detail object; no-op when muted. */
    fun notifyNewMail(
        context: Context,
        detail: JSONObject,
    ) {
        if (detail.optBoolean("muted")) return
        notifyNewMail(
            context = context,
            accountName = detail.optString("accountName"),
            from = detail.optString("from"),
            subject = detail.optString("subject"),
            count = detail.optInt("count", 1),
            accountId = detail.optString("account"),
            folder = detail.optString("folder"),
            threadKey = detail.optString("threadKey"),
        )
    }

    fun notifyNewMail(
        context: Context,
        accountName: String,
        from: String,
        subject: String,
        count: Int,
        accountId: String = "",
        folder: String = "",
        threadKey: String = "",
    ) {
        if (!canNotify(context)) return
        ensureChannels(context)
        val title =
            when {
                count > 1 -> "$count new messages"
                from.isNotBlank() -> from
                accountName.isNotBlank() -> accountName
                else -> "New mail"
            }
        val body =
            listOf(subject, accountName)
                .map { it.trim() }
                .filter { it.isNotEmpty() }
                .joinToString(" - ")
                .ifBlank { "New mail arrived" }
        val notification =
            NotificationCompat
                .Builder(context, CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_stat_mail)
                .setContentTitle(title)
                .setContentText(body)
                .setStyle(NotificationCompat.BigTextStyle().bigText(body))
                .setContentIntent(openAppIntent(context, accountId, folder, threadKey))
                .setPriority(NotificationCompat.PRIORITY_DEFAULT)
                .setAutoCancel(true)
                .build()
        try {
            NotificationManagerCompat.from(context).notify((accountName + subject).hashCode(), notification)
        } catch (_: SecurityException) {
            // Notification permission can change after canNotify() checks it.
        }
    }

    fun openAppIntent(
        context: Context,
        accountId: String = "",
        folder: String = "",
        threadKey: String = "",
    ): PendingIntent {
        val intent =
            Intent(context, ComposeMainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP
                if (accountId.isNotBlank() && folder.isNotBlank() && threadKey.isNotBlank()) {
                    putExtra(EXTRA_ACCOUNT_ID, accountId)
                    putExtra(EXTRA_FOLDER, folder)
                    putExtra(EXTRA_THREAD_KEY, threadKey)
                }
            }
        return PendingIntent.getActivity(
            context,
            listOf(accountId, folder, threadKey).joinToString("|").hashCode(),
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
    }
}
