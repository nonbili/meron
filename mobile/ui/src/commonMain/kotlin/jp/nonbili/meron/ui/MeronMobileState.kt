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
import jp.nonbili.meron.shared.ProxySpec
import jp.nonbili.meron.shared.SignatureMark
import jp.nonbili.meron.shared.SignatureTracking
import jp.nonbili.meron.shared.StarredItemSummary
import jp.nonbili.meron.shared.StorageUsage
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.coercePollIntervalMinutes
import jp.nonbili.meron.shared.defaultOAuthRedirectUri
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.sync.Mutex
import kotlin.uuid.ExperimentalUuidApi
import kotlin.uuid.Uuid

internal data class ComposeSeed(
    // Recipients as parsed entries rather than raw text: the chips field
    // rewrites "a@b" to "a@b, " the moment it first renders, and that is the
    // field tidying itself, not the user adding anyone.
    val to: List<String> = emptyList(),
    val cc: List<String> = emptyList(),
    val bcc: List<String> = emptyList(),
    val subject: String = "",
    val attachments: List<DraftAttachment> = emptyList(),
)

internal data class ComposeDraftOwner(
    val accountId: String,
    val draftId: String,
    // The thread this draft replies in, blank for a standalone compose. A draft
    // is not always present in the loaded messages, so this is what says which
    // thread's draft marker the discard clears.
    val threadId: String = "",
)

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
    /** Write-through/hydrate bridge between the platform caches above and the
     *  authoritative `settings` table. See MobileSettings.kt. */
    val settingsMirror: SettingsMirror,
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
    var launchBackupExport: (String) -> Unit = {}
    var launchAttachmentSave: (String) -> Unit = {}
    var launchGoogleAccountPicker: () -> Unit = {}

    var host by mutableStateOf("")

    // Set once the field holds a server the user owns (typed by hand, or
    // pre-filled from the account being reconnected), so autodiscovery knows
    // not to replace it. A host it filled in itself stays replaceable.
    var hostTouched by mutableStateOf(false)
    var email by mutableStateOf("")
    var username by mutableStateOf("")
    var password by mutableStateOf("")
    var displayName by mutableStateOf("")
    var senderName by mutableStateOf("")
    var imapPort by mutableStateOf("993")
    var imapPortTouched by mutableStateOf(false)
    var imapSecurity by mutableStateOf(MailSecurity.TLS)
    var imapSecurityTouched by mutableStateOf(false)
    var smtpHost by mutableStateOf("")
    var smtpHostTouched by mutableStateOf(false)
    var smtpPort by mutableStateOf("465")
    var smtpPortTouched by mutableStateOf(false)
    var smtpSecurity by mutableStateOf(MailSecurity.TLS)
    var smtpSecurityTouched by mutableStateOf(false)
    var passwordServerSettingsOpen by mutableStateOf(false)

    // Last email we ran autodiscovery for, so the automatic (on-blur) lookup
    // doesn't repeat for an unchanged address.
    var lastAutodiscoverEmail by mutableStateOf("")
    var passwordAutodiscoverGeneration = 0

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

    // Identifies the query whose results are currently in coreThreads. This is
    // deliberately separate from mailSearch, which changes while the user is
    // still editing an unsubmitted query.
    var visibleMailboxKey by mutableStateOf<MailboxCacheKey?>(null)
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

    // Set while the "empty trash/junk" confirmation is on screen; the delete only
    // runs if the user confirms.
    var pendingEmptyFolder by mutableStateOf<EmptyFolderTarget?>(null)
    var pendingDeleteFolder by mutableStateOf<DeleteFolderTarget?>(null)
    var selectedMailCopyThread by mutableStateOf<ThreadSummary?>(null)

    // False until the first inbox load (cache or server) settles, so the list can
    // show a loading indicator instead of an empty state on cold start.
    var initialThreadsLoaded by mutableStateOf(false)
    var initialAccountsLoaded by mutableStateOf(false)
    var mailboxCursor by mutableStateOf("")
    var mailboxAccountCursors by mutableStateOf(emptyMap<String, String>())

    // Per-account header depth the visible mailbox holds: one page until the user
    // pages further. Reloads re-request it so a background sync event can't shrink
    // the list under a scrolled reader.
    var mailboxPageDepth by mutableStateOf(MAILBOX_PAGE_SIZE)

    // In-flight debounce for event-driven mailbox reloads.
    var mailboxReloadJob: Job? = null
    var mailListScrollToTopRequest by mutableStateOf(0L)
    var loadingMoreThreads by mutableStateOf(false)
    var selectedCoreThread by mutableStateOf<ThreadSummary?>(null)
    var activeThreadReadToken by mutableStateOf(0L)
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
    var composeDraftCleanupOwners = emptyList<ComposeDraftOwner>()

    // True while a send round-trip is running; gates re-entry (a second Send
    // tap must not submit the message twice) and pauses draft autosaves so a
    // save landing mid-send can't resurrect the just-discarded draft.
    var composeSendInFlight by mutableStateOf(false)

    // Owns all asynchronous work started for one full-composer session. Opening
    // another compose invalidates completions from the previous one.
    var composeSessionGeneration = 0
    var composeIdentityGeneration = 0
    var composeSignaturePending by mutableStateOf(false)
    val composeSaveMutex = Mutex()

    var composeInReplyTo by mutableStateOf("")
    var composeReferences by mutableStateOf("")

    // The quoted original of a forward, as HTML, plus the inline images it
    // references. The composer itself edits plain text; these ride alongside so
    // the send path can rebuild the HTML alternative. Both are empty for every
    // other kind of draft, which stays plain-text-only.
    var composeForwardHtml by mutableStateOf("")
    var composeForwardInlineAttachments by mutableStateOf(emptyList<DraftAttachment>())
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

    // Send-as address explicitly chosen in the reply bar's From row. Blank means
    // "auto" — fall back to the alias the original was delivered to
    // (detectReplyFromIdentity). Cleared on thread switch, so an override never
    // leaks into the next conversation.
    var quickReplyFrom by mutableStateOf("")

    // The signature this app seeded into the reply bar, or null when there is
    // none to account for — the account sends none, or the body was hydrated
    // from a saved draft that already carries its own.
    //
    // Unlike [composeSignature] this needs no third "inserted nothing, but
    // managed" state: a quick reply cannot change sending account, since its
    // From row only offers aliases of the one account and those share a
    // signature. With nothing to swap, the only question ever asked of this is
    // which part of the box is the user's.
    var quickReplySignature by mutableStateOf<SignatureMark?>(null)

    // Same double-send/autosave-race gate as composeSendInFlight, for the
    // inline reply bar.
    var quickReplySendInFlight by mutableStateOf(false)
    var quickReplyGeneration = 0
    val quickReplySaveMutex = Mutex()

    // Debounce bookkeeping for autosaving the quick-reply draft as the user
    // types; not UI state, so a plain var rather than mutableStateOf.
    var quickReplyAutosaveJob: Job? = null

    // Server drafts a quick reply has consumed by sending, held while that send
    // settles. The copy on the server outlives the send until its discard
    // returns, and reopening the conversation in that window would otherwise
    // hydrate the just-sent text straight back into the reply bar. Normalized
    // draft ids; dropped once the send lifecycle finishes, so a discard that
    // genuinely failed leaves its draft reachable again. Not UI state, so a
    // plain set rather than mutableStateOf.
    val quickReplyConsumedDraftIds = mutableSetOf<String>()

    /** Thread id and draft id an autosave landed for a send that was waiting on
     * the save lock, where the reply bar had already moved on and could not
     * carry the id across. Consumed by that send when it takes the lock. */
    var quickReplySendDraftHandover: Pair<String, String>? = null
    var status by mutableStateOf("")
    var syncing by mutableStateOf(false)
    var showUnreadBadges by mutableStateOf(loadAppBoolean(prefs, SHOW_UNREAD_BADGES_PREF, true))
    var showUnifiedInboxNav by mutableStateOf(loadAppBoolean(prefs, SHOW_UNIFIED_INBOX_PREF, true))
    var showSenderImages by mutableStateOf(loadAppBoolean(prefs, SHOW_SENDER_IMAGES_PREF, false))
    var liveMailPushEnabled by mutableStateOf(loadAppBoolean(prefs, LIVE_MAIL_PUSH_PREF, false))
    var backgroundSyncEnabled by mutableStateOf(loadAppBoolean(prefs, BACKGROUND_SYNC_ENABLED_PREF, true))
    var pollIntervalMinutes by mutableStateOf(
        coercePollIntervalMinutes(loadAppInt(prefs, POLL_INTERVAL_MINUTES_PREF, 15)),
    )
    var sendShortcutMode by mutableStateOf(loadSendShortcutMode(prefs))
    var conversationLayout by mutableStateOf(loadConversationLayout(prefs))
    var messageFontScale by mutableStateOf(loadMessageFontScale(prefs))
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
            if (value == Screen.Mail || value == Screen.Kanban) {
                saveLastTopScreen(prefs, value)
            }
        }
    var errorBanner by mutableStateOf<String?>(null)
    var syncError by mutableStateOf<MobileSyncError?>(null)

    // The certificate a server presented that we could not validate, once the
    // user has asked to see it. Non-null puts the trust dialog on screen.
    var certPrompt by mutableStateOf<MobileCertPrompt?>(null)
    var certPromptBusy by mutableStateOf(false)

    // The exact operation refused by a certificate. A prompt snapshots this so
    // unrelated failures cannot change what accepting that prompt resumes.
    var pendingCertificateRetry: PendingCertificateRetry? = null

    // The message a refused send was carrying, kept so the retry sends exactly
    // that one rather than whatever the composer holds by then.
    var pendingComposeSend: PendingComposeSend? = null
    var pendingQuickReplySend: PendingQuickReplySend? = null
    var addSection by mutableStateOf(0)
    var notificationPermissionGranted by mutableStateOf(mobileHost.notificationsEnabled())
    var notificationBannerDismissed by mutableStateOf(loadAppBoolean(prefs, NOTIFICATION_BANNER_DISMISSED_PREF, false))
    var accountsLoading by mutableStateOf(false)
    var mailboxMenuOpen by mutableStateOf(false)
    var mailSelectionMenuOpen by mutableStateOf(false)
    var accountSettingsTargetId by mutableStateOf<String?>(null)
    var accountSettingsProxyTargetId by mutableStateOf<String?>(null)
    var settingsGeneralTarget by mutableStateOf(false)
    var showAddFeedDialog by mutableStateOf(false)
    var addFeedUrl by mutableStateOf("")
    var addFeedError by mutableStateOf("")
    var addFeedSubmitting by mutableStateOf(false)
    var rssAccountAdding by mutableStateOf(false)
    var showAboutDialog by mutableStateOf(false)
    var showChangelogDialog by mutableStateOf(false)

    // Summary of a crash from the previous run, read once at startup; blank
    // when the last run ended cleanly. Clearing it hides the report prompt.
    var pendingCrashReport by mutableStateOf(mobileHost.pendingCrashReport())
    var pendingOpmlExport by mutableStateOf("")

    // Backup / restore (see MeronMobileState+Backup.kt). The export document
    // waits here for the save-file picker; the restore document waits here only
    // when the file turned out to be encrypted, so the retry after the
    // passphrase prompt doesn't send the user back to the file picker.
    var pendingBackupExport by mutableStateOf("")
    var pendingBackupRestore by mutableStateOf("")
    var backupPassphraseMode by mutableStateOf<BackupPassphraseMode?>(null)
    var backupPassphraseError by mutableStateOf("")
    var backupBusy by mutableStateOf(false)
    var backupRestoreGeneration = 0
    var accountLoadGeneration = 0
    var proxyLoadGeneration = 0
    var accountMediaUploadTarget by mutableStateOf<AccountMediaUploadTarget?>(null)
    var kanbanBoardMediaTarget by mutableStateOf<KanbanBoardMediaTarget?>(null)

    // App-wide proxy. Unlike the other preferences it lives in the core store,
    // not platform prefs: the socket layer reads it, including from background
    // syncs that never build this state.
    var appProxy by mutableStateOf(ProxySpec.off)

    // App-wide signature HTML, in the same core `settings` row desktop writes,
    // so a restored backup carries it across platforms. Accounts can override it
    // (see AccountSummary.signature).
    var appSignatureHtml by mutableStateOf("")

    // Senders whose remote content always loads, whatever an account's own
    // "load remote images" toggle says. Lives in the same core `settings` row
    // desktop writes, and the core applies it when it bakes a message body —
    // this copy is what the reader shows and edits.
    var remoteImageSenders by mutableStateOf<List<String>>(emptyList())

    // Bumped by each allowlist read *and* by each write that lands, so neither a
    // slow earlier read nor the startup read can answer over a fresher value —
    // a reload (a backup restore) or an edit made while the read was in flight.
    var remoteImageSendersLoadGeneration = 0

    // The allowlist is stored as one whole-list row, so two edits racing would
    // each write a list the other's change is missing from. Every write takes
    // this first and re-reads the row inside it, making the pair one step.
    val remoteImageSendersWrites = Mutex()

    // What the compose body knows about the signature in it, so changing the From
    // identity can swap it for the new account's. Null means unmanaged — a body
    // this app did not compose (see SignatureTracking).
    var composeSignature by mutableStateOf<SignatureTracking>(null)

    // The recipients, subject and attachments the composer was opened with. A
    // reply arrives with To and Subject already filled in, and a forward with the
    // message attached, so "did the user write anything" is measured against this
    // rather than against empty fields.
    var composeSeed by mutableStateOf(ComposeSeed())

    // Whether the app-wide signature row has been read back from the core.
    // A compose opened before it lands (a cold-start `mailto:` link) would
    // otherwise record "this account has no signature" and never revisit it.
    var appSignatureLoaded by mutableStateOf(false)

    // Bumped by each app-signature read, so a slow earlier one cannot answer
    // after a reload (a backup restore) has asked for a fresher value.
    var appSignatureLoadGeneration = 0
    var appSignatureLoadCompletion = CompletableDeferred<Unit>()
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
