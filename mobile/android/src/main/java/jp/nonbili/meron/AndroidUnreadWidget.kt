package jp.nonbili.meron

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Build
import android.text.format.DateUtils
import android.util.Log
import android.widget.RemoteViews
import androidx.core.widget.RemoteViewsCompat
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.ParsedThreadId
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.coreErrorMessage
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseFolderListResponse
import jp.nonbili.meron.shared.parseNotificationThreadId
import jp.nonbili.meron.shared.parseThreadListPage
import jp.nonbili.meron.shared.threadIdIsRss
import org.json.JSONObject
import java.util.concurrent.Executors

/** Home-screen widget showing how much inbox mail is unread. */
class AndroidUnreadWidgetProvider : AppWidgetProvider() {
    override fun onUpdate(
        context: Context,
        appWidgetManager: AppWidgetManager,
        appWidgetIds: IntArray,
    ) {
        AndroidUnreadWidget.refreshAsync(context, goAsync())
    }

    override fun onDeleted(
        context: Context,
        appWidgetIds: IntArray,
    ) {
        AndroidUnreadWidget.forget(context, appWidgetIds)
    }
}

/** Home-screen widget listing the unread inbox mail itself. */
class AndroidUnreadListWidgetProvider : AppWidgetProvider() {
    override fun onUpdate(
        context: Context,
        appWidgetManager: AppWidgetManager,
        appWidgetIds: IntArray,
    ) {
        AndroidUnreadWidget.refreshAsync(context, goAsync())
    }

    override fun onDeleted(
        context: Context,
        appWidgetIds: IntArray,
    ) {
        AndroidUnreadWidget.forget(context, appWidgetIds)
    }
}

/** Both widgets share their settings (opacity, account) and their refresh:
 *  every trigger recounts whichever of them are placed. */
object AndroidUnreadWidget {
    private const val TAG = "MeronWidget"
    private const val PREFS = "meron_widget"
    const val DEFAULT_OPACITY = 85

    /** Account scope covering every account included in the unified inbox. */
    const val UNIFIED = "unified"

    /** Rows the list widget holds; a home screen shows only a handful. */
    private const val LIST_LIMIT = 25

    /** One thread so concurrent triggers (a sync finishing as the app stops)
     *  queue instead of racing to paint stale counts over fresh ones. */
    private val executor = Executors.newSingleThreadExecutor()

    fun opacity(
        context: Context,
        appWidgetId: Int,
    ): Int = prefs(context).getInt(opacityKey(appWidgetId), DEFAULT_OPACITY)

    /** [UNIFIED], or the id of the one account the widget follows. */
    fun accountScope(
        context: Context,
        appWidgetId: Int,
    ): String = prefs(context).getString(accountKey(appWidgetId), null) ?: UNIFIED

    fun save(
        context: Context,
        appWidgetId: Int,
        opacityPercent: Int,
        accountScope: String,
    ) {
        prefs(context)
            .edit()
            .putInt(opacityKey(appWidgetId), opacityPercent.coerceIn(0, 100))
            .putString(accountKey(appWidgetId), accountScope)
            .apply()
    }

    fun forget(
        context: Context,
        appWidgetIds: IntArray,
    ) {
        prefs(context)
            .edit()
            .apply {
                appWidgetIds.forEach {
                    remove(opacityKey(it))
                    remove(accountKey(it))
                    remove(lastCountKey(it))
                }
            }.apply()
    }

    /** Recounts off the calling thread. Every place that changes unread state
     *  outside the widgets calls this; it is a no-op when none is placed. */
    fun refreshAsync(
        context: Context,
        pending: BroadcastReceiver.PendingResult? = null,
    ) {
        val app = context.applicationContext
        executor.execute {
            try {
                refresh(app)
            } catch (error: RuntimeException) {
                Log.w(TAG, "widget refresh failed: ${error.message}")
            } finally {
                pending?.finish()
            }
        }
    }

    /** Accounts the configure screen offers, read from the local store. */
    fun listAccounts(context: Context): List<AccountSummary> {
        if (!ensureCore(context)) return emptyList()
        val response = MeronCoreNative.invokeJson(requestJson(1, "account.list"))
        if (coreErrorMessage(response) != null) return emptyList()
        return parseAccountListResponse(response)
    }

    private fun refresh(context: Context) {
        val manager = AppWidgetManager.getInstance(context)
        val countIds = manager.getAppWidgetIds(ComponentName(context, AndroidUnreadWidgetProvider::class.java))
        val listIds = manager.getAppWidgetIds(ComponentName(context, AndroidUnreadListWidgetProvider::class.java))
        if (countIds.isEmpty() && listIds.isEmpty()) return
        val snapshot = if (ensureCore(context)) StoreSnapshot.read() else null

        countIds.forEach { id ->
            // A failed read keeps the last count rather than flashing zero.
            val count =
                snapshot?.unreadCount(accountScope(context, id))
                    ?: prefs(context).getInt(lastCountKey(id), 0)
            prefs(context).edit().putInt(lastCountKey(id), count).apply()
            manager.updateAppWidget(id, countViews(context, id, count))
        }

        // A list can't be painted from a remembered count; leave it as it was.
        if (snapshot == null) return
        listIds.forEach { id ->
            val scope = accountScope(context, id)
            val threads = snapshot.unreadThreads(scope) ?: return@forEach
            val views = listViews(context, id, snapshot.title(context, scope), snapshot.unreadCount(scope) ?: 0)
            RemoteViewsCompat.setRemoteAdapter(
                context,
                views,
                id,
                R.id.widget_list,
                RemoteViewsCompat.RemoteCollectionItems
                    .Builder()
                    .setHasStableIds(true)
                    .setViewTypeCount(1)
                    .apply { threads.forEach { addItem(it.id.hashCode().toLong(), rowViews(context, it)) } }
                    .build(),
            )
            manager.updateAppWidget(id, views)
        }
    }

    /** The local store as one refresh sees it; each scope is read at most once
     *  however many widgets share it. No network, so this is cheap enough to
     *  run on every change. */
    private class StoreSnapshot(
        private val accounts: List<AccountSummary>,
    ) {
        /** Per account: its folder rows, or null when the read failed. */
        private val folders = HashMap<String, List<FolderSummary>?>()
        private val threads = HashMap<String, List<ThreadSummary>?>()

        /** Accounts a scope covers. An account removed since the widget was
         *  set up falls back to unified rather than showing nothing forever. */
        private fun accountsIn(scope: String): List<AccountSummary> = accounts.filter { it.id == scope }.ifEmpty { accounts.filter { it.includedInUnified } }

        private fun single(scope: String): AccountSummary? = accounts.firstOrNull { it.id == scope }

        /** Null only for a failed read. An account that has not synced yet
         *  reads fine but has no folder rows, so no inbox: that is zero unread,
         *  not a reason to freeze every widget on its old count. */
        private fun foldersOf(accountId: String): List<FolderSummary>? {
            if (accountId in folders) return folders[accountId]
            val response =
                MeronCoreNative.invokeJson(
                    requestJson(2, "mail.folderList", JSONObject().put("account_id", accountId)),
                )
            return (if (coreErrorMessage(response) != null) null else parseFolderListResponse(response))
                .also { folders[accountId] = it }
        }

        fun unreadCount(scope: String): Int? =
            accountsIn(scope).sumOf { account ->
                val folders = foldersOf(account.id) ?: return null
                widgetInbox(folders)?.unread ?: 0
            }

        fun unreadThreads(scope: String): List<ThreadSummary>? {
            val account = single(scope)
            val key = account?.id ?: UNIFIED
            if (key in threads) return threads[key]
            return run {
                val params =
                    JSONObject()
                        .put("filter", "unread")
                        .put("refresh", false)
                        .put("limit", LIST_LIMIT)
                if (account == null) {
                    params.put("account_id", UNIFIED).put("folder_id", "inbox").put("folder_role", "inbox")
                } else {
                    val folders = foldersOf(account.id) ?: return@run null
                    val inbox = widgetInbox(folders) ?: return@run emptyList()
                    params.put("account_id", account.id).put("folder_id", inbox.name)
                }
                val response = MeronCoreNative.invokeJson(requestJson(3, "mail.threadList", params))
                if (coreErrorMessage(response) != null) {
                    null
                } else {
                    parseThreadListPage(response)
                        .threads
                        .sortedByDescending { it.dateEpochSeconds }
                        .take(LIST_LIMIT)
                }
            }.also { threads[key] = it }
        }

        fun title(
            context: Context,
            scope: String,
        ): String =
            single(scope)?.let { it.displayName.ifBlank { it.email } }
                ?: context.getString(R.string.mobile_android_widget_name)

        companion object {
            fun read(): StoreSnapshot? {
                val response = MeronCoreNative.invokeJson(requestJson(1, "account.list"))
                if (coreErrorMessage(response) != null) return null
                return StoreSnapshot(parseAccountListResponse(response))
            }
        }
    }

    private fun countViews(
        base: Context,
        appWidgetId: Int,
        count: Int,
    ): RemoteViews {
        val context = localizedAppContext(base)
        return RemoteViews(context.packageName, R.layout.unread_widget).apply {
            setTextViewText(R.id.widget_count, widgetCountLabel(count))
            setInt(R.id.widget_background, "setImageAlpha", opacityAlpha(context, appWidgetId))
            setContentDescription(
                R.id.widget_root,
                "${context.getString(R.string.mobile_android_widget_name)}: $count",
            )
            setOnClickPendingIntent(R.id.widget_root, AndroidNotificationService.openAppIntent(context))
        }
    }

    private fun listViews(
        base: Context,
        appWidgetId: Int,
        title: String,
        count: Int,
    ): RemoteViews {
        val context = localizedAppContext(base)
        return RemoteViews(context.packageName, R.layout.unread_list_widget).apply {
            setInt(R.id.widget_background, "setImageAlpha", opacityAlpha(context, appWidgetId))
            setTextViewText(R.id.widget_title, title)
            setTextViewText(R.id.widget_count, widgetCountLabel(count))
            setTextViewText(R.id.widget_list_empty, context.getString(R.string.mobile_android_widget_list_empty))
            setEmptyView(R.id.widget_list, R.id.widget_list_empty)
            setOnClickPendingIntent(R.id.widget_header, AndroidNotificationService.openAppIntent(context))
            setPendingIntentTemplate(R.id.widget_list, rowIntentTemplate(context))
        }
    }

    private fun rowViews(
        context: Context,
        thread: ThreadSummary,
    ): RemoteViews =
        RemoteViews(context.packageName, R.layout.unread_list_widget_item).apply {
            setTextViewText(R.id.widget_item_sender, thread.sender)
            setTextViewText(R.id.widget_item_subject, thread.subject)
            setTextViewText(R.id.widget_item_date, rowDate(context, thread.dateEpochSeconds))
            // Keep feed taps opening the app, as before the shared parser gained RSS support.
            val target = widgetThreadTarget(thread.id)
            setOnClickFillInIntent(
                R.id.widget_item_root,
                Intent().apply {
                    if (target != null) {
                        putExtra(AndroidNotificationService.EXTRA_ACCOUNT_ID, target.accountId)
                        putExtra(AndroidNotificationService.EXTRA_FOLDER, target.folder)
                        putExtra(AndroidNotificationService.EXTRA_THREAD_KEY, target.threadKey)
                    }
                },
            )
        }

    /** Rows fill this in with their thread, so it must stay mutable. */
    private fun rowIntentTemplate(context: Context): PendingIntent =
        PendingIntent.getActivity(
            context,
            "unread-list-widget".hashCode(),
            Intent(context, ComposeMainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP
            },
            PendingIntent.FLAG_UPDATE_CURRENT or
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0,
        )

    private fun rowDate(
        context: Context,
        epochSeconds: Long,
    ): String {
        if (epochSeconds <= 0) return ""
        val millis = epochSeconds * 1000
        val flags =
            if (DateUtils.isToday(millis)) {
                DateUtils.FORMAT_SHOW_TIME
            } else {
                DateUtils.FORMAT_SHOW_DATE or DateUtils.FORMAT_ABBREV_MONTH
            }
        return DateUtils.formatDateTime(context, millis, flags)
    }

    private fun ensureCore(context: Context): Boolean {
        if (!MeronCoreNative.isLoaded()) return false
        MeronCoreNative.initJson(context.filesDir.absolutePath, MeronDbKey.get(context))
        return true
    }

    private fun opacityAlpha(
        context: Context,
        appWidgetId: Int,
    ): Int = opacity(context, appWidgetId) * 255 / 100

    private fun prefs(context: Context) = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    private fun opacityKey(appWidgetId: Int) = "opacity_$appWidgetId"

    private fun accountKey(appWidgetId: Int) = "account_$appWidgetId"

    private fun lastCountKey(appWidgetId: Int) = "last_count_$appWidgetId"

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
}

/** An account's inbox — the mail that raises new-mail notifications, matching
 *  the desktop tray. INBOX is matched by name too, for stores that predate
 *  folder roles. */
internal fun widgetInbox(folders: List<FolderSummary>): FolderSummary? = folders.firstOrNull { it.role == "inbox" || it.name.equals("INBOX", ignoreCase = true) }

/** Fits a 1x1 cell: four digits would crowd the icon. */
internal fun widgetCountLabel(count: Int): String = if (count > 999) "999+" else count.coerceAtLeast(0).toString()

/** Feed rows preserve their app-open behavior even though the shared parser supports RSS. */
internal fun widgetThreadTarget(threadId: String): ParsedThreadId? = threadId.takeUnless(::threadIdIsRss)?.let(::parseNotificationThreadId)
