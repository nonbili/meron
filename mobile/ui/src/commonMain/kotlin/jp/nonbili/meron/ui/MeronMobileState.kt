package jp.nonbili.meron.ui

import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.ChangelogRelease
import jp.nonbili.meron.shared.ContactSuggestion
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.StarredItemSummary
import jp.nonbili.meron.shared.StorageUsage
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.coercePollIntervalMinutes
import jp.nonbili.meron.shared.defaultOAuthRedirectUri
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlin.uuid.ExperimentalUuidApi
import kotlin.uuid.Uuid

@OptIn(ExperimentalUuidApi::class)
internal class MeronMobileState(
    val scope: CoroutineScope,
    val core: MeronCore,
    val coreLoaded: Boolean,
    val prefs: AppPreferences,
    val kanbanPrefs: AppPreferences,
    val services: PlatformServices,
    val locale: LocaleController,
    val mobileHost: MobileHost,
) {
    val snackbarHost = SnackbarHostState()

    /** Error for actions that need the native core when it failed to load;
     *  includes the host's load diagnostics (ABIs, missing split APK, ...) so a
     *  screenshot of the message is enough to diagnose the install. */
    val coreUnavailableMessage: String
        get() {
            val detail = mobileHost.coreLoadDiagnostics.trim()
            return if (detail.isEmpty()) "Rust core not packaged." else "Rust core not packaged. $detail"
        }

    // Wired by the composable after its platform pickers/launchers are created.
    var launchOpmlExport: (String) -> Unit = {}
    var launchAttachmentSave: (String) -> Unit = {}
    var launchGoogleAccountPicker: () -> Unit = {}

    var host by mutableStateOf("")
    var email by mutableStateOf("")
    var username by mutableStateOf("")
    var password by mutableStateOf("")
    var displayName by mutableStateOf("")
    var senderName by mutableStateOf("")
    var imapPort by mutableStateOf("993")
    var smtpHost by mutableStateOf("")
    var smtpPort by mutableStateOf("465")
    var passwordServerSettingsOpen by mutableStateOf(false)

    // Last email we ran autodiscovery for, so the automatic (on-blur) lookup
    // doesn't repeat for an unchanged address.
    var lastAutodiscoverEmail by mutableStateOf("")

    // Set to a managed account id when its on-device Google token can no longer
    // be silently refreshed, signalling the user must reconnect it.
    var googleReauthAccountId by mutableStateOf<String?>(null)
    var oauthProvider by mutableStateOf("gmail")
    var oauthEmail by mutableStateOf("")
    var oauthAccessToken by mutableStateOf("")
    var oauthRefreshToken by mutableStateOf("")
    var oauthExpiresAt by mutableStateOf("0")
    var oauthRedirectUri by mutableStateOf(defaultOAuthRedirectUri())
    var oauthState by mutableStateOf(Uuid.random().toString())
    var oauthVerifier by mutableStateOf(Uuid.random().toString() + Uuid.random().toString())
    var oauthAuthorizationCode by mutableStateOf("")
    var rssFeedUrl by mutableStateOf("")
    var rssDisplayName by mutableStateOf("")
    var accountJson by mutableStateOf("")
    var coreAccounts by mutableStateOf(emptyList<AccountSummary>())
    var selectedCoreAccountId by mutableStateOf(loadLastMailAccountId(prefs))
    var coreFolders by mutableStateOf(emptyList<FolderSummary>())
    var foldersByAccount by mutableStateOf(emptyMap<String, List<FolderSummary>>())
    var selectedCoreFolder by mutableStateOf(loadLastMailFolder(prefs))
    var mailSearch by mutableStateOf("")
    var mailFilter by mutableStateOf(FilterMode.All)
    var coreThreads by mutableStateOf(emptyList<ThreadSummary>())
    var mailboxCache by mutableStateOf(emptyMap<MailboxCacheKey, MailboxLoadResult>())
    var activeMailboxLoadKey by mutableStateOf<MailboxCacheKey?>(null)
    var activeMailboxLoadToken by mutableStateOf(0L)
    var activeMailboxLoadStartedAtMillis by mutableStateOf(0L)
    var blockingMailboxLoadWarned by mutableStateOf(false)

    // True once a blocking inbox load has outlived the soft timeout with the
    // sync still in flight — switches the loader to "still syncing" copy
    // instead of surfacing a timeout error for a slow-but-healthy first sync.
    var blockingMailboxLoadSlow by mutableStateOf(false)
    var selectedMailThreadIds by mutableStateOf(emptySet<String>())
    var selectedMailMoveThread by mutableStateOf<ThreadSummary?>(null)
    var selectedMailCopyThread by mutableStateOf<ThreadSummary?>(null)

    // False until the first inbox load (cache or server) settles, so the list can
    // show a loading indicator instead of an empty state on cold start.
    var initialThreadsLoaded by mutableStateOf(false)
    var initialAccountsLoaded by mutableStateOf(false)
    var mailboxCursor by mutableStateOf("")
    var mailboxAccountCursors by mutableStateOf(emptyMap<String, String>())
    var mailListScrollToTopRequest by mutableStateOf(0L)
    var loadingMoreThreads by mutableStateOf(false)
    var starredItems by mutableStateOf(emptyList<StarredItemSummary>())
    var selectedCoreThread by mutableStateOf<ThreadSummary?>(null)
    var conversationHtmlOverrides by mutableStateOf(emptyMap<String, Boolean>())
    var previousTopScreen by mutableStateOf(Screen.Mail)
    var composeReturnScreen by mutableStateOf(Screen.Mail)

    // Parse saved boards as-is here; the default board (seeded with the user's
    // accounts) is created later in applyAccounts once accounts are available, so a
    // fresh install gets per-account columns without re-seeding on every restart.
    var kanbanBoards by mutableStateOf(parseKanbanBoards(kanbanPrefs.getString(KANBAN_BOARDS_PREF, "")))
    var activeKanbanBoardId by mutableStateOf(loadActiveKanbanBoardId(kanbanPrefs))
    var kanbanColumns by mutableStateOf(emptyMap<String, KanbanColumnState>())
    var kanbanFilter by mutableStateOf(loadKanbanFilter(kanbanPrefs))
    var kanbanSearch by mutableStateOf(loadKanbanSearch(kanbanPrefs))
    var kanbanSearchScope by mutableStateOf(loadKanbanSearchScope(kanbanPrefs))
    var kanbanActionThread by mutableStateOf<ThreadSummary?>(null)
    var kanbanSettingsTargetId by mutableStateOf<String?>(null)
    var kanbanMenuOpen by mutableStateOf(false)
    var showKanbanColumnDialog by mutableStateOf(false)
    var showKanbanCreateFolderDialog by mutableStateOf<AccountSummary?>(null)
    var kanbanFolderNameInput by mutableStateOf("")
    var to by mutableStateOf("")
    var composeFromAccountId by mutableStateOf("")
    var composeFromEmail by mutableStateOf("")
    var composeDraftId by mutableStateOf("")
    var composeDraftSaved by mutableStateOf(false)

    // Account the draft was last saved under. Discards must target this
    // account: the user can switch the From identity to another account after
    // an autosave, and discarding under the new account would orphan the copy
    // on the old one.
    var composeDraftAccountId by mutableStateOf("")

    // True while a send round-trip is running; gates re-entry (a second Send
    // tap must not submit the message twice) and pauses draft autosaves so a
    // save landing mid-send can't resurrect the just-discarded draft.
    var composeSendInFlight by mutableStateOf(false)
    var composeInReplyTo by mutableStateOf("")
    var composeReferences by mutableStateOf("")
    var locallyDraftedThreadIds by mutableStateOf(emptySet<String>())
    var locallyDiscardedThreadIds by mutableStateOf(emptySet<String>())
    var subject by mutableStateOf("")
    var body by mutableStateOf("")
    var quickReplyBody by mutableStateOf("")
    var quickReplyAttachments by mutableStateOf(emptyList<DraftAttachment>())
    var quickReplyFailure by mutableStateOf("")
    var quickReplyDraftId by mutableStateOf("")
    var quickReplyDraftSaved by mutableStateOf(false)
    var quickReplyInReplyTo by mutableStateOf("")
    var quickReplyReferences by mutableStateOf("")
    var quickReplyThreadId by mutableStateOf("")

    // Same double-send/autosave-race gate as composeSendInFlight, for the
    // inline reply bar.
    var quickReplySendInFlight by mutableStateOf(false)

    // Debounce bookkeeping for autosaving the quick-reply draft as the user
    // types; not UI state, so a plain var rather than mutableStateOf.
    var quickReplyAutosaveJob: Job? = null
    var status by mutableStateOf("")
    var syncing by mutableStateOf(false)
    var showUnreadBadges by mutableStateOf(loadAppBoolean(prefs, SHOW_UNREAD_BADGES_PREF, true))
    var showUnifiedInboxNav by mutableStateOf(loadAppBoolean(prefs, SHOW_UNIFIED_INBOX_PREF, true))
    var showStarredNav by mutableStateOf(loadAppBoolean(prefs, SHOW_STARRED_NAV_PREF, false))
    var showSenderImages by mutableStateOf(loadAppBoolean(prefs, SHOW_SENDER_IMAGES_PREF, false))
    var liveMailPushEnabled by mutableStateOf(loadAppBoolean(prefs, LIVE_MAIL_PUSH_PREF, false))
    var backgroundSyncEnabled by mutableStateOf(loadAppBoolean(prefs, BACKGROUND_SYNC_ENABLED_PREF, true))
    var pollIntervalMinutes by mutableStateOf(
        coercePollIntervalMinutes(loadAppInt(prefs, POLL_INTERVAL_MINUTES_PREF, 15)),
    )
    var sendShortcutMode by mutableStateOf(loadSendShortcutMode(prefs))
    var kanbanColumnWidth by mutableStateOf(
        loadAppInt(prefs, KANBAN_COLUMN_WIDTH_PREF, KANBAN_COLUMN_DEFAULT_WIDTH)
            .coerceIn(KANBAN_COLUMN_MIN_WIDTH, KANBAN_COLUMN_MAX_WIDTH),
    )
    var hiddenNavigationAccountIds by mutableStateOf(loadAppStringSet(prefs, HIDDEN_NAV_ACCOUNTS_PREF))
    var messages by mutableStateOf(emptyList<MessageBody>())
    var messageCursor by mutableStateOf("")
    var loadingMoreMessages by mutableStateOf(false)
    var attachments by mutableStateOf(emptyList<DraftAttachment>())
    var cc by mutableStateOf("")
    var bcc by mutableStateOf("")
    var recipientSuggestionField by mutableStateOf("")
    var recipientSuggestions by mutableStateOf(emptyList<ContactSuggestion>())

    // Restore the last top-level screen on cold start; persist whenever the user
    // navigates to a top-level screen so a restart returns to the same place.
    private var screenState by mutableStateOf(loadLastTopScreen(prefs))
    var screen: Screen
        get() = screenState
        set(value) {
            screenState = value
            if (value == Screen.Mail || value == Screen.Starred || value == Screen.Kanban) {
                saveLastTopScreen(prefs, value)
            }
        }
    var errorBanner by mutableStateOf<String?>(null)
    var addSection by mutableStateOf(0)
    var notificationPermissionGranted by mutableStateOf(mobileHost.notificationsEnabled())
    var notificationBannerDismissed by mutableStateOf(loadAppBoolean(prefs, NOTIFICATION_BANNER_DISMISSED_PREF, false))
    var accountsLoading by mutableStateOf(false)
    var mailboxMenuOpen by mutableStateOf(false)
    var mailSelectionMenuOpen by mutableStateOf(false)
    var accountSettingsTargetId by mutableStateOf<String?>(null)
    var showAddFeedDialog by mutableStateOf(false)
    var addFeedUrl by mutableStateOf("")
    var addFeedError by mutableStateOf("")
    var addFeedSubmitting by mutableStateOf(false)
    var rssAccountAdding by mutableStateOf(false)
    var showAboutDialog by mutableStateOf(false)
    var showChangelogDialog by mutableStateOf(false)
    var pendingOpmlExport by mutableStateOf("")
    var accountMediaUploadTarget by mutableStateOf<AccountMediaUploadTarget?>(null)
    var kanbanBoardMediaTarget by mutableStateOf<KanbanBoardMediaTarget?>(null)
    var storageUsage by mutableStateOf<StorageUsage?>(null)
    var storageBusy by mutableStateOf(false)

    // In-app changelog: null while loading/unloaded, populated on success.
    var changelog by mutableStateOf<List<ChangelogRelease>?>(null)
    var changelogLoading by mutableStateOf(false)
    var changelogError by mutableStateOf(false)
    var storageClearConfirming by mutableStateOf(false)
    var imagePreview by mutableStateOf<ImagePreview?>(null)
    var pendingAttachmentSave by mutableStateOf<MessageAttachment?>(null)
}
