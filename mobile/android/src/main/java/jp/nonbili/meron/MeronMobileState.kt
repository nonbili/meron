package jp.nonbili.meron

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Base64
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Reply
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.FilterList
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.MarkEmailUnread
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.OpenInFull
import androidx.compose.material.icons.filled.PersonAdd
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.RssFeed
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.StarBorder
import androidx.compose.material.icons.filled.ViewKanban
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material.icons.outlined.Drafts
import androidx.compose.material.icons.outlined.FolderOpen
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.rememberDrawerState
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.FileProvider
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import jp.nonbili.meron.shared.AccountAliasParams
import jp.nonbili.meron.shared.AccountAliasesParams
import jp.nonbili.meron.shared.AccountAvatarParams
import jp.nonbili.meron.shared.AccountChatWallpaperParams
import jp.nonbili.meron.shared.AccountFlagParams
import jp.nonbili.meron.shared.AccountIdParams
import jp.nonbili.meron.shared.AccountMediaFileParams
import jp.nonbili.meron.shared.AccountNameParams
import jp.nonbili.meron.shared.AccountReorderParams
import jp.nonbili.meron.shared.AccountRssSyncIntervalParams
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.AddOAuthAccountParams
import jp.nonbili.meron.shared.AddPasswordAccountParams
import jp.nonbili.meron.shared.AddRssAccountParams
import jp.nonbili.meron.shared.AddRssFeedParams
import jp.nonbili.meron.shared.AttachmentReadParams
import jp.nonbili.meron.shared.AutodiscoverAccountParams
import jp.nonbili.meron.shared.ComposeDraft
import jp.nonbili.meron.shared.ContactSuggestParams
import jp.nonbili.meron.shared.ContactSuggestion
import jp.nonbili.meron.shared.CopyThreadParams
import jp.nonbili.meron.shared.DiscardDraftParams
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.ExchangeOAuthCodeParams
import jp.nonbili.meron.shared.ExportOpmlParams
import jp.nonbili.meron.shared.FolderCreateParams
import jp.nonbili.meron.shared.FolderListParams
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.ImportOpmlParams
import jp.nonbili.meron.shared.MarkAllReadParams
import jp.nonbili.meron.shared.MarkReadParams
import jp.nonbili.meron.shared.MarkStarredParams
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.MoveRssFeedParams
import jp.nonbili.meron.shared.MoveThreadParams
import jp.nonbili.meron.shared.OAuthAuthorizationRequest
import jp.nonbili.meron.shared.RemoveRssFeedParams
import jp.nonbili.meron.shared.RssMarkReadParams
import jp.nonbili.meron.shared.RssMarkStarredParams
import jp.nonbili.meron.shared.RssThreadParams
import jp.nonbili.meron.shared.SendIdentity
import jp.nonbili.meron.shared.SharedMobileContract
import jp.nonbili.meron.shared.StarredItemSummary
import jp.nonbili.meron.shared.StorageUsage
import jp.nonbili.meron.shared.SyncMailParams
import jp.nonbili.meron.shared.SyncRssParams
import jp.nonbili.meron.shared.ThreadActionParams
import jp.nonbili.meron.shared.ThreadListParams
import jp.nonbili.meron.shared.ThreadReadParams
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.accountSendIdentities
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.attachmentToDraftAttachment
import jp.nonbili.meron.shared.buildOAuthAuthorizationUrl
import jp.nonbili.meron.shared.defaultOAuthRedirectUri
import jp.nonbili.meron.shared.detectReplyFromIdentity
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.folderIsTrash
import jp.nonbili.meron.shared.formatContactSuggestion
import jp.nonbili.meron.shared.formatSendIdentity
import jp.nonbili.meron.shared.forwardableAttachments
import jp.nonbili.meron.shared.isOAuthCallbackUrl
import jp.nonbili.meron.shared.isPotentialOAuthCallbackUrl
import jp.nonbili.meron.shared.messageEditAsNewDraft
import jp.nonbili.meron.shared.messageForwardDraft
import jp.nonbili.meron.shared.newDraftMessageId
import jp.nonbili.meron.shared.ownAddressList
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseAttachmentDataResponse
import jp.nonbili.meron.shared.parseAutodiscoverResponse
import jp.nonbili.meron.shared.parseContactSuggestResponse
import jp.nonbili.meron.shared.parseFolderListResponse
import jp.nonbili.meron.shared.parseMailtoUrl
import jp.nonbili.meron.shared.parseMediaFileUrlResponse
import jp.nonbili.meron.shared.parseOAuthCallbackUrlForRedirect
import jp.nonbili.meron.shared.parseOpmlExportResponse
import jp.nonbili.meron.shared.parseOpmlImportCountResponse
import jp.nonbili.meron.shared.parseStarredItemsResponse
import jp.nonbili.meron.shared.parseStorageUsageResponse
import jp.nonbili.meron.shared.parseThreadListPage
import jp.nonbili.meron.shared.parseThreadListResponse
import jp.nonbili.meron.shared.parseThreadReadPage
import jp.nonbili.meron.shared.recipientTail
import jp.nonbili.meron.shared.replaceRecipientTail
import jp.nonbili.meron.shared.threadIdIsRss
import jp.nonbili.meron.shared.toReplyMailParams
import jp.nonbili.meron.shared.toSaveDraftParams
import jp.nonbili.meron.shared.toSendMailParams
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.net.URL
import java.security.MessageDigest
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.UUID
import kotlin.math.abs

internal class MeronMobileState(
    val context: Context,
    val scope: CoroutineScope,
) {
    val snackbarHost = SnackbarHostState()

    // Wired by the composable after its activity-result launchers are created.
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
    // Set to a managed account id when its on-device Google token can no longer
    // be silently refreshed, signalling the user must reconnect it.
    var googleReauthAccountId by mutableStateOf<String?>(null)
    var oauthProvider by mutableStateOf("gmail")
    var oauthEmail by mutableStateOf("")
    var oauthAccessToken by mutableStateOf("")
    var oauthRefreshToken by mutableStateOf("")
    var oauthExpiresAt by mutableStateOf("0")
    var oauthClientId by mutableStateOf("")
    var oauthClientSecret by mutableStateOf("")
    var oauthRedirectUri by mutableStateOf(defaultOAuthRedirectUri())
    var oauthState by mutableStateOf(UUID.randomUUID().toString())
    var oauthVerifier by mutableStateOf(UUID.randomUUID().toString() + UUID.randomUUID().toString())
    var oauthAuthorizationCode by mutableStateOf("")
    var rssFeedUrl by mutableStateOf("")
    var rssDisplayName by mutableStateOf("")
    var accountJson by mutableStateOf("")
    var coreAccounts by mutableStateOf(emptyList<AccountSummary>())
    var selectedCoreAccountId by mutableStateOf(UNIFIED_ACCOUNT_ID)
    var coreFolders by mutableStateOf(emptyList<FolderSummary>())
    var foldersByAccount by mutableStateOf(emptyMap<String, List<FolderSummary>>())
    var selectedCoreFolder by mutableStateOf(INBOX_FOLDER)
    var mailSearch by mutableStateOf("")
    var mailFilter by mutableStateOf(FilterMode.All)
    var coreThreads by mutableStateOf(emptyList<ThreadSummary>())
    var selectedMailThreadIds by mutableStateOf(emptySet<String>())
    var selectedMailMoveThread by mutableStateOf<ThreadSummary?>(null)
    var selectedMailCopyThread by mutableStateOf<ThreadSummary?>(null)

    // False until the first inbox load (cache or server) settles, so the list can
    // show a loading indicator instead of an empty state on cold start.
    var initialThreadsLoaded by mutableStateOf(false)
    var initialAccountsLoaded by mutableStateOf(false)
    var mailboxCursor by mutableStateOf("")
    var mailboxAccountCursors by mutableStateOf(emptyMap<String, String>())
    var loadingMoreThreads by mutableStateOf(false)
    var starredItems by mutableStateOf(emptyList<StarredItemSummary>())
    var selectedCoreThread by mutableStateOf<ThreadSummary?>(null)
    var conversationHtmlOverrides by mutableStateOf(emptyMap<String, Boolean>())
    var previousTopScreen by mutableStateOf(Screen.Mail)
    var kanbanBoards by mutableStateOf(loadKanbanBoards(context, emptyList()))
    var activeKanbanBoardId by mutableStateOf(loadActiveKanbanBoardId(context))
    var kanbanColumns by mutableStateOf(emptyMap<String, KanbanColumnState>())
    var kanbanFilter by mutableStateOf(loadKanbanFilter(context))
    var kanbanSearch by mutableStateOf(loadKanbanSearch(context))
    var kanbanSearchScope by mutableStateOf(loadKanbanSearchScope(context))
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
    var composeInReplyTo by mutableStateOf("")
    var composeReferences by mutableStateOf("")
    var subject by mutableStateOf("")
    var body by mutableStateOf("")
    var quickReplyBody by mutableStateOf("")
    var quickReplyAttachments by mutableStateOf(emptyList<DraftAttachment>())
    var quickReplyFailure by mutableStateOf("")
    var status by mutableStateOf("")
    var syncing by mutableStateOf(false)
    var showUnreadBadges by mutableStateOf(loadAppBoolean(context, SHOW_UNREAD_BADGES_PREF, true))
    var showUnifiedInboxNav by mutableStateOf(loadAppBoolean(context, SHOW_UNIFIED_INBOX_PREF, true))
    var showStarredNav by mutableStateOf(loadAppBoolean(context, SHOW_STARRED_NAV_PREF, true))
    var showSenderImages by mutableStateOf(loadAppBoolean(context, SHOW_SENDER_IMAGES_PREF, false))
    var sendShortcutMode by mutableStateOf(loadSendShortcutMode(context))
    var kanbanColumnWidth by mutableStateOf(
        loadAppInt(context, KANBAN_COLUMN_WIDTH_PREF, KANBAN_COLUMN_DEFAULT_WIDTH)
            .coerceIn(KANBAN_COLUMN_MIN_WIDTH, KANBAN_COLUMN_MAX_WIDTH),
    )
    var hiddenNavigationAccountIds by mutableStateOf(loadAppStringSet(context, HIDDEN_NAV_ACCOUNTS_PREF))
    var messages by mutableStateOf(emptyList<MessageBody>())
    var messageCursor by mutableStateOf("")
    var loadingMoreMessages by mutableStateOf(false)
    var attachments by mutableStateOf(emptyList<DraftAttachment>())
    var cc by mutableStateOf("")
    var bcc by mutableStateOf("")
    var recipientSuggestionField by mutableStateOf("")
    var recipientSuggestions by mutableStateOf(emptyList<ContactSuggestion>())
    var screen by mutableStateOf(Screen.Mail)
    var errorBanner by mutableStateOf<String?>(null)
    var addSection by mutableStateOf(0)
    var notificationPermissionGranted by mutableStateOf(AndroidNotificationService.canNotify(context))
    var accountsLoading by mutableStateOf(false)
    var mailboxMenuOpen by mutableStateOf(false)
    var mailSelectionMenuOpen by mutableStateOf(false)
    var accountSettingsTargetId by mutableStateOf<String?>(null)
    var showAddFeedDialog by mutableStateOf(false)
    var addFeedUrl by mutableStateOf("")
    var showAboutDialog by mutableStateOf(false)
    var pendingOpmlExport by mutableStateOf("")
    var accountMediaUploadTarget by mutableStateOf<AccountMediaUploadTarget?>(null)
    var kanbanBoardMediaTarget by mutableStateOf<KanbanBoardMediaTarget?>(null)
    var storageUsage by mutableStateOf<StorageUsage?>(null)
    var storageBusy by mutableStateOf(false)
    var storageClearConfirming by mutableStateOf(false)
    var imagePreview by mutableStateOf<AndroidImagePreview?>(null)
    var pendingAttachmentSave by mutableStateOf<MessageAttachment?>(null)
}
