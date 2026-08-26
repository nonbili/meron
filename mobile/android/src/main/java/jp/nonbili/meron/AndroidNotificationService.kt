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
import jp.nonbili.meron.shared.accountIdIsRss
import org.json.JSONObject
import java.util.Locale

/** One new mail, as the shade shows it. Built from a `mail.newMessages` entry. */
data class NewMailItem(
    val uid: Long,
    val from: String,
    val subject: String,
    val preview: String,
    val threadKey: String,
    val date: Long,
    /** A feed entry's stable key, empty for mail (which has a uid instead).
     *  A feed is one thread, so this is all that tells its entries apart. */
    val itemKey: String = "",
)

/** A batch of arrivals for one account: one notification per item, under a
 *  per-account group summary. */
data class NewMailBatch(
    val accountId: String,
    val accountName: String,
    val folder: String,
    val count: Int,
    val items: List<NewMailItem>,
)

object AndroidNotificationService {
    private const val CHANNEL_ID = "meron_sync"

    /** New mail lives on its own channel so muting background-refresh status
     *  notifications doesn't mute the mail itself (and vice versa). */
    private const val MAIL_CHANNEL_ID = "meron_new_mail"

    /** Private extra: the summary row a child stands for, see [activeLines]. */
    private const val EXTRA_SUMMARY_LINE = "jp.nonbili.meron.extra.SUMMARY_LINE"

    /** Private extra marking a row that sits in the mail group but is not mail
     *  — an undo offer, a failure report. [activeLines] skips these, or the
     *  summary would count them as arrivals and list them as messages. */
    private const val EXTRA_ANCILLARY = "jp.nonbili.meron.extra.ANCILLARY"
    private const val NOTIFICATION_ID = 1001
    const val EXTRA_ACCOUNT_ID = "jp.nonbili.meron.extra.ACCOUNT_ID"
    const val EXTRA_FOLDER = "jp.nonbili.meron.extra.FOLDER"
    const val EXTRA_THREAD_KEY = "jp.nonbili.meron.extra.THREAD_KEY"

    fun refreshChannelIdForTesting(): String = CHANNEL_ID

    fun mailChannelIdForTesting(): String = MAIL_CHANNEL_ID

    fun ensureChannels(base: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val context = localizedAppContext(base)
        val manager = context.getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(
                CHANNEL_ID,
                context.getString(R.string.mobile_android_sync_channel_name),
                NotificationManager.IMPORTANCE_DEFAULT,
            ).apply {
                description = context.getString(R.string.mobile_android_sync_channel_desc)
            },
        )
        manager.createNotificationChannel(
            NotificationChannel(
                MAIL_CHANNEL_ID,
                context.getString(R.string.mobile_android_new_mail_channel_name),
                NotificationManager.IMPORTANCE_DEFAULT,
            ).apply {
                description = context.getString(R.string.mobile_android_new_mail_channel_desc)
            },
        )
    }

    fun canNotify(context: Context): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) ==
            PackageManager.PERMISSION_GRANTED

    fun notifyRefreshComplete(
        base: Context,
        body: String,
    ) {
        // Notifications are built from service/application contexts, which carry
        // the per-app language only on API 33+ (where the platform applies it).
        // Below that the choice lives in prefs alone, so apply it here or the
        // shade speaks the device language while the UI speaks the app's.
        val context = localizedAppContext(base)
        if (!canNotify(context)) return
        ensureChannels(context)
        val notification =
            NotificationCompat
                .Builder(context, CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_stat_mail)
                .setContentTitle(context.getString(R.string.mobile_android_refresh_complete))
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

    /** Posts new-mail notifications from a `mail.newMessages`-shaped detail
     *  object; no-op when the account is muted. */
    fun notifyNewMail(
        context: Context,
        detail: JSONObject,
    ) {
        if (detail.optBoolean("muted")) return
        notifyNewMail(context, parseNewMailBatch(detail))
    }

    /** One notification per arrival plus a group summary, the way the platform
     *  expects a mail client to report a batch: the summary collapses the lot
     *  under the account, and each child opens its own thread. */
    fun notifyNewMail(
        base: Context,
        batch: NewMailBatch,
    ) {
        val context = localizedAppContext(base)
        if (batch.items.isEmpty() || !canNotify(context)) return
        ensureChannels(context)
        val manager = NotificationManagerCompat.from(context)
        val groupKey = newMailGroupKey(batch.accountId)
        // Read the shade before posting: afterwards it also holds this batch,
        // and those lines would be counted a second time.
        val carriedOver = activeLines(context, groupKey)
        try {
            for (item in batch.items) {
                manager.notify(
                    newMailNotificationId(batch.accountId, item),
                    buildNewMailChild(context, batch, item, groupKey),
                )
            }
            // Posted last so its alert is the one the user hears (the children
            // are silenced via GROUP_ALERT_SUMMARY).
            manager.notify(
                newMailSummaryId(batch.accountId),
                buildNewMailSummary(context, batch, groupKey, carriedOver),
            )
        } catch (_: SecurityException) {
            // Notification permission can change after canNotify() checks it.
        }
    }

    private fun buildNewMailChild(
        context: Context,
        batch: NewMailBatch,
        item: NewMailItem,
        groupKey: String,
    ): android.app.Notification {
        val fallback = context.getString(R.string.mobile_android_new_mail_arrived)
        val title =
            newMailChildTitle(
                item.from,
                batch.accountName,
                context.getString(R.string.mobile_android_new_mail_channel_name),
            )
        // Lock screens that hide sensitive content show this instead: who wrote
        // and about what, but never the body.
        val publicVersion =
            NotificationCompat
                .Builder(context, MAIL_CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_stat_mail)
                .setContentTitle(title)
                .setContentText(item.subject.ifBlank { context.getString(R.string.mobile_android_new_mail_arrived) })
                .setGroup(groupKey)
                .build()
        return NotificationCompat
            .Builder(context, MAIL_CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_stat_mail)
            .setContentTitle(title)
            .setContentText(newMailChildText(item.subject, item.preview, fallback))
            .setStyle(
                NotificationCompat
                    .BigTextStyle()
                    .bigText(newMailChildBigText(item.subject, item.preview, fallback))
                    .setSummaryText(batch.accountName),
            ).setContentIntent(openAppIntent(context, batch.accountId, batch.folder, item.threadKey))
            // Archive and mark-as-read only; a reply needs a send queue that
            // survives being offline, which does not exist yet. Both address the
            // thread by id, so a payload missing any part of it gets no buttons
            // rather than buttons that quietly do nothing. A feed arrival drops
            // Archive, see [newMailSupportsArchive].
            .apply {
                if (batch.accountId.isNotBlank() && batch.folder.isNotBlank() && item.threadKey.isNotBlank()) {
                    val id = newMailNotificationId(batch.accountId, item)
                    if (newMailSupportsArchive(batch.accountId)) {
                        addAction(
                            archiveAction(
                                context,
                                accountId = batch.accountId,
                                folder = batch.folder,
                                threadKey = item.threadKey,
                                accountName = batch.accountName,
                                title = title,
                                notificationId = id,
                            ),
                        )
                    }
                    addAction(
                        markReadAction(
                            context,
                            accountId = batch.accountId,
                            folder = batch.folder,
                            threadKey = item.threadKey,
                            accountName = batch.accountName,
                            title = title,
                            notificationId = id,
                        ),
                    )
                }
            }.setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .setVisibility(NotificationCompat.VISIBILITY_PRIVATE)
            .setPublicVersion(publicVersion)
            .setGroup(groupKey)
            .setGroupAlertBehavior(NotificationCompat.GROUP_ALERT_SUMMARY)
            // The summary row this child contributes, carried on the child so a
            // later batch can read it back verbatim: the visible text is
            // "subject - preview", which would not match a freshly built line.
            .addExtras(
                android.os.Bundle().apply {
                    putString(EXTRA_SUMMARY_LINE, newMailInboxLine(item.from, item.subject))
                    // Which thread this row belongs to, so an action on any one
                    // row can find its siblings; see [cancelThreadRows].
                    putString(EXTRA_THREAD_KEY, item.threadKey)
                },
            ).apply { if (item.date > 0) setWhen(item.date * 1000L) }
            .setAutoCancel(true)
            .build()
    }

    private fun buildNewMailSummary(
        context: Context,
        batch: NewMailBatch,
        groupKey: String,
        carriedOver: List<String>,
    ): android.app.Notification {
        // Lines the user hasn't dismissed yet (read back from the shade) plus
        // this batch's, so the summary describes what is actually showing rather
        // than only the newest arrivals.
        val lines = (batch.items.map { newMailInboxLine(it.from, it.subject) } + carriedOver).distinct()
        val style = NotificationCompat.InboxStyle().setSummaryText(batch.accountName)
        lines.take(NEW_MAIL_SUMMARY_LINES).forEach { style.addLine(it) }
        return NotificationCompat
            .Builder(context, MAIL_CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_stat_mail)
            .setContentTitle(batch.accountName.ifBlank { context.getString(R.string.notify_new_message) })
            // `count` can exceed the listed messages when a batch is larger than
            // the detail carries; never report fewer than the shade is showing.
            .setContentText(
                generatedIcuString(
                    context.resourceLanguageTag(),
                    "notify.newMessageCount",
                    mapOf("count" to maxOf(batch.count, lines.size)),
                ),
            ).setStyle(style)
            .setContentIntent(openAppIntent(context, batch.accountId, batch.folder))
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .setGroup(groupKey)
            .setGroupSummary(true)
            .setGroupAlertBehavior(NotificationCompat.GROUP_ALERT_SUMMARY)
            .setAutoCancel(true)
            .build()
    }

    /** Cancels every row standing for the given thread, and returns their ids.
     *
     *  A notification is posted per arriving message, but Archive and Mark as
     *  read act on the whole thread: a reply that arrived alongside the message
     *  the user pressed the button on would otherwise stay in the shade
     *  advertising mail that is already filed or already read. */
    fun cancelThreadRows(
        context: Context,
        accountId: String,
        threadKey: String,
    ): List<Int> {
        val manager = NotificationManagerCompat.from(context)
        val groupKey = newMailGroupKey(accountId)
        val ids =
            try {
                context
                    .getSystemService(NotificationManager::class.java)
                    .activeNotifications
                    .asSequence()
                    .filter { it.notification.group == groupKey }
                    .filter { (it.notification.flags and android.app.Notification.FLAG_GROUP_SUMMARY) == 0 }
                    .filter { it.notification.extras?.getString(EXTRA_THREAD_KEY) == threadKey }
                    .map { it.id }
                    .toList()
            } catch (_: RuntimeException) {
                // Reading the shade back is an enhancement; on any refusal the
                // caller still cancels the row that was pressed.
                emptyList()
            }
        ids.forEach { manager.cancel(it) }
        return ids
    }

    /** Replaces the archived mail's row with an offer to put it back.
     *
     *  Posted into the same group as the mail it replaced, so the account's
     *  shade stays a single stack rather than sprouting a loose row. */
    fun notifyArchivedWithUndo(
        base: Context,
        accountId: String,
        accountName: String,
        folder: String,
        threadKey: String,
        title: String,
        notificationId: Int,
    ) {
        val context = localizedAppContext(base)
        if (!canNotify(context)) return
        ensureChannels(context)
        val notification =
            NotificationCompat
                .Builder(context, MAIL_CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_stat_mail)
                .setContentTitle(context.getString(R.string.notification_archived_title))
                .setContentText(title)
                .setPriority(NotificationCompat.PRIORITY_LOW)
                .setGroup(newMailGroupKey(accountId))
                // Silent: the mail already alerted once, and the user is holding
                // the phone — they just pressed the button.
                .setGroupAlertBehavior(NotificationCompat.GROUP_ALERT_SUMMARY)
                .setOnlyAlertOnce(true)
                .setTimeoutAfter(UNDO_WINDOW_MS)
                .addExtras(android.os.Bundle().apply { putBoolean(EXTRA_ANCILLARY, true) })
                .addAction(
                    NotificationCompat
                        .Action
                        .Builder(
                            R.drawable.ic_action_undo,
                            context.getString(R.string.notification_action_undo),
                            notificationActionIntent(
                                context,
                                ACTION_UNDO_ARCHIVE,
                                accountId,
                                folder,
                                threadKey,
                                accountName,
                                title,
                                notificationId,
                            ),
                        ).build(),
                ).setAutoCancel(true)
                .build()
        try {
            NotificationManagerCompat.from(context).notify(undoNotificationId(notificationId), notification)
        } catch (_: SecurityException) {
            // Notification permission can change after canNotify() checks it.
        }
    }

    /** Tells the user an action did not take, naming the mail it was meant for
     *  so a retry is possible from the app. */
    fun notifyActionFailed(
        base: Context,
        accountId: String,
        accountName: String,
        folder: String,
        threadKey: String,
        title: String,
        notificationId: Int,
        action: String,
    ) {
        val context = localizedAppContext(base)
        if (!canNotify(context)) return
        ensureChannels(context)
        // An archive that failed changed nothing, so its offer to undo would
        // undo nothing — drop it rather than leave it contradicting the failure
        // reported right beside it.
        NotificationManagerCompat.from(context).cancel(undoNotificationId(notificationId))
        val message =
            when (action) {
                ACTION_ARCHIVE -> context.getString(R.string.notification_archive_failed)
                ACTION_MARK_READ -> context.getString(R.string.notification_mark_read_failed)
                else -> context.getString(R.string.notification_undo_failed)
            }
        val notification =
            NotificationCompat
                .Builder(context, MAIL_CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_stat_mail)
                .setContentTitle(message)
                .setContentText(title)
                .setStyle(NotificationCompat.BigTextStyle().bigText(title))
                .setContentIntent(openAppIntent(context, accountId, folder, threadKey))
                .setPriority(NotificationCompat.PRIORITY_DEFAULT)
                .setGroup(newMailGroupKey(accountId))
                .setGroupAlertBehavior(NotificationCompat.GROUP_ALERT_SUMMARY)
                .addExtras(android.os.Bundle().apply { putBoolean(EXTRA_ANCILLARY, true) })
                .setAutoCancel(true)
                .build()
        try {
            NotificationManagerCompat.from(context).notify(actionFailedNotificationId(notificationId), notification)
        } catch (_: SecurityException) {
            // Notification permission can change after canNotify() checks it.
        }
    }

    /** Recounts the group summary after a row leaves the shade.
     *
     *  Android keeps a group summary showing even when its last child is gone,
     *  and its count would still describe the mail that was just handled. */
    fun refreshNewMailSummary(
        base: Context,
        accountId: String,
        accountName: String,
        folder: String,
        justCancelled: List<Int> = emptyList(),
    ) {
        val context = localizedAppContext(base)
        if (!canNotify(context)) return
        val manager = NotificationManagerCompat.from(context)
        val groupKey = newMailGroupKey(accountId)
        // Cancelling is asynchronous: a row dismissed a moment ago can still be
        // in activeNotifications, and counting it would leave the summary
        // advertising mail the shade no longer shows.
        val remaining = activeLines(context, groupKey, justCancelled.toSet())
        if (remaining.isEmpty()) {
            manager.cancel(newMailSummaryId(accountId))
            return
        }
        val batch =
            NewMailBatch(
                accountId = accountId,
                accountName = accountName,
                folder = folder,
                count = remaining.size,
                items = emptyList(),
            )
        try {
            manager.notify(
                newMailSummaryId(accountId),
                buildNewMailSummary(context, batch, groupKey, remaining),
            )
        } catch (_: SecurityException) {
            // Notification permission can change after canNotify() checks it.
        }
    }

    /** Inbox lines for the group's notifications that are still showing, newest
     *  first. Read from the shade rather than remembered in-process, because a
     *  background sync posts from a worker that doesn't outlive the batch. */
    private fun activeLines(
        context: Context,
        groupKey: String,
        excludedIds: Set<Int> = emptySet(),
    ): List<String> =
        try {
            context
                .getSystemService(NotificationManager::class.java)
                .activeNotifications
                .asSequence()
                .filterNot { it.id in excludedIds }
                .map { it.notification }
                .filter { it.group == groupKey && (it.flags and android.app.Notification.FLAG_GROUP_SUMMARY) == 0 }
                .filterNot { it.extras?.getBoolean(EXTRA_ANCILLARY) == true }
                .sortedByDescending { it.`when` }
                .mapNotNull { notification ->
                    val extras = notification.extras ?: return@mapNotNull null
                    val line =
                        extras.getString(EXTRA_SUMMARY_LINE) ?: run {
                            val title = extras.getCharSequence(NotificationCompat.EXTRA_TITLE)?.toString().orEmpty()
                            val text = extras.getCharSequence(NotificationCompat.EXTRA_TEXT)?.toString().orEmpty()
                            newMailInboxLine(title, text)
                        }
                    line.ifBlank { null }
                }.toList()
        } catch (_: RuntimeException) {
            // Reading the shade back is an enhancement, not the notification
            // itself: on any refusal fall back to this batch's lines alone.
            emptyList()
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
                // A blank thread key still navigates: the group summary targets
                // the account's folder, and only the thread is left unopened.
                if (accountId.isNotBlank() && folder.isNotBlank()) {
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

/** Most lines an expanded group summary lists; beyond this the shade elides
 *  them anyway and `count` in the collapsed line carries the total. */
const val NEW_MAIL_SUMMARY_LINES = 6

/** Read a `mail.newMessages` detail into a batch.
 *
 *  `messages` carries one entry per arrival, mail and feed alike. A core older
 *  than the per-message payload sends only the top-level summary fields, which
 *  degrade to a single notification for the batch. */
fun parseNewMailBatch(detail: JSONObject): NewMailBatch {
    val accountId = detail.optString("account")
    val listed = detail.optJSONArray("messages")
    val items =
        if (listed != null && listed.length() > 0) {
            (0 until listed.length()).mapNotNull { index ->
                listed.optJSONObject(index)?.let { entry ->
                    NewMailItem(
                        uid = entry.optLong("uid"),
                        itemKey = entry.optString("itemKey"),
                        from = entry.optString("from"),
                        subject = entry.optString("subject"),
                        preview = entry.optString("preview"),
                        threadKey = entry.optString("threadKey"),
                        date = entry.optLong("date"),
                    )
                }
            }
        } else {
            listOf(
                NewMailItem(
                    uid = 0,
                    from = detail.optString("from"),
                    subject = detail.optString("subject"),
                    preview = detail.optString("preview"),
                    threadKey = detail.optString("threadKey"),
                    date = 0,
                ),
            )
        }
    return NewMailBatch(
        accountId = accountId,
        accountName = detail.optString("accountName"),
        folder = detail.optString("folder"),
        count = detail.optInt("count", items.size),
        items = items,
    )
}

/** Notifications for one account share a group, so the shade collapses that
 *  account's arrivals under a single summary instead of interleaving accounts. */
fun newMailGroupKey(accountId: String): String = "jp.nonbili.meron.NEW_MAIL:$accountId"

/** Stable per-message id: re-posting the same mail (a retried sync, a push and
 *  a periodic refresh racing) updates its notification instead of stacking a
 *  duplicate. A feed entry has no UID but carries its own key, scoped by its
 *  feed the way core scopes the stored row — an entry key is a hash of the
 *  GUID, unique only within one subscription, and two feeds can syndicate the
 *  same post. Only a payload with neither falls back to the thread key, which
 *  cannot separate two same-titled entries of one feed. */
fun newMailNotificationId(
    accountId: String,
    item: NewMailItem,
): Int =
    if (item.uid > 0) {
        "$accountId#uid:${item.uid}".hashCode()
    } else if (item.itemKey.isNotBlank()) {
        "$accountId#item:${item.threadKey}#${item.itemKey}".hashCode()
    } else {
        "$accountId#thread:${item.threadKey}#${item.subject}".hashCode()
    }

fun newMailSummaryId(accountId: String): Int = "$accountId#summary".hashCode()

/** Sender, or the account when the envelope carried no From at all. */
fun newMailChildTitle(
    from: String,
    accountName: String,
    fallback: String = "New mail",
): String = from.trim().ifBlank { accountName.trim().ifBlank { fallback } }

/** Collapsed line: subject and the start of the body, the way the shade shows a
 *  message before it is expanded. */
fun newMailChildText(
    subject: String,
    preview: String,
    fallback: String = "New mail arrived",
): String {
    val parts = listOf(subject, preview).map { it.trim() }.filter { it.isNotEmpty() }
    return parts.joinToString(" - ").ifBlank { fallback }
}

/** Expanded body: the subject on its own line above the body snippet, so the
 *  mail is readable without opening the app. */
fun newMailChildBigText(
    subject: String,
    preview: String,
    fallback: String = "New mail arrived",
): String {
    val parts = listOf(subject, preview).map { it.trim() }.filter { it.isNotEmpty() }
    return parts.joinToString("\n").ifBlank { fallback }
}

/** Whether a new-mail row offers Archive. A feed item has nowhere to be filed —
 *  core keeps no archive folder for an RSS account, and `mail.archive` does not
 *  route feed threads — so feeds get Mark as read alone. Matches the mail list,
 *  where an RSS row cannot be archive-swiped either. */
fun newMailSupportsArchive(accountId: String): Boolean = !accountIdIsRss(accountId)

/** One summary row: who wrote, and about what. */
fun newMailInboxLine(
    from: String,
    subject: String,
): String {
    val parts = listOf(from, subject).map { it.trim() }.filter { it.isNotEmpty() }
    return parts.joinToString(" - ")
}

private fun Context.resourceLanguageTag(): String {
    val configuration = resources.configuration
    val locale =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            configuration.locales.takeIf { !it.isEmpty }?.get(0)
        } else {
            @Suppress("DEPRECATION")
            configuration.locale
        }
    return catalogLanguageTag(locale ?: Locale.ENGLISH)
}
