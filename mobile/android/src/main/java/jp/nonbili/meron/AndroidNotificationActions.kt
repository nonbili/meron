package jp.nonbili.meron

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.work.BackoffPolicy
import androidx.work.CoroutineWorker
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import androidx.work.workDataOf
import jp.nonbili.meron.shared.notificationThreadId
import org.json.JSONArray
import org.json.JSONObject
import java.util.concurrent.TimeUnit

private const val TAG = "MeronNotifAction"

/** Raised in-process after a notification action changes a mailbox, so a
 *  running app reloads the folder it just changed behind the UI's back. Named
 *  like a core event because it travels the same path and is handled beside
 *  them; see [MeronCoreNative.dispatchLocalEvent]. */
const val MAILBOX_CHANGED_EVENT = "mail.mailboxChanged"

const val ACTION_ARCHIVE = "jp.nonbili.meron.action.ARCHIVE"
const val ACTION_MARK_READ = "jp.nonbili.meron.action.MARK_READ"
const val ACTION_UNDO_ARCHIVE = "jp.nonbili.meron.action.UNDO_ARCHIVE"

private const val EXTRA_NOTIFICATION_ID = "jp.nonbili.meron.extra.NOTIFICATION_ID"
private const val EXTRA_ACCOUNT_NAME = "jp.nonbili.meron.extra.ACCOUNT_NAME"
private const val EXTRA_TITLE = "jp.nonbili.meron.extra.TITLE"

private const val KEY_ACTION = "action"
private const val KEY_ACCOUNT_ID = "accountId"
private const val KEY_FOLDER = "folder"
private const val KEY_THREAD_KEY = "threadKey"
private const val KEY_ACCOUNT_NAME = "accountName"
private const val KEY_TITLE = "title"
private const val KEY_NOTIFICATION_ID = "notificationId"

/** Handles the Archive / Mark as read / Undo buttons on a new-mail
 *  notification.
 *
 *  The shade is updated here and now — the row disappears the moment the button
 *  is pressed — while the mailbox change runs in a [NotificationActionWorker].
 *  A receiver only gets a few seconds before the system may kill the process,
 *  which an IMAP move over a cold connection can easily outlast; WorkManager
 *  also survives the reboot or process death that would otherwise drop the
 *  change silently. */
class AndroidNotificationActionReceiver : BroadcastReceiver() {
    override fun onReceive(
        context: Context,
        intent: Intent,
    ) {
        val action = intent.action ?: return
        val accountId = intent.getStringExtra(AndroidNotificationService.EXTRA_ACCOUNT_ID).orEmpty()
        val folder = intent.getStringExtra(AndroidNotificationService.EXTRA_FOLDER).orEmpty()
        val threadKey = intent.getStringExtra(AndroidNotificationService.EXTRA_THREAD_KEY).orEmpty()
        val accountName = intent.getStringExtra(EXTRA_ACCOUNT_NAME).orEmpty()
        val title = intent.getStringExtra(EXTRA_TITLE).orEmpty()
        val notificationId = intent.getIntExtra(EXTRA_NOTIFICATION_ID, 0)
        if (accountId.isBlank() || folder.isBlank() || threadKey.isBlank()) return

        val manager = NotificationManagerCompat.from(context)
        when (action) {
            ACTION_ARCHIVE -> {
                // The whole thread is filed, so every row standing for it goes,
                // not just the one pressed.
                val cleared = AndroidNotificationService.cancelThreadRows(context, accountId, threadKey)
                if (cleared.isEmpty()) manager.cancel(notificationId)
                // The group summary must be recounted without the rows just
                // cancelled. The undo row comes later, from the worker, once
                // the archive has reported the copies an undo would move back.
                AndroidNotificationService.refreshNewMailSummary(
                    context,
                    accountId,
                    accountName,
                    folder,
                    cleared + notificationId,
                )
            }

            ACTION_MARK_READ -> {
                val cleared = AndroidNotificationService.cancelThreadRows(context, accountId, threadKey)
                if (cleared.isEmpty()) manager.cancel(notificationId)
                AndroidNotificationService.refreshNewMailSummary(
                    context,
                    accountId,
                    accountName,
                    folder,
                    cleared + notificationId,
                )
            }

            ACTION_UNDO_ARCHIVE -> {
                manager.cancel(undoNotificationId(notificationId))
            }
        }

        enqueue(
            context,
            action = action,
            accountId = accountId,
            folder = folder,
            threadKey = threadKey,
            accountName = accountName,
            title = title,
            notificationId = notificationId,
        )
    }

    private fun enqueue(
        context: Context,
        action: String,
        accountId: String,
        folder: String,
        threadKey: String,
        accountName: String,
        title: String,
        notificationId: Int,
    ) {
        val request =
            OneTimeWorkRequestBuilder<NotificationActionWorker>()
                .setInputData(
                    workDataOf(
                        KEY_ACTION to action,
                        KEY_ACCOUNT_ID to accountId,
                        KEY_FOLDER to folder,
                        KEY_THREAD_KEY to threadKey,
                        KEY_ACCOUNT_NAME to accountName,
                        KEY_TITLE to title,
                        KEY_NOTIFICATION_ID to notificationId,
                    ),
                ).setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS)
                .build()
        // Keyed per thread so an undo runs after the archive it reverses, never
        // beside it — two actions racing on one thread would otherwise leave the
        // mail in whichever folder finished last.
        //
        // APPEND_OR_REPLACE, not APPEND: a chain that ended in failure stays in
        // the graph under its unique name, and anything appended to a failed
        // prerequisite is failed too without ever running. One permanent error
        // would leave every later action on that thread silently inert while its
        // buttons still looked live.
        WorkManager.getInstance(context).enqueueUniqueWork(
            "meron-notification-action:$accountId#$threadKey",
            ExistingWorkPolicy.APPEND_OR_REPLACE,
            request,
        )
    }
}

/** Runs one notification action against core. */
class NotificationActionWorker(
    appContext: Context,
    workerParams: WorkerParameters,
) : CoroutineWorker(appContext, workerParams) {
    override suspend fun doWork(): Result {
        val action = inputData.getString(KEY_ACTION) ?: return Result.success()
        val accountId = inputData.getString(KEY_ACCOUNT_ID).orEmpty()
        val folder = inputData.getString(KEY_FOLDER).orEmpty()
        val threadKey = inputData.getString(KEY_THREAD_KEY).orEmpty()
        val accountName = inputData.getString(KEY_ACCOUNT_NAME).orEmpty()
        val title = inputData.getString(KEY_TITLE).orEmpty()
        val notificationId = inputData.getInt(KEY_NOTIFICATION_ID, 0)
        if (!MeronCoreNative.isLoaded()) return Result.success()
        MeronCoreNative.initJson(applicationContext.filesDir.absolutePath, MeronDbKey.get(applicationContext))

        // Managed Gmail accounts hold a token that expires between syncs; an
        // IMAP write with a stale one fails as an auth error rather than a
        // retryable network blip.
        GoogleAccountManagerAuth.mintAndPushToken(applicationContext, accountId, requestId = 1)

        val threadId = notificationThreadId(accountId, folder, threadKey)
        val request =
            when (action) {
                ACTION_ARCHIVE -> {
                    requestJson(2, "mail.archive", JSONObject().put("thread_id", threadId))
                }

                ACTION_MARK_READ -> {
                    requestJson(
                        2,
                        "mail.markRead",
                        JSONObject().put("thread_id", threadId).put("seen", true),
                    )
                }

                ACTION_UNDO_ARCHIVE -> {
                    // Where the archive put the mail, and exactly the copies it
                    // made there — recorded by the archive that ran just before
                    // this (work for one thread is APPENDed, so it has finished),
                    // and the only case the undo row is ever posted for. Without
                    // them there is no safe way back: the thread key would also
                    // take older mail of the conversation.
                    val archived =
                        takeArchivedFolder(applicationContext, accountId, threadKey)
                            ?.takeIf { it.uids.isNotEmpty() }
                    if (archived == null) {
                        Log.w(TAG, "$action has no archived copies to move back")
                        AndroidNotificationService.notifyActionFailed(
                            applicationContext,
                            accountId = accountId,
                            accountName = accountName,
                            folder = folder,
                            threadKey = threadKey,
                            title = title,
                            notificationId = notificationId,
                            action = action,
                        )
                        return Result.failure()
                    }
                    requestJson(
                        2,
                        "mail.move",
                        JSONObject()
                            .put("thread_id", notificationThreadId(accountId, archived.folder, threadKey))
                            .put("target_folder_id", folder)
                            .put("message_ids", JSONArray(archived.uids.map { it.toString() })),
                    )
                }

                else -> {
                    return Result.success()
                }
            }

        val response = JSONObject(MeronCoreNative.invokeJson(request))
        val error = response.optJSONObject("error")?.optString("message").orEmpty()
        if (error.isEmpty()) {
            if (action == ACTION_ARCHIVE) {
                // Core picks the archive folder itself; remember its answer so a
                // later undo moves the mail out of the folder it really landed
                // in rather than one guessed from the folder roles.
                val result = response.optJSONObject("result")
                result?.optString("folder")?.takeIf { it.isNotBlank() }?.let { archived ->
                    val targetUids =
                        result
                            .optJSONArray("target_uids")
                            ?.let { uids -> (0 until uids.length()).map { uids.getLong(it) } }
                            .orEmpty()
                    // Offer Undo only for copies it can move back exactly.
                    if (targetUids.isNotEmpty()) {
                        rememberArchivedFolder(applicationContext, accountId, threadKey, archived, targetUids)
                        AndroidNotificationService.notifyArchivedWithUndo(
                            applicationContext,
                            accountId = accountId,
                            accountName = accountName,
                            folder = folder,
                            threadKey = threadKey,
                            title = title,
                            notificationId = notificationId,
                        )
                    }
                }
            }
            // A running app has no other way to learn the mailbox changed: this
            // ran outside the UI, and core raises no event for a move or a flag
            // write. Without it the thread list keeps showing mail the user just
            // archived from the shade. Nobody is listening when the app is
            // closed, which is fine — it reloads on next start anyway.
            notifyAppOfMailboxChange(accountId, folder)
            return Result.success()
        }

        if (isTransientNetworkError(error)) {
            Log.i(TAG, "retrying $action after transient error")
            return Result.retry()
        }
        Log.w(TAG, "$action failed: ${redactMessage(error)}")
        AndroidNotificationService.notifyActionFailed(
            applicationContext,
            accountId = accountId,
            accountName = accountName,
            folder = folder,
            threadKey = threadKey,
            title = title,
            notificationId = notificationId,
            action = action,
        )
        return Result.failure()
    }

    private fun notifyAppOfMailboxChange(
        accountId: String,
        folder: String,
    ) {
        MeronCoreNative.dispatchLocalEvent(
            JSONObject()
                .put("event", MAILBOX_CHANGED_EVENT)
                .put("detail", JSONObject().put("account", accountId).put("folder", folder))
                .toString(),
        )
    }

    private fun requestJson(
        id: Long,
        method: String,
        params: JSONObject,
    ): String =
        JSONObject()
            .put("id", id)
            .put("method", method)
            .put("params", params)
            .toString()
}

/** Where a notification-archived thread was moved to, kept only until its undo
 *  offer is taken or expires.
 *
 *  Its own preferences file rather than the app's: these entries are transient
 *  bookkeeping between two background workers, and the app never reads them. */
private const val ARCHIVED_FOLDER_PREFS = "meron_notification_archived"

private fun archivedFolderKey(
    accountId: String,
    threadKey: String,
): String = "$accountId#$threadKey"

/** Where the archive put the mail, and the UIDs of the copies it made there.
 *  Recorded only when core could tell those copies apart. */
internal data class ArchivedPlacement(
    val folder: String,
    val uids: List<Long>,
)

internal fun rememberArchivedFolder(
    context: Context,
    accountId: String,
    threadKey: String,
    folder: String,
    uids: List<Long>,
) {
    val key = archivedFolderKey(accountId, threadKey)
    context
        .getSharedPreferences(ARCHIVED_FOLDER_PREFS, Context.MODE_PRIVATE)
        .edit()
        .putString(key, folder)
        .putString("$key#uids", uids.joinToString(","))
        .apply()
}

/** Reads the recorded folder and clears it: an undo is offered once, and a
 *  stale entry would let a later undo move mail that was archived by hand. */
internal fun takeArchivedFolder(
    context: Context,
    accountId: String,
    threadKey: String,
): ArchivedPlacement? {
    val prefs = context.getSharedPreferences(ARCHIVED_FOLDER_PREFS, Context.MODE_PRIVATE)
    val key = archivedFolderKey(accountId, threadKey)
    val folder = prefs.getString(key, null)?.takeIf { it.isNotBlank() }
    val uids =
        prefs
            .getString("$key#uids", null)
            .orEmpty()
            .split(',')
            .mapNotNull { it.toLongOrNull() }
    prefs
        .edit()
        .remove(key)
        .remove("$key#uids")
        .apply()
    return folder?.let { ArchivedPlacement(it, uids) }
}

/** How long the undo offer stays in the shade. Long enough to notice a mis-tap,
 *  short enough that the row does not outlive the user's memory of causing it.
 *  Past it the archive simply stands. */
const val UNDO_WINDOW_MS = 30_000L

/** The undo row stands in for the mail it replaced, so it needs an id of its
 *  own — reusing the mail's would make a later re-post of that mail silently
 *  overwrite the undo offer. */
fun undoNotificationId(notificationId: Int): Int = "undo:$notificationId".hashCode()

/** Likewise distinct, so a failure report never overwrites the mail it is
 *  reporting about. */
fun actionFailedNotificationId(notificationId: Int): Int = "failed:$notificationId".hashCode()

internal fun notificationActionIntent(
    context: Context,
    action: String,
    accountId: String,
    folder: String,
    threadKey: String,
    accountName: String,
    title: String,
    notificationId: Int,
): PendingIntent {
    val intent =
        Intent(context, AndroidNotificationActionReceiver::class.java).apply {
            this.action = action
            putExtra(AndroidNotificationService.EXTRA_ACCOUNT_ID, accountId)
            putExtra(AndroidNotificationService.EXTRA_FOLDER, folder)
            putExtra(AndroidNotificationService.EXTRA_THREAD_KEY, threadKey)
            putExtra(EXTRA_ACCOUNT_NAME, accountName)
            putExtra(EXTRA_TITLE, title)
            putExtra(EXTRA_NOTIFICATION_ID, notificationId)
        }
    return PendingIntent.getBroadcast(
        context,
        "$action|$accountId|$folder|$threadKey".hashCode(),
        intent,
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
    )
}

internal fun archiveAction(
    context: Context,
    accountId: String,
    folder: String,
    threadKey: String,
    accountName: String,
    title: String,
    notificationId: Int,
): NotificationCompat.Action =
    NotificationCompat
        .Action
        .Builder(
            R.drawable.ic_action_archive,
            context.getString(R.string.notification_action_archive),
            notificationActionIntent(
                context,
                ACTION_ARCHIVE,
                accountId,
                folder,
                threadKey,
                accountName,
                title,
                notificationId,
            ),
        ).build()

internal fun markReadAction(
    context: Context,
    accountId: String,
    folder: String,
    threadKey: String,
    accountName: String,
    title: String,
    notificationId: Int,
): NotificationCompat.Action =
    NotificationCompat
        .Action
        .Builder(
            R.drawable.ic_action_mark_read,
            context.getString(R.string.notification_action_mark_read),
            notificationActionIntent(
                context,
                ACTION_MARK_READ,
                accountId,
                folder,
                threadKey,
                accountName,
                title,
                notificationId,
            ),
        ).build()
