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
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.ContentCopy
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
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
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
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Velocity
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.standaloneAttachments
import kotlinx.coroutines.launch

// Full-screen reader for a single message — the mobile equivalent of the desktop
// "open in new tab" reader, showing the full header plus the message body.
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun MessageReaderScreen(
    message: MessageBody,
    preferHtml: Boolean,
    onBack: () -> Unit,
    onCopy: (String, String) -> Unit,
    onOpenAttachment: (MessageAttachment) -> Unit,
    onSaveAttachment: (MessageAttachment) -> Unit,
    loadImageAttachment: suspend (MessageAttachment) -> ImageBitmap?,
    onOpenImageAttachment: (MessageAttachment) -> Unit,
    onOpenHtmlImage: (String) -> Unit,
    onOpenUrl: (String) -> Unit,
) {
    val messageTextLabel = tr("chat.messageText")
    val scrollState = rememberScrollState()
    val density = LocalDensity.current
    val dismissThresholdPx = remember(density) { with(density) { 120.dp.toPx() } }
    var screenHeightPx by remember { mutableStateOf(0f) }
    var pullDistancePx by remember(message.id) { mutableStateOf(0f) }
    var dismissedByPull by remember(message.id) { mutableStateOf(false) }
    var isAnimating by remember(message.id) { mutableStateOf(false) }
    val coroutineScope = rememberCoroutineScope()

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
                    title = {
                        Text(
                            message.subject.ifBlank { tr("threads.noSubject") },
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            fontWeight = FontWeight.SemiBold,
                            style = MaterialTheme.typography.titleMedium,
                        )
                    },
                    navigationIcon = {
                        IconButton(onClick = onBack) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = tr("buttons.back"))
                        }
                    },
                    actions = {
                        IconButton(onClick = { onCopy(messageTextLabel, messagePlainText(message)) }) {
                            Icon(Icons.Filled.ContentCopy, contentDescription = tr("chat.copyMessageText"))
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
                    .verticalScroll(scrollState)
                    .padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(
                        message.from.ifBlank { message.fromAddr },
                        fontWeight = FontWeight.SemiBold,
                        style = MaterialTheme.typography.titleSmall,
                    )
                    MessageReaderHeaderRow(tr("composer.fields.to"), message.to)
                    MessageReaderHeaderRow(tr("composer.fields.cc"), message.cc)
                    Text(
                        formatMessageFullTimestamp(message.dateEpochSeconds),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                HorizontalDivider()
                if (preferHtml && message.bodyHtml.isNotBlank()) {
                    HtmlMessageBody(html = message.bodyHtml, onOpenUrl = onOpenUrl, onOpenImage = onOpenHtmlImage)
                } else {
                    Text(
                        message.body.ifBlank {
                            if (message.bodyMissing) tr("chat.messageLoadFailed") else "(no content)"
                        },
                        style = MaterialTheme.typography.bodyLarge,
                    )
                }
                val standaloneAttachmentsForMessage = standaloneAttachments(message)
                if (standaloneAttachmentsForMessage.isNotEmpty()) {
                    HorizontalDivider()
                    val (imageAttachments, otherAttachments) =
                        standaloneAttachmentsForMessage.partition { it.mimeType.startsWith("image/") }
                    if (imageAttachments.isNotEmpty()) {
                        AttachmentImageGrid(
                            images = imageAttachments,
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

@Composable
private fun MessageReaderHeaderRow(
    label: String,
    value: String,
) {
    if (value.isBlank()) return
    Text(
        "$label: $value",
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}
