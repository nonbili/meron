package jp.nonbili.meron.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
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
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.input.OutputTransformation
import androidx.compose.foundation.text.input.TextFieldBuffer
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.focus.FocusState
import androidx.compose.ui.window.PopupProperties
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Reply
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.ContentPaste
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
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.LeadingIconTab
import androidx.compose.material3.SecondaryTabRow
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
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
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
import androidx.compose.ui.platform.LocalClipboardManager
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
import jp.nonbili.meron.ui.resources.Res
import jp.nonbili.meron.ui.resources.ic_google_g
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.jetbrains.compose.resources.DrawableResource
import org.jetbrains.compose.resources.painterResource
import kotlin.math.abs
import kotlin.math.max

private const val COMPOSE_AUTOSAVE_DELAY_MS = 3_000L

internal data class ComposeAutosaveSnapshot(
    val selectedFromKey: String,
    val to: String,
    val cc: String,
    val bcc: String,
    val subject: String,
    val body: String,
    val attachments: List<DraftAttachment>,
)

internal fun composeAutosaveSnapshot(
    selectedFromKey: String,
    to: String,
    cc: String,
    bcc: String,
    subject: String,
    body: String,
    attachments: List<DraftAttachment>,
): ComposeAutosaveSnapshot? {
    val hasContent =
        to.isNotBlank() || cc.isNotBlank() || bcc.isNotBlank() ||
            subject.isNotBlank() || body.isNotBlank() || attachments.isNotEmpty()
    if (!hasContent) return null
    return ComposeAutosaveSnapshot(
        selectedFromKey = selectedFromKey,
        to = to,
        cc = cc,
        bcc = bcc,
        subject = subject,
        body = body,
        attachments = attachments,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun ComposeScreen(
    sendIdentities: List<SendIdentity>,
    selectedFromKey: String,
    onFromChange: (String) -> Unit,
    to: String,
    onToChange: (String) -> Unit,
    cc: String,
    onCcChange: (String) -> Unit,
    bcc: String,
    onBccChange: (String) -> Unit,
    subject: String,
    onSubjectChange: (String) -> Unit,
    body: String,
    onBodyChange: (String) -> Unit,
    attachments: List<DraftAttachment>,
    recipientSuggestionField: String,
    recipientSuggestions: List<ContactSuggestion>,
    onRecipientFocus: (field: String, value: String) -> Unit,
    onAcceptRecipientSuggestion: (field: String, contact: ContactSuggestion) -> Unit,
    onAttach: () -> Unit,
    onClearAttachments: () -> Unit,
    onRemoveAttachment: (DraftAttachment) -> Unit,
    sendShortcutMode: SendShortcutMode,
    onSaveDraft: () -> Unit,
    onAutoSaveDraft: () -> Unit,
    onDiscardDraft: () -> Unit,
    onSend: () -> Unit,
    onBack: () -> Unit,
) {
    var confirmDiscard by remember { mutableStateOf(false) }
    var overflowOpen by remember { mutableStateOf(false) }
    var showCcBcc by remember { mutableStateOf(cc.isNotBlank() || bcc.isNotBlank()) }
    var maxLabelWidth by remember { mutableStateOf(0.dp) }
    val hasContent =
        to.isNotBlank() || cc.isNotBlank() || bcc.isNotBlank() ||
            subject.isNotBlank() || body.isNotBlank() || attachments.isNotEmpty()
    val autosaveSnapshot =
        remember(selectedFromKey, to, cc, bcc, subject, body, attachments) {
            composeAutosaveSnapshot(
                selectedFromKey = selectedFromKey,
                to = to,
                cc = cc,
                bcc = bcc,
                subject = subject,
                body = body,
                attachments = attachments,
            )
        }
    var autosaveInitialized by remember { mutableStateOf(false) }
    var lastAutosavedSnapshot by remember { mutableStateOf<ComposeAutosaveSnapshot?>(null) }

    LaunchedEffect(autosaveSnapshot) {
        if (!autosaveInitialized) {
            autosaveInitialized = true
            lastAutosavedSnapshot = autosaveSnapshot
            return@LaunchedEffect
        }
        val snapshot = autosaveSnapshot ?: return@LaunchedEffect
        if (snapshot == lastAutosavedSnapshot) return@LaunchedEffect
        delay(COMPOSE_AUTOSAVE_DELAY_MS)
        if (snapshot != lastAutosavedSnapshot) {
            onAutoSaveDraft()
            lastAutosavedSnapshot = snapshot
        }
    }

    if (confirmDiscard) {
        AlertDialog(
            onDismissRequest = { confirmDiscard = false },
            title = { Text(tr("mobile.compose.discardDraftTitle")) },
            text = { Text(tr("mobile.compose.discardDraftText")) },
            confirmButton = {
                TextButton(onClick = {
                    confirmDiscard = false
                    onDiscardDraft()
                }) {
                    Text(tr("buttons.discard"), color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmDiscard = false }) { Text(tr("mobile.compose.keepEditing")) }
            },
        )
    }

    Scaffold(
        containerColor = MaterialTheme.colorScheme.surface,
        topBar = {
            TopAppBar(
                title = { Text(tr("composer.actions.newMessage")) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.Filled.Close, contentDescription = tr("buttons.discard"))
                    }
                },
                actions = {
                    IconButton(onClick = onAttach) {
                        Icon(Icons.Filled.AttachFile, contentDescription = tr("composer.actions.attachFiles"))
                    }
                    val sendEnabled = to.isNotBlank()
                    IconButton(onClick = onSend, enabled = sendEnabled) {
                        Icon(
                            Icons.AutoMirrored.Filled.Send,
                            contentDescription = tr("buttons.send"),
                            tint = if (sendEnabled) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface.copy(alpha = 0.38f),
                        )
                    }
                    Box {
                        IconButton(onClick = { overflowOpen = true }) {
                            Icon(Icons.Filled.MoreVert, contentDescription = tr("common.more"))
                        }
                        DropdownMenu(expanded = overflowOpen, onDismissRequest = { overflowOpen = false }) {
                            DropdownMenuItem(
                                text = { Text(tr("composer.actions.saveDraft")) },
                                leadingIcon = { Icon(Icons.Outlined.Drafts, contentDescription = null) },
                                onClick = {
                                    overflowOpen = false
                                    onSaveDraft()
                                },
                            )
                            DropdownMenuItem(
                                text = { Text(tr("chat.actions.discardDraft"), color = MaterialTheme.colorScheme.error) },
                                leadingIcon = { Icon(Icons.Filled.Delete, contentDescription = null, tint = MaterialTheme.colorScheme.error) },
                                onClick = {
                                    overflowOpen = false
                                    if (hasContent) confirmDiscard = true else onDiscardDraft()
                                },
                            )
                        }
                    }
                },
            )
        },
    ) { innerPadding ->
        BoxWithConstraints(
            Modifier
                .fillMaxSize()
                .background(MaterialTheme.colorScheme.surface)
                .padding(innerPadding)
                .consumeWindowInsets(innerPadding)
                .imePadding(),
        ) {
            val optionalFieldsHeight = (if (sendIdentities.size > 1) 56 else 0) + (if (showCcBcc) 128 else 0)
            val attachmentHeight = attachments.size * 36 + if (attachments.isNotEmpty()) 44 else 0
            val bodyMinHeight =
                max(
                    280f,
                    maxHeight.value - optionalFieldsHeight - attachmentHeight - 220f,
                ).dp
            Column(
                Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState()),
            ) {
                if (sendIdentities.size > 1) {
                    FromIdentitySelector(
                        identities = sendIdentities,
                        selectedKey = selectedFromKey,
                        onSelect = onFromChange,
                        labelWidth = maxLabelWidth,
                        onLabelWidthChanged = { maxLabelWidth = it },
                    )
                }
                ComposeField(
                    value = to,
                    onChange = onToChange,
                    label = tr("composer.fields.to"),
                    field = "to",
                    suggestions = if (recipientSuggestionField == "to") recipientSuggestions else emptyList(),
                    onFocus = onRecipientFocus,
                    onAcceptSuggestion = onAcceptRecipientSuggestion,
                    placeholder = tr("composer.placeholders.recipient"),
                    trailingContent = if (!showCcBcc) {
                        {
                            Icon(
                                imageVector = Icons.Filled.ArrowDropDown,
                                contentDescription = tr("composer.actions.ccBcc"),
                                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier
                                    .padding(end = 12.dp)
                                    .clickable { showCcBcc = true }
                            )
                        }
                    } else null,
                    labelWidth = maxLabelWidth,
                    onLabelWidthChanged = { maxLabelWidth = it },
                )
                if (showCcBcc) {
                    ComposeField(
                        value = cc,
                        onChange = onCcChange,
                        label = tr("composer.fields.cc"),
                        field = "cc",
                        suggestions = if (recipientSuggestionField == "cc") recipientSuggestions else emptyList(),
                        onFocus = onRecipientFocus,
                        onAcceptSuggestion = onAcceptRecipientSuggestion,
                        placeholder = tr("composer.placeholders.commaSeparated"),
                        labelWidth = maxLabelWidth,
                        onLabelWidthChanged = { maxLabelWidth = it },
                    )
                    ComposeField(
                        value = bcc,
                        onChange = onBccChange,
                        label = tr("composer.fields.bcc"),
                        field = "bcc",
                        suggestions = if (recipientSuggestionField == "bcc") recipientSuggestions else emptyList(),
                        onFocus = onRecipientFocus,
                        onAcceptSuggestion = onAcceptRecipientSuggestion,
                        placeholder = tr("composer.placeholders.commaSeparated"),
                        labelWidth = maxLabelWidth,
                        onLabelWidthChanged = { maxLabelWidth = it },
                    )
                }
                ComposeField(
                    value = subject,
                    onChange = onSubjectChange,
                    label = tr("composer.fields.subject"),
                    placeholder = tr("composer.fields.subject"),
                    labelWidth = maxLabelWidth,
                    onLabelWidthChanged = { maxLabelWidth = it },
                )
                TextField(
                    value = body,
                    onValueChange = onBodyChange,
                    placeholder = { Text(tr("composer.placeholders.message"), style = MaterialTheme.typography.bodyMedium) },
                    keyboardOptions = nativeTextKeyboardOptions,
                    colors = TextFieldDefaults.colors(
                        focusedContainerColor = Color.Transparent,
                        unfocusedContainerColor = Color.Transparent,
                        disabledContainerColor = Color.Transparent,
                        focusedIndicatorColor = Color.Transparent,
                        unfocusedIndicatorColor = Color.Transparent,
                        disabledIndicatorColor = Color.Transparent,
                    ),
                    textStyle = MaterialTheme.typography.bodyMedium,
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .heightIn(min = bodyMinHeight)
                            .onPreviewKeyEvent { event ->
                                if (shouldSendFromEditor(event, sendShortcutMode)) {
                                    onSend()
                                    true
                                } else {
                                    false
                                }
                            },
                )
                if (attachments.isNotEmpty()) {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp, vertical = 8.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        attachments.forEach { attachment ->
                            Row(
                                modifier = Modifier.fillMaxWidth().heightIn(min = 36.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(8.dp)
                            ) {
                                Icon(
                                    imageVector = Icons.Filled.AttachFile,
                                    contentDescription = null,
                                    modifier = Modifier.size(18.dp),
                                    tint = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                                Text(
                                    text = "${attachment.displayName} · ${formatBytes(attachment.sizeBytes)}",
                                    style = MaterialTheme.typography.bodyMedium,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                    modifier = Modifier.weight(1f),
                                )
                                IconButton(
                                    onClick = { onRemoveAttachment(attachment) },
                                    modifier = Modifier.size(24.dp)
                                ) {
                                    Icon(
                                        imageVector = Icons.Filled.Close,
                                        contentDescription = tr("composer.actions.removeAttachment"),
                                        modifier = Modifier.size(16.dp),
                                        tint = MaterialTheme.colorScheme.onSurfaceVariant
                                    )
                                }
                            }
                        }
                        TextButton(
                            onClick = onClearAttachments,
                            modifier = Modifier.align(Alignment.Start)
                        ) {
                            Text(tr("mobile.compose.clearAttachments"))
                        }
                    }
                }
            }
        }
    }
}

internal fun parseRecipients(value: String): Pair<List<String>, String> {
    if (value.isEmpty()) return Pair(emptyList(), "")
    val parts = value.split(",")
    if (parts.size <= 1) {
        return Pair(emptyList(), value)
    }
    val completed = parts.subList(0, parts.size - 1)
        .map { it.trim() }
        .filter { it.isNotEmpty() }
    val activeInput = parts.last().trimStart()
    return Pair(completed, activeInput)
}

private fun getAvatarColor(identifier: String): Color {
    val colors = listOf(
        Color(0xFFC2185B),
        Color(0xFF7B1FA2),
        Color(0xFF512DA8),
        Color(0xFF303F9F),
        Color(0xFF1976D2),
        Color(0xFF00796B),
        Color(0xFF388E3C),
        Color(0xFFF57C00),
        Color(0xFF5D4037),
        Color(0xFF455A64)
    )
    val hash = identifier.hashCode()
    val index = abs(hash) % colors.size
    return colors[index]
}

internal fun parseEmailRecipient(recipient: String): Pair<String, String> {
    val trimmed = recipient.trim()
    if (trimmed.contains("<") && trimmed.endsWith(">")) {
        val name = trimmed.substringBefore("<").trim()
        val email = trimmed.substringAfter("<").removeSuffix(">").trim()
        return Pair(name, email)
    }
    if (trimmed.contains("(") && trimmed.endsWith(")")) {
        val email = trimmed.substringBefore("(").trim()
        val name = trimmed.substringAfter("(").removeSuffix(")").trim()
        return Pair(name, email)
    }
    return Pair("", trimmed)
}

@Composable
internal fun RecipientChip(
    recipient: String,
    onDelete: () -> Unit,
) {
    val (name, email) = remember(recipient) { parseEmailRecipient(recipient) }
    val displayName = if (name.isNotEmpty()) name else email
    val initials = if (displayName.isNotEmpty()) displayName.take(1).uppercase() else "?"
    var menuExpanded by remember { mutableStateOf(false) }
    val clipboardManager = LocalClipboardManager.current

    Box {
        Surface(
            shape = RoundedCornerShape(16.dp),
            color = MaterialTheme.colorScheme.surface,
            border = androidx.compose.foundation.BorderStroke(1.dp, MaterialTheme.colorScheme.outline.copy(alpha = 0.5f)),
            modifier = Modifier
                .height(32.dp)
                .clickable { menuExpanded = true }
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.padding(end = 12.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Box(
                    modifier = Modifier
                        .size(32.dp)
                        .background(getAvatarColor(email), CircleShape),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = initials,
                        color = Color.White,
                        fontWeight = FontWeight.Bold,
                        fontSize = 13.sp
                    )
                }
                Text(
                    text = displayName,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
            }
        }

        DropdownMenu(
            expanded = menuExpanded,
            onDismissRequest = { menuExpanded = false },
            modifier = Modifier
                .width(280.dp)
                .background(MaterialTheme.colorScheme.surfaceContainerHigh)
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = 8.dp)
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp)
                ) {
                    Box(
                        modifier = Modifier
                            .size(40.dp)
                            .background(getAvatarColor(email), CircleShape),
                        contentAlignment = Alignment.Center
                    ) {
                        Text(
                            text = initials,
                            color = Color.White,
                            fontWeight = FontWeight.Bold,
                            fontSize = 16.sp
                        )
                    }
                    Column {
                        if (name.isNotEmpty()) {
                            Text(
                                text = name,
                                style = MaterialTheme.typography.bodyLarge,
                                fontWeight = FontWeight.Bold,
                                color = MaterialTheme.colorScheme.onSurface,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis
                            )
                        }
                        Text(
                            text = email,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis
                        )
                    }
                }

                HorizontalDivider(
                    color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
                    thickness = 0.5.dp,
                    modifier = Modifier.padding(vertical = 4.dp)
                )

                DropdownMenuItem(
                    text = {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(12.dp)
                        ) {
                            Icon(
                                imageVector = Icons.Filled.ContentCopy,
                                contentDescription = null,
                                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.size(20.dp)
                            )
                            Text(
                                text = tr("common.copy"),
                                style = MaterialTheme.typography.bodyMedium
                            )
                        }
                    },
                    onClick = {
                        clipboardManager.setText(AnnotatedString(email))
                        menuExpanded = false
                    }
                )

                HorizontalDivider(
                    color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
                    thickness = 0.5.dp,
                    modifier = Modifier.padding(vertical = 4.dp)
                )

                DropdownMenuItem(
                    text = {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(12.dp)
                        ) {
                            Icon(
                                imageVector = Icons.Filled.Delete,
                                contentDescription = null,
                                tint = MaterialTheme.colorScheme.error,
                                modifier = Modifier.size(20.dp)
                            )
                            Text(
                                text = tr("buttons.delete"),
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.error
                            )
                        }
                    },
                    onClick = {
                        onDelete()
                        menuExpanded = false
                    }
                )
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun RecipientChipsInput(
    value: String,
    onChange: (String) -> Unit,
    placeholder: String,
    keyboardOptions: KeyboardOptions,
    onFocusChanged: (FocusState) -> Unit,
    isFocused: Boolean,
) {
    val (completed, activeInput) = remember(value) { parseRecipients(value) }
    var initialized by remember { mutableStateOf(false) }

    // Maintain local TextFieldValue state for correct cursor positioning
    var activeInputState by remember { mutableStateOf(TextFieldValue("")) }

    // Keep activeInputState in sync with parent activeInput changes (e.g. initial reply, suggestion click, or backspace deletion)
    LaunchedEffect(activeInput) {
        if (activeInput != activeInputState.text) {
            activeInputState = TextFieldValue(
                text = activeInput,
                selection = TextRange(activeInput.length)
            )
        }
    }

    LaunchedEffect(value) {
        if (!initialized) {
            initialized = true
            if (value.isNotEmpty() && !value.trim().endsWith(",")) {
                onChange(value.trim() + ", ")
            }
        }
    }

    FlowRow(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        completed.forEach { recipient ->
            RecipientChip(
                recipient = recipient,
                onDelete = {
                    val newCompleted = completed.filter { it != recipient }
                    val newValue = if (newCompleted.isEmpty()) {
                        activeInput
                    } else {
                        newCompleted.joinToString(", ") + (if (activeInput.isNotEmpty()) ", $activeInput" else "")
                    }
                    onChange(newValue)
                }
            )
        }
        
        BasicTextField(
            value = activeInputState,
            onValueChange = { newActiveState ->
                // Update local state immediately to preserve cursor position
                activeInputState = newActiveState

                val newActiveText = newActiveState.text
                val committedValue = if (newActiveText.endsWith(",")) {
                    val cleaned = newActiveText.removeSuffix(",").trim()
                    if (cleaned.isNotEmpty()) {
                        (completed + cleaned).joinToString(", ") + ", "
                    } else {
                        value
                    }
                } else {
                    if (completed.isEmpty()) {
                        newActiveText
                    } else {
                        completed.joinToString(", ") + ", " + newActiveText
                    }
                }
                onChange(committedValue)
            },
            keyboardOptions = keyboardOptions,
            textStyle = MaterialTheme.typography.bodyMedium.copy(color = MaterialTheme.colorScheme.onSurface),
            modifier = Modifier
                .widthIn(min = 120.dp)
                .align(Alignment.CenterVertically)
                .onFocusChanged { focusState ->
                    if (!focusState.isFocused && activeInput.trim().isNotEmpty()) {
                        onChange((completed + activeInput.trim()).joinToString(", ") + ", ")
                    }
                    onFocusChanged(focusState)
                },
            decorationBox = { innerTextField ->
                Box(
                    modifier = Modifier.padding(vertical = 6.dp),
                    contentAlignment = Alignment.CenterStart
                ) {
                    if (activeInput.isEmpty() && completed.isEmpty() && placeholder.isNotEmpty()) {
                        Text(
                            text = placeholder,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f)
                        )
                    }
                    innerTextField()
                }
            }
        )
    }
}

@Composable
internal fun FromIdentitySelector(
    identities: List<SendIdentity>,
    selectedKey: String,
    onSelect: (String) -> Unit,
    labelWidth: Dp = 0.dp,
    onLabelWidthChanged: (Dp) -> Unit = {},
) {
    var expanded by remember { mutableStateOf(false) }
    val selected = identities.firstOrNull { identityKey(it) == selectedKey } ?: identities.firstOrNull()
    val density = LocalDensity.current
    Box {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(min = 48.dp)
                .clickable { expanded = true }
                .padding(horizontal = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                text = tr("composer.fields.from"),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier
                    .onGloballyPositioned { layoutCoordinates ->
                        val widthDp = with(density) { layoutCoordinates.size.width.toDp() }
                        if (widthDp > labelWidth) {
                            onLabelWidthChanged(widthDp)
                        }
                    }
                    .then(
                        if (labelWidth > 0.dp) Modifier.width(labelWidth) else Modifier.wrapContentWidth()
                    ),
            )
            Box(modifier = Modifier.weight(1f)) {
                Text(
                    text = selected?.let { formatSendIdentity(it) } ?: tr("mobile.compose.selectSender"),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier.fillMaxWidth(),
                )
                DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
                    identities.forEach { identity ->
                        DropdownMenuItem(
                            text = {
                                Text(
                                    formatSendIdentity(identity),
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            },
                            onClick = {
                                onSelect(identityKey(identity))
                                expanded = false
                            },
                        )
                    }
                }
            }
            Icon(
                imageVector = Icons.Filled.ArrowDropDown,
                contentDescription = tr("common.more"),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(end = 12.dp)
            )
        }
    }
    HorizontalDivider(
        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.6f),
        thickness = 0.5.dp,
    )
}

@Composable
internal fun ComposeField(
    value: String,
    onChange: (String) -> Unit,
    label: String,
    field: String = "",
    suggestions: List<ContactSuggestion> = emptyList(),
    onFocus: (field: String, value: String) -> Unit = { _, _ -> },
    onAcceptSuggestion: (field: String, contact: ContactSuggestion) -> Unit = { _, _ -> },
    modifier: Modifier = Modifier,
    placeholder: String = "",
    autoFocus: Boolean = false,
    trailingContent: @Composable (RowScope.() -> Unit)? = null,
    labelWidth: Dp = 0.dp,
    onLabelWidthChanged: (Dp) -> Unit = {},
) {
    var isFocused by remember { mutableStateOf(false) }
    val isRecipientField = field == "to" || field == "cc" || field == "bcc"
    val density = LocalDensity.current

    Column(modifier) {
        Row(
            modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(horizontal = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                text = label,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier
                    .onGloballyPositioned { layoutCoordinates ->
                        val widthDp = with(density) { layoutCoordinates.size.width.toDp() }
                        if (widthDp > labelWidth) {
                            onLabelWidthChanged(widthDp)
                        }
                    }
                    .then(
                        if (labelWidth > 0.dp) Modifier.width(labelWidth) else Modifier.wrapContentWidth()
                    ),
            )

            Box(modifier = Modifier.weight(1f)) {
                if (isRecipientField) {
                    RecipientChipsInput(
                        value = value,
                        onChange = onChange,
                        placeholder = placeholder,
                        keyboardOptions = nativeTextKeyboardOptions,
                        onFocusChanged = {
                            isFocused = it.isFocused
                            if (it.isFocused && field.isNotBlank()) onFocus(field, value)
                        },
                        isFocused = isFocused,
                    )
                } else {
                    BasicTextField(
                        value = value,
                        onValueChange = onChange,
                        keyboardOptions = nativeTextKeyboardOptions,
                        textStyle = MaterialTheme.typography.bodyMedium.copy(color = MaterialTheme.colorScheme.onSurface),
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .onFocusChanged {
                                    isFocused = it.isFocused
                                    if (it.isFocused && field.isNotBlank()) onFocus(field, value)
                                },
                        decorationBox = { innerTextField ->
                            Box(
                                modifier = Modifier.padding(vertical = 12.dp),
                                contentAlignment = Alignment.CenterStart
                            ) {
                                if (value.isEmpty() && placeholder.isNotEmpty()) {
                                    Text(
                                        text = placeholder,
                                        style = MaterialTheme.typography.bodyMedium,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f)
                                    )
                                }
                                innerTextField()
                            }
                        }
                    )
                }
            }

            if (value.isEmpty() && !isRecipientField) {
                val clipboardManager = LocalClipboardManager.current
                IconButton(
                    onClick = {
                        val text = clipboardManager.getText()?.text.orEmpty()
                        if (text.isNotEmpty()) {
                            onChange(text)
                        }
                    },
                ) {
                    Icon(
                        Icons.Filled.ContentPaste,
                        contentDescription = "Paste",
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }

            if (trailingContent != null) {
                trailingContent()
            }
        }

        HorizontalDivider(
            color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.6f),
            thickness = 0.5.dp,
        )

        val activeInputText = if (isRecipientField) parseRecipients(value).second else value
        val showSuggestions = isFocused && suggestions.isNotEmpty() && activeInputText.trim().isNotEmpty()

        if (showSuggestions) {
            Surface(
                color = MaterialTheme.colorScheme.surfaceContainerLow,
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(
                    modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp)
                ) {
                    suggestions.forEach { contact ->
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { onAcceptSuggestion(field, contact) }
                                .padding(horizontal = 24.dp, vertical = 12.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(16.dp)
                        ) {
                            Box(
                                modifier = Modifier
                                    .size(36.dp)
                                    .background(getAvatarColor(contact.addr), CircleShape),
                                contentAlignment = Alignment.Center
                            ) {
                                val initials = if (contact.name.isNotEmpty()) contact.name.take(1).uppercase() else contact.addr.take(1).uppercase()
                                Text(
                                    text = initials,
                                    color = Color.White,
                                    fontWeight = FontWeight.Bold,
                                    fontSize = 14.sp
                                )
                            }
                            Column {
                                if (contact.name.isNotEmpty()) {
                                    Text(
                                        text = contact.name,
                                        style = MaterialTheme.typography.bodyMedium,
                                        fontWeight = FontWeight.Bold,
                                        color = MaterialTheme.colorScheme.onSurface
                                    )
                                }
                                Text(
                                    text = contact.addr,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }
                        }
                    }
                }
            }
            HorizontalDivider(
                color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.6f),
                thickness = 0.5.dp,
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun AddAccountScreen(
    onBack: () -> Unit,
    initialSection: Int,
    displayName: String,
    onDisplayNameChange: (String) -> Unit,
    senderName: String,
    onSenderNameChange: (String) -> Unit,
    email: String,
    onEmailChange: (String) -> Unit,
    username: String,
    onUsernameChange: (String) -> Unit,
    password: String,
    onPasswordChange: (String) -> Unit,
    host: String,
    onHostChange: (String) -> Unit,
    imapPort: String,
    onImapPortChange: (String) -> Unit,
    smtpHost: String,
    onSmtpHostChange: (String) -> Unit,
    smtpPort: String,
    onSmtpPortChange: (String) -> Unit,
    serverSettingsOpen: Boolean,
    onServerSettingsOpenChange: (Boolean) -> Unit,
    onAutodiscover: () -> Unit,
    onEmailBlur: () -> Unit,
    onAddPassword: () -> Unit,
    oauthAuthorizationCode: String,
    onLaunchOAuth: () -> Unit,
    onConnectGoogleDeviceAccount: () -> Unit,
    rssFeedUrl: String,
    onRssFeedUrlChange: (String) -> Unit,
    rssDisplayName: String,
    onRssDisplayNameChange: (String) -> Unit,
    onAddRss: () -> Unit,
) {
    val pagerState = rememberPagerState(initialPage = initialSection) { 3 }
    val coroutineScope = rememberCoroutineScope()
    val tabs = listOf(
        tr("accounts.setup.oauthTab"),
        tr("accounts.setup.passwordTab"),
        tr("accounts.setup.rssTab"),
    )
    val icons = listOf(
        Icons.Default.PersonAdd,
        Icons.Default.Inbox,
        Icons.Default.RssFeed,
    )

    LaunchedEffect(initialSection) {
        pagerState.scrollToPage(initialSection)
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(tr("accounts.actions.addAccount")) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = tr("buttons.back"))
                    }
                },
            )
        },
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .consumeWindowInsets(innerPadding)
        ) {
            SecondaryTabRow(
                selectedTabIndex = pagerState.currentPage,
                modifier = Modifier.fillMaxWidth()
            ) {
                tabs.forEachIndexed { index, label ->
                    LeadingIconTab(
                        selected = pagerState.currentPage == index,
                        onClick = {
                            coroutineScope.launch {
                                pagerState.animateScrollToPage(index)
                            }
                        },
                        text = { Text(label, maxLines = 1) },
                        icon = { Icon(icons[index], contentDescription = label) },
                        selectedContentColor = MaterialTheme.colorScheme.primary,
                        unselectedContentColor = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }

            HorizontalPager(
                state = pagerState,
                modifier = Modifier
                    .fillMaxSize()
                    .weight(1f)
                    .imePadding()
            ) { page ->
                LazyColumn(
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(16.dp),
                    verticalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    when (page) {
                        0 -> {
                            item {
                                SetupCard(title = "") {
                                    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                                        OAuthSignInButton(
                                            label = trf("accounts.oauth.signInWithProvider", tr("accounts.providers.googleName")),
                                            provider = "google",
                                            onClick = onConnectGoogleDeviceAccount,
                                        )
                                        OAuthSignInButton(
                                            label = trf("accounts.oauth.signInWithProvider", tr("accounts.providers.outlookName")),
                                            provider = "outlook",
                                            onClick = onLaunchOAuth,
                                        )
                                    }
                                    if (oauthAuthorizationCode.isNotBlank()) {
                                        Text(
                                            tr("accounts.oauth.finishingSignIn"),
                                            style = MaterialTheme.typography.bodySmall,
                                            color = MaterialTheme.colorScheme.primary,
                                        )
                                    }
                                }
                            }
                        }

                        1 -> {
                            item {
                                SetupCard(title = "") {
                                    SetupField(displayName, onDisplayNameChange, tr("accounts.fields.displayNameMeronOnly"))
                                    SetupField(senderName, onSenderNameChange, tr("accounts.fields.senderNameOutgoing"))
                                    SetupField(email, onEmailChange, tr("accounts.fields.emailAddress"), onFocusLost = onEmailBlur)
                                    SetupField(password, onPasswordChange, tr("accounts.fields.password"), isPassword = true)
                                    OutlinedButton(onClick = onAutodiscover, modifier = Modifier.fillMaxWidth()) {
                                        Text(tr("mobile.accounts.findMailSettings"))
                                    }
                                    TextButton(
                                        onClick = { onServerSettingsOpenChange(!serverSettingsOpen) },
                                        modifier = Modifier.align(Alignment.Start),
                                    ) {
                                        Text(tr("accounts.advancedServerSettings"))
                                    }
                                    if (serverSettingsOpen) {
                                        SetupField(username, onUsernameChange, tr("accounts.fields.username"))
                                        SetupField(host, onHostChange, tr("accounts.fields.imapHost"))
                                        SetupField(imapPort, onImapPortChange, tr("accounts.fields.imapPort"))
                                        SetupField(smtpHost, onSmtpHostChange, tr("accounts.fields.smtpHost"))
                                        SetupField(smtpPort, onSmtpPortChange, tr("accounts.fields.smtpPort"))
                                    }
                                    Button(onClick = onAddPassword, modifier = Modifier.fillMaxWidth()) { Text(tr("accounts.actions.addAccount")) }
                                }
                            }
                        }

                        else -> {
                            item {
                                SetupCard(title = "") {
                                    SetupField(rssDisplayName, onRssDisplayNameChange, tr("accounts.fields.accountName"))
                                    SetupField(rssFeedUrl, onRssFeedUrlChange, tr("accounts.fields.firstFeedUrl"))
                                    Text(
                                        tr("accounts.setup.feedAccountHint"),
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                    Button(
                                        onClick = onAddRss,
                                        enabled = rssDisplayName.isNotBlank(),
                                        modifier = Modifier.fillMaxWidth(),
                                    ) { Text(tr("accounts.actions.saveAccount")) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
internal fun OAuthSignInButton(
    label: String,
    provider: String,
    onClick: () -> Unit,
) {
    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(14.dp),
        color = MaterialTheme.colorScheme.surface,
        border = androidx.compose.foundation.BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
        onClick = onClick,
    ) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 18.dp, vertical = 13.dp),
            horizontalArrangement = Arrangement.Center,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OAuthBrandIcon(provider = provider, size = 20.dp)
            Spacer(Modifier.width(12.dp))
            Text(label, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
        }
    }
}

@Composable
internal fun OAuthBrandIcon(
    provider: String,
    size: Dp,
) {
    if (provider == "outlook") {
        Column(Modifier.size(size), verticalArrangement = Arrangement.spacedBy(1.dp)) {
            Row(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(1.dp)) {
                Box(Modifier.weight(1f).fillMaxSize().background(Color(0xFFF25022)))
                Box(Modifier.weight(1f).fillMaxSize().background(Color(0xFF7FBA00)))
            }
            Row(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(1.dp)) {
                Box(Modifier.weight(1f).fillMaxSize().background(Color(0xFF00A4EF)))
                Box(Modifier.weight(1f).fillMaxSize().background(Color(0xFFFFB900)))
            }
        }
    } else {
        Image(
            painter = painterResource(Res.drawable.ic_google_g),
            contentDescription = null,
            modifier = Modifier.size(size),
        )
    }
}

@Composable
internal fun SetupCard(
    title: String,
    content: @Composable ColumnScope.() -> Unit,
) {
    Card(Modifier.fillMaxWidth(), colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            if (title.isNotBlank()) {
                Text(title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
            }
            content()
        }
    }
}

@Composable
internal fun SetupField(
    value: String,
    onChange: (String) -> Unit,
    label: String,
    isPassword: Boolean = false,
    placeholder: String? = null,
    onFocusLost: (() -> Unit)? = null,
) {
    // Compose Multiplatform on iOS doesn't show the long-press paste menu while a
    // PasswordVisualTransformation is active, so reveal the password by default
    // there; the eye toggle still lets the user hide it. Android keeps it masked.
    var revealed by remember { mutableStateOf(isPassword && maskPasswordsByDefault.not()) }
    var wasFocused by remember { mutableStateOf(false) }

    // Use TextFieldState so that nativeTextKeyboardOptions (usingNativeTextInput
    // on iOS) actually takes effect – the value/onValueChange overload ignores it.
    val textFieldState = rememberTextFieldState(initialText = value)

    // Sync external value → internal state (e.g. when the parent clears the form).
    LaunchedEffect(value) {
        if (textFieldState.text.toString() != value) {
            textFieldState.edit { replace(0, length, value) }
        }
    }

    // Propagate internal edits → external onChange callback.
    LaunchedEffect(textFieldState) {
        snapshotFlow { textFieldState.text.toString() }
            .collect { newText ->
                if (newText != value) onChange(newText)
            }
    }

    OutlinedTextField(
        state = textFieldState,
        label = { Text(label) },
        placeholder = placeholder?.let { { Text(it) } },
        lineLimits = TextFieldLineLimits.SingleLine,
        keyboardOptions = nativeTextKeyboardOptions,
        modifier =
            Modifier.fillMaxWidth().let { m ->
                if (onFocusLost != null) {
                    m.onFocusChanged { focusState ->
                        if (wasFocused && !focusState.isFocused) onFocusLost()
                        wasFocused = focusState.isFocused
                    }
                } else {
                    m
                }
            },
        outputTransformation = if (isPassword && !revealed) PasswordOutputTransformation else null,
        trailingIcon = if (isPassword || value.isEmpty()) {
            {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    val clipboardManager = LocalClipboardManager.current
                    if (value.isEmpty()) {
                        IconButton(onClick = {
                            val text = clipboardManager.getText()?.text.orEmpty()
                            if (text.isNotEmpty()) {
                                onChange(text)
                            }
                        }) {
                            Icon(
                                Icons.Filled.ContentPaste,
                                contentDescription = "Paste",
                                modifier = Modifier.size(20.dp),
                            )
                        }
                    }
                    if (isPassword) {
                        IconButton(onClick = { revealed = !revealed }) {
                            Icon(
                                if (revealed) Icons.Filled.VisibilityOff else Icons.Filled.Visibility,
                                contentDescription = if (revealed) "Hide password" else "Show password",
                                modifier = Modifier.size(20.dp),
                            )
                        }
                    }
                }
            }
        } else null
    )
}

/**
 * [OutputTransformation] that masks every character with a bullet (●),
 * equivalent to [PasswordVisualTransformation] but for the TextFieldState API.
 */
private object PasswordOutputTransformation : OutputTransformation {
    override fun TextFieldBuffer.transformOutput() {
        val masked = "●".repeat(length)
        replace(0, length, masked)
    }
}
