package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Email
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.backhandler.BackHandler
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.ThreadMediaItem

@OptIn(ExperimentalMaterial3Api::class, ExperimentalComposeUiApi::class)
@Composable
internal fun ConversationDetailsScreen(
    subject: String,
    messages: List<MessageBody>,
    mediaItems: List<ThreadMediaItem>,
    loadImageAttachment: suspend (MessageAttachment) -> ImageBitmap?,
    isRss: Boolean,
    feedUrl: String,
    ownEmail: String,
    onBack: () -> Unit,
    onComposeTo: (String) -> Unit,
    onCopy: (String, String) -> Unit,
    onOpenAttachment: (MessageAttachment) -> Unit,
    onSaveAttachment: (MessageAttachment) -> Unit,
    onOpenGalleryIndex: (Int) -> Unit,
    onOpenUrl: (String) -> Unit,
) {
    val participants = remember(messages, isRss, ownEmail) { conversationParticipants(messages, ownEmail, isRss) }
    val attachments = remember(messages) { messages.flatMap { it.attachments }.asReversed() }
    val fileAttachments = remember(attachments) { attachments.filter { !it.mimeType.startsWith("image/") && !it.mimeType.startsWith("video/") } }
    val mediaRows = remember(mediaItems) { mediaItems.chunked(3) }
    val subjectLabel = tr("composer.fields.subject")

    BackHandler(onBack = onBack)

    val detailsListState = rememberLazyListState()
    Scaffold(
        modifier = Modifier.fillMaxSize(),
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            TopAppBar(
                colors = canvasAppBarColors(detailsListState.canScrollBackward),
                title = { Text(if (isRss) tr("chat.feedDetails") else tr("chat.conversationDetails")) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = tr("buttons.back"))
                    }
                },
            )
        },
    ) { innerPadding ->
        LazyColumn(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(innerPadding.withoutBottom())
                    .appScrollbar(detailsListState)
                    .padding(horizontal = 16.dp),
            state = detailsListState,
            // The details scroll on under the navigation bar.
            contentPadding = PaddingValues(bottom = navigationBarInset()),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            // Section 0: Subject
            item {
                Text(
                    text = subjectLabel.uppercase(),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(top = 8.dp, bottom = 4.dp),
                )
                Row(
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .padding(vertical = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Text(
                        text = subject,
                        modifier = Modifier.weight(1f),
                    )
                    IconButton(
                        onClick = { onCopy(subjectLabel, subject) },
                        modifier = Modifier.size(36.dp),
                    ) {
                        Icon(
                            imageVector = Icons.Filled.ContentCopy,
                            contentDescription = tr("chat.copySubject"),
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
                HorizontalDivider(
                    modifier = Modifier.padding(top = 12.dp),
                    color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
                )
            }

            // Section 1: Feed URL
            if (isRss && feedUrl.isNotBlank()) {
                item {
                    Text(
                        text = tr("chat.feedUrl").uppercase(),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        fontWeight = FontWeight.Bold,
                        modifier = Modifier.padding(top = 8.dp, bottom = 4.dp),
                    )
                    Row(
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(8.dp))
                                .clickable { onOpenUrl(feedUrl) }
                                .padding(vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Text(
                            text = feedUrl,
                            color = MaterialTheme.colorScheme.onSurface,
                            // Tapping opens it; without the accent, the underline says so.
                            textDecoration = androidx.compose.ui.text.style.TextDecoration.Underline,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f),
                        )
                        val feedUrlLabel = tr("chat.feedUrl")
                        IconButton(
                            onClick = { onCopy(feedUrlLabel, feedUrl) },
                            modifier = Modifier.size(36.dp),
                        ) {
                            Icon(
                                imageVector = Icons.Filled.ContentCopy,
                                contentDescription = tr("common.copy"),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    }
                    HorizontalDivider(
                        modifier = Modifier.padding(top = 12.dp),
                        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
                    )
                }
            }

            // Section 2: People
            if (participants.isNotEmpty()) {
                item {
                    Text(
                        text = tr("chat.people", mapOf("count" to participants.size)).uppercase(),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        fontWeight = FontWeight.Bold,
                        modifier = Modifier.padding(top = 8.dp, bottom = 4.dp),
                    )
                }
                items(participants, key = { it.email }) { person ->
                    Row(
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(8.dp))
                                .padding(vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Avatar(person.name.ifBlank { person.email }, 36.dp)
                        Column(Modifier.weight(1f)) {
                            Text(
                                text = person.name.ifBlank { person.email } + if (person.isSelf) " (${tr("chat.you")})" else "",
                                fontWeight = FontWeight.Medium,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            if (person.name.isNotBlank() && person.name != person.email) {
                                Text(
                                    text = person.email,
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                        }
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(4.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            val emailLabel = tr("accounts.fields.emailAddress")
                            IconButton(
                                onClick = { onCopy(emailLabel, person.email) },
                                modifier = Modifier.size(36.dp),
                            ) {
                                Icon(
                                    imageVector = Icons.Filled.ContentCopy,
                                    contentDescription = tr("chat.copyEmailAddress"),
                                    modifier = Modifier.size(18.dp),
                                )
                            }
                            if (!person.isSelf) {
                                IconButton(
                                    onClick = { onComposeTo(person.email) },
                                    modifier = Modifier.size(36.dp),
                                ) {
                                    Icon(
                                        imageVector = Icons.Filled.Email,
                                        contentDescription = tr("mobile.tabs.mail"),
                                        modifier = Modifier.size(18.dp),
                                    )
                                }
                            }
                        }
                    }
                }
            }

            // Section 3: Media
            if (mediaItems.isNotEmpty()) {
                item {
                    HorizontalDivider(
                        modifier = Modifier.padding(vertical = 12.dp),
                        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
                    )
                    Text(
                        text = tr("chat.media").uppercase(),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        fontWeight = FontWeight.Bold,
                        modifier = Modifier.padding(bottom = 4.dp),
                    )
                }
                itemsIndexed(
                    mediaRows,
                    key = { index, row -> "$index:${row.joinToString("|") { "${it.type}-${it.filename}" }}" },
                ) { _, row ->
                    Row(
                        modifier = Modifier.fillMaxWidth().padding(bottom = 6.dp),
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        row.forEach { item ->
                            Box(Modifier.weight(1f)) {
                                ConversationMediaTile(
                                    item = item,
                                    loadImageAttachment = loadImageAttachment,
                                    onOpen = {
                                        val imageIndex = item.galleryIndex
                                        if (item.type == "image" && imageIndex != null) {
                                            onOpenGalleryIndex(imageIndex)
                                        } else {
                                            onOpenAttachment(item.attachment)
                                        }
                                    },
                                )
                            }
                        }
                        repeat(3 - row.size) { Spacer(Modifier.weight(1f)) }
                    }
                }
            }

            // Section 3: Files
            if (fileAttachments.isNotEmpty()) {
                item {
                    HorizontalDivider(
                        modifier = Modifier.padding(vertical = 12.dp),
                        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
                    )
                    Text(
                        text = tr("chat.files").uppercase(),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        fontWeight = FontWeight.Bold,
                        modifier = Modifier.padding(bottom = 4.dp),
                    )
                }
                items(fileAttachments.withIndex().toList(), key = { "file-${it.index}" }) { indexed ->
                    val attachment = indexed.value
                    Row(
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(8.dp))
                                .padding(vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Icon(
                            imageVector = Icons.Filled.AttachFile,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.primary,
                            modifier = Modifier.size(24.dp),
                        )
                        Column(Modifier.weight(1f)) {
                            Text(
                                text = attachment.filename.ifBlank { tr("chat.attachment") },
                                fontWeight = FontWeight.Medium,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            Text(
                                text =
                                    listOf(
                                        attachment.mimeType,
                                        formatBytes(attachment.sizeBytes),
                                    ).filter { it.isNotBlank() }.joinToString(" · "),
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(4.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            TextButton(
                                onClick = { onOpenAttachment(attachment) },
                                contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                                modifier = Modifier.height(32.dp),
                            ) {
                                Text(tr("kanban.actions.openThread"), fontSize = 12.sp)
                            }
                            TextButton(
                                onClick = { onSaveAttachment(attachment) },
                                contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                                modifier = Modifier.height(32.dp),
                            ) {
                                Text(tr("buttons.save"), fontSize = 12.sp)
                            }
                        }
                    }
                }
            }
        }
    }
}

/**
 * The conversation's search, in the top bar in place of its actions: one
 * floating chip like theirs, holding the field, the match count and the arrows
 * between matches. It takes the focus as it opens.
 */
@Composable
internal fun ConversationSearchChip(
    query: String,
    onQueryChange: (String) -> Unit,
    matchLabel: String,
    canNavigate: Boolean,
    onPrevious: () -> Unit,
    onNext: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = MaterialTheme.colorScheme
    val focusRequester = remember { FocusRequester() }
    FloatingCard(RoundedCornerShape(50), modifier.fillMaxWidth().height(FLOATING_BAR_CHIP_HEIGHT)) {
        // The field keeps room for the query in a narrow window: the search
        // icon goes first, then the clear button, then the count, while the
        // arrows between matches always stay. Decided by width alone, so typing
        // never shifts the field.
        BoxWithConstraints {
            val showIcon = maxWidth >= 320.dp
            val showClear = maxWidth >= 280.dp
            val showCount = maxWidth >= 240.dp
            // Here, not above: this content composes during layout, and the
            // field must be attached before it can take the focus.
            LaunchedEffect(Unit) { focusRequester.requestFocus() }
            Row(Modifier.fillMaxHeight().padding(start = 16.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                if (showIcon) {
                    Icon(Icons.Filled.Search, contentDescription = null, tint = colors.onSurfaceVariant, modifier = Modifier.size(20.dp))
                }
                BasicTextField(
                    value = query,
                    onValueChange = onQueryChange,
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    keyboardActions = KeyboardActions(onSearch = { if (canNavigate) onNext() }),
                    cursorBrush = SolidColor(colors.primary),
                    textStyle = MaterialTheme.typography.bodyLarge.copy(color = colors.onSurface),
                    modifier = Modifier.weight(1f).padding(start = if (showIcon) 12.dp else 0.dp, end = 4.dp).focusRequester(focusRequester),
                    decorationBox = { field ->
                        Box(contentAlignment = Alignment.CenterStart) {
                            if (query.isEmpty()) {
                                Text(
                                    tr("chat.searchThread"),
                                    style = MaterialTheme.typography.bodyLarge,
                                    color = colors.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                            field()
                        }
                    },
                )
                // The count, the arrows and the clear button come with a query:
                // an empty search has nothing to step through.
                if (query.isNotEmpty()) {
                    if (showCount) {
                        Text(
                            matchLabel,
                            style = MaterialTheme.typography.labelMedium,
                            color = colors.onSurfaceVariant,
                            maxLines = 1,
                            modifier = Modifier.padding(horizontal = 4.dp),
                        )
                    }
                    IconButton(onClick = onPrevious, enabled = canNavigate, modifier = Modifier.size(40.dp)) {
                        Icon(Icons.Filled.KeyboardArrowUp, contentDescription = tr("chat.previousMatch"))
                    }
                    IconButton(onClick = onNext, enabled = canNavigate, modifier = Modifier.size(40.dp)) {
                        Icon(Icons.Filled.KeyboardArrowDown, contentDescription = tr("chat.nextMatch"))
                    }
                    if (showClear) {
                        IconButton(onClick = { onQueryChange("") }, modifier = Modifier.size(40.dp)) {
                            Icon(Icons.Filled.Close, contentDescription = tr("common.clearSearch"), modifier = Modifier.size(20.dp))
                        }
                    }
                }
            }
        }
    }
}
