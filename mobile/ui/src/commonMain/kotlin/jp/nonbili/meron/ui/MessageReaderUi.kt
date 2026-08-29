package jp.nonbili.meron.ui

import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.backhandler.BackHandler
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Velocity
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.standaloneAttachments
import jp.nonbili.meron.shared.visibleImageAttachments
import kotlinx.coroutines.launch

// Full-screen reader for a single message — the mobile equivalent of the desktop
// "open in new tab" reader, showing the full header plus the message body.
@OptIn(ExperimentalMaterial3Api::class, ExperimentalComposeUiApi::class)
@Composable
internal fun MessageReaderScreen(
    message: MessageBody,
    preferHtml: Boolean,
    actionsEnabled: Boolean,
    remoteContent: MessageRemoteContent,
    onBack: () -> Unit,
    onCopy: (String, String) -> Unit,
    onComposeTo: (String) -> Unit,
    onForward: (MessageBody) -> Unit,
    onEditAsNew: (MessageBody) -> Unit,
    onDelete: (MessageBody) -> Unit,
    onOpenAttachment: (MessageAttachment) -> Unit,
    onSaveAttachment: (MessageAttachment) -> Unit,
    loadImageAttachment: suspend (MessageAttachment) -> ImageBitmap?,
    onOpenImageAttachment: (MessageAttachment) -> Unit,
    onOpenHtmlImage: (String) -> Unit,
    onOpenUrl: (String) -> Unit,
) {
    val messageTextLabel = tr("chat.messageText")
    val subjectLabel = tr("composer.fields.subject")
    val messageIdLabel = tr("chat.messageId")
    val noSubjectLabel = tr("threads.noSubject")
    var menuOpen by remember(message.id) { mutableStateOf(false) }
    val scrollState = rememberScrollState()
    val density = LocalDensity.current
    val dismissThresholdPx = remember(density) { with(density) { 120.dp.toPx() } }
    var screenHeightPx by remember { mutableStateOf(0f) }
    var pullDistancePx by remember(message.id) { mutableStateOf(0f) }
    var dismissedByPull by remember(message.id) { mutableStateOf(false) }
    var isAnimating by remember(message.id) { mutableStateOf(false) }
    val coroutineScope = rememberCoroutineScope()

    BackHandler(onBack = onBack)

    suspend fun handleDragRelease(velocityY: Float) {
        isAnimating = true
        if (pullDistancePx >= dismissThresholdPx || velocityY > 1000f) {
            val target = if (screenHeightPx > 0f) screenHeightPx else with(density) { 800.dp.toPx() }
            animate(
                initialValue = pullDistancePx,
                targetValue = target,
                initialVelocity = velocityY,
                animationSpec = spring(stiffness = Spring.StiffnessMediumLow),
            ) { value, _ ->
                pullDistancePx = value
            }
            dismissedByPull = true
            onBack()
        } else {
            animate(
                initialValue = pullDistancePx,
                targetValue = 0f,
                initialVelocity = velocityY,
                animationSpec = spring(stiffness = Spring.StiffnessMedium),
            ) { value, _ ->
                pullDistancePx = value
            }
        }
        isAnimating = false
    }

    val pullToConversationConnection =
        remember(message.id, dismissThresholdPx, scrollState, screenHeightPx) {
            object : NestedScrollConnection {
                override fun onPreScroll(
                    available: Offset,
                    source: NestedScrollSource,
                ): Offset {
                    if (dismissedByPull || isAnimating) return Offset.Zero

                    val resistance =
                        if (screenHeightPx > 0f) {
                            (1f - (pullDistancePx / screenHeightPx).coerceIn(0f, 1f)).coerceAtLeast(0.3f)
                        } else {
                            0.8f
                        }

                    // When pulling down (available.y > 0) and at the top of the scrollable content (scrollState.value == 0)
                    if (available.y > 0f && scrollState.value == 0) {
                        pullDistancePx += available.y * resistance
                        return Offset(0f, available.y)
                    }

                    // When pushing back up (available.y < 0) and we have already pulled down (pullDistancePx > 0)
                    if (available.y < 0f && pullDistancePx > 0f) {
                        val consumedY = available.y.coerceAtLeast(-pullDistancePx / resistance)
                        pullDistancePx = (pullDistancePx + consumedY * resistance).coerceAtLeast(0f)
                        return Offset(0f, consumedY)
                    }

                    return Offset.Zero
                }

                override suspend fun onPreFling(available: Velocity): Velocity {
                    if (dismissedByPull || isAnimating || pullDistancePx == 0f) return Velocity.Zero
                    handleDragRelease(available.y)
                    return available
                }
            }
        }

    Box(
        modifier =
            Modifier
                .fillMaxSize()
                .onSizeChanged { size ->
                    screenHeightPx = size.height.toFloat()
                }.graphicsLayer {
                    translationY = pullDistancePx
                    val progress = if (screenHeightPx > 0f) (pullDistancePx / screenHeightPx).coerceIn(0f, 1f) else 0f
                    alpha = 1f - progress * 0.4f
                    scaleX = 1f - progress * 0.05f
                    scaleY = 1f - progress * 0.05f
                }.nestedScroll(pullToConversationConnection)
                .pointerInput(Unit) {
                    detectTapGestures { }
                },
    ) {
        Scaffold(
            topBar = {
                TopAppBar(
                    modifier =
                        Modifier.pointerInput(message.id, screenHeightPx) {
                            detectVerticalDragGestures(
                                onVerticalDrag = { _, dragAmount ->
                                    if (!dismissedByPull && !isAnimating) {
                                        val resistance =
                                            if (screenHeightPx > 0f) {
                                                (1f - (pullDistancePx / screenHeightPx).coerceIn(0f, 1f)).coerceAtLeast(0.3f)
                                            } else {
                                                0.8f
                                            }
                                        pullDistancePx = (pullDistancePx + dragAmount * resistance).coerceAtLeast(0f)
                                    }
                                },
                                onDragEnd = {
                                    if (!dismissedByPull && !isAnimating && pullDistancePx > 0f) {
                                        coroutineScope.launch {
                                            handleDragRelease(0f)
                                        }
                                    }
                                },
                                onDragCancel = {
                                    if (!dismissedByPull && !isAnimating && pullDistancePx > 0f) {
                                        coroutineScope.launch {
                                            handleDragRelease(0f)
                                        }
                                    }
                                },
                            )
                        },
                    // The subject lives in the content area below, so the bar
                    // stays empty and gives the body the full width.
                    title = {},
                    navigationIcon = {
                        IconButton(onClick = onBack) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = tr("buttons.back"))
                        }
                    },
                    actions = {
                        Box {
                            IconButton(onClick = { menuOpen = true }) {
                                Icon(Icons.Filled.MoreVert, contentDescription = tr("chat.moreMessageActions"))
                            }
                            DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                                DropdownMenuItem(
                                    text = { Text(tr("chat.copyMessageText")) },
                                    onClick = {
                                        menuOpen = false
                                        onCopy(messageTextLabel, messagePlainText(message))
                                    },
                                )
                                DropdownMenuItem(
                                    text = { Text(tr("chat.copySubject")) },
                                    onClick = {
                                        menuOpen = false
                                        onCopy(subjectLabel, message.subject.ifBlank { noSubjectLabel })
                                    },
                                )
                                if (message.messageId.isNotBlank()) {
                                    DropdownMenuItem(
                                        text = { Text(tr("chat.copyMessageId")) },
                                        onClick = {
                                            menuOpen = false
                                            onCopy(messageIdLabel, message.messageId)
                                        },
                                    )
                                }
                                if (actionsEnabled) {
                                    DropdownMenuItem(
                                        text = { Text(tr("chat.actions.forward")) },
                                        onClick = {
                                            menuOpen = false
                                            onForward(message)
                                        },
                                    )
                                    DropdownMenuItem(
                                        text = { Text(tr("chat.actions.editAsNewMessage")) },
                                        onClick = {
                                            menuOpen = false
                                            onEditAsNew(message)
                                        },
                                    )
                                    DropdownMenuItem(
                                        text = { Text(tr("chat.actions.deleteMessage"), color = MaterialTheme.colorScheme.error) },
                                        onClick = {
                                            menuOpen = false
                                            onDelete(message)
                                        },
                                    )
                                }
                            }
                        }
                    },
                )
            },
        ) { innerPadding ->
            Column(
                Modifier
                    .fillMaxSize()
                    .background(MaterialTheme.colorScheme.background)
                    .padding(innerPadding)
                    .appScrollbar(scrollState)
                    .verticalScroll(scrollState)
                    .padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text(
                    message.subject.ifBlank { noSubjectLabel },
                    fontWeight = FontWeight.SemiBold,
                    style = MaterialTheme.typography.titleLarge,
                )
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    // The From chip already names the sender, so there is no
                    // separate sender line. Tapping any chip copies the full
                    // `Name <addr>`.
                    MessageAddressDetails(
                        message = message,
                        onCopy = onCopy,
                        onComposeTo = onComposeTo,
                        textColor = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            formatMessageFullTimestamp(message.dateEpochSeconds),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        BlockedRemoteButton(
                            message = message,
                            remoteContent = remoteContent,
                            preferHtml = preferHtml,
                            searchQuery = "",
                        )
                    }
                }
                HorizontalDivider()
                val htmlBody = preferHtml && message.bodyHtml.isNotBlank()
                val standaloneAttachmentsForMessage = standaloneAttachments(message)
                val (imageAttachments, otherAttachments) =
                    standaloneAttachmentsForMessage.partition { it.mimeType.startsWith("image/") }
                val visibleImages = visibleImageAttachments(imageAttachments, remoteContent.allowRemote)
                if (htmlBody) {
                    // Only the reader shrinks over-wide mail to fit: it has the
                    // full screen to scale into, where a bubble would render the
                    // same mail as an unreadable thumbnail.
                    HtmlMessageBody(
                        html = message.bodyHtml,
                        allowRemote = remoteContent.allowRemote,
                        onOpenUrl = onOpenUrl,
                        onOpenImage = onOpenHtmlImage,
                        fitWideContent = true,
                    )
                } else {
                    SelectableMessageText(
                        text =
                            message.body.ifBlank {
                                if (message.bodyMissing) tr("chat.messageLoadFailed") else "(no content)"
                            },
                        onOpenUrl = onOpenUrl,
                        style = messageBodyTextStyle(MaterialTheme.typography.bodyLarge),
                    )
                }
                if (visibleImages.isNotEmpty() || otherAttachments.isNotEmpty()) {
                    HorizontalDivider()
                    if (visibleImages.isNotEmpty()) {
                        AttachmentImageGrid(
                            images = visibleImages,
                            loadImageAttachment = loadImageAttachment,
                            onOpen = onOpenImageAttachment,
                        )
                    }
                    otherAttachments.forEach { attachment ->
                        AttachmentRow(
                            attachment = attachment,
                            textColor = MaterialTheme.colorScheme.onSurface,
                            onOpen = { onOpenAttachment(attachment) },
                            onSave = { onSaveAttachment(attachment) },
                        )
                    }
                }
            }
        }
    }
}
