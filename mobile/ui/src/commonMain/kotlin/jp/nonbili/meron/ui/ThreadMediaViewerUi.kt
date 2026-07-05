package jp.nonbili.meron.ui

import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.spring
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Image
import androidx.compose.material.icons.filled.KeyboardArrowLeft
import androidx.compose.material.icons.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Save
import androidx.compose.material.icons.filled.Share
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.backhandler.BackHandler
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.ThreadGalleryImage
import jp.nonbili.meron.shared.ThreadMediaItem
import jp.nonbili.meron.shared.attachmentMediaRef
import kotlinx.coroutines.launch
import kotlin.math.abs

@Composable
internal fun ConversationMediaTile(
    item: ThreadMediaItem,
    loadImageAttachment: suspend (MessageAttachment) -> ImageBitmap?,
    onOpen: () -> Unit,
) {
    Surface(
        onClick = onOpen,
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(8.dp),
        color = Color.Black.copy(alpha = 0.08f),
    ) {
        Box(
            Modifier
                .fillMaxWidth()
                .height(96.dp),
            contentAlignment = Alignment.Center,
        ) {
            if (item.type == "image") {
                AsyncAttachmentImage(
                    attachment = item.attachment,
                    ref = item.ref,
                    loadImageAttachment = loadImageAttachment,
                    contentDescription = item.filename,
                    modifier = Modifier.fillMaxSize(),
                    contentScale = ContentScale.Crop,
                )
            } else {
                Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Icon(Icons.Filled.PlayArrow, contentDescription = null, tint = Color.White, modifier = Modifier.size(26.dp))
                    Text(
                        item.filename,
                        color = Color.White,
                        fontSize = 10.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.padding(horizontal = 6.dp),
                    )
                }
            }
        }
    }
}

@Composable
internal fun AsyncMediaImage(
    ref: String,
    contentDescription: String,
    modifier: Modifier,
    contentScale: ContentScale,
) {
    var bitmap by remember(ref) { mutableStateOf<ImageBitmap?>(null) }
    LaunchedEffect(ref) {
        bitmap = loadImageBitmapRef(ref)
    }
    val image = bitmap
    if (image != null) {
        Image(
            bitmap = image,
            contentDescription = contentDescription,
            modifier = modifier,
            contentScale = contentScale,
        )
    } else {
        Box(modifier.background(Color.Black.copy(alpha = 0.35f)), contentAlignment = Alignment.Center) {
            Icon(Icons.Filled.Image, contentDescription = null, tint = Color.White.copy(alpha = 0.75f), modifier = Modifier.size(24.dp))
        }
    }
}

@Composable
internal fun AsyncAttachmentImage(
    attachment: MessageAttachment,
    ref: String,
    loadImageAttachment: suspend (MessageAttachment) -> ImageBitmap?,
    contentDescription: String,
    modifier: Modifier,
    contentScale: ContentScale,
) {
    var bitmap by remember(attachment.key, attachment.url, ref) { mutableStateOf<ImageBitmap?>(null) }
    LaunchedEffect(attachment.key, attachment.url, ref) {
        bitmap = loadImageAttachment(attachment) ?: loadImageBitmapRef(ref)
    }
    val image = bitmap
    if (image != null) {
        Image(
            bitmap = image,
            contentDescription = contentDescription,
            modifier = modifier,
            contentScale = contentScale,
        )
    } else {
        Box(modifier.background(Color.Black.copy(alpha = 0.35f)), contentAlignment = Alignment.Center) {
            Icon(Icons.Filled.Image, contentDescription = null, tint = Color.White.copy(alpha = 0.75f), modifier = Modifier.size(24.dp))
        }
    }
}

@OptIn(ExperimentalComposeUiApi::class)
@Composable
internal fun ThreadImageGallery(
    images: List<ThreadGalleryImage>,
    index: Int,
    onIndexChange: (Int) -> Unit,
    onClose: () -> Unit,
    onSave: (MessageAttachment) -> Unit,
    onShare: (MessageAttachment) -> Unit,
    onCopy: (MessageAttachment) -> Unit,
    loadImageAttachment: suspend (MessageAttachment) -> ImageBitmap?,
) {
    val current = images.getOrNull(index) ?: return
    val canPrev = index > 0
    val canNext = index < images.lastIndex
    var scale by remember(current.ref) { mutableStateOf(1f) }
    var offset by remember(current.ref) { mutableStateOf(Offset.Zero) }
    var swipeDistance by remember(current.ref) { mutableStateOf(0f) }
    var menuOpen by remember { mutableStateOf(false) }
    // 0 = undecided, 1 = horizontal swipe (change photo), 2 = vertical pull (dismiss).
    var dragMode by remember(current.ref) { mutableStateOf(0) }
    var pullDistancePx by remember(current.ref) { mutableStateOf(0f) }
    var screenHeightPx by remember { mutableStateOf(0f) }
    val density = LocalDensity.current
    val dismissThresholdPx = remember(density) { with(density) { 120.dp.toPx() } }
    val coroutineScope = rememberCoroutineScope()

    BackHandler(onBack = onClose)

    fun goPrev() {
        if (!canPrev) return
        scale = 1f
        offset = Offset.Zero
        onIndexChange(index - 1)
    }

    fun goNext() {
        if (!canNext) return
        scale = 1f
        offset = Offset.Zero
        onIndexChange(index + 1)
    }

    suspend fun settleOrDismissPull(velocityY: Float) {
        if (pullDistancePx >= dismissThresholdPx || velocityY > 1000f) {
            val target = if (screenHeightPx > 0f) screenHeightPx else with(density) { 800.dp.toPx() }
            animate(
                initialValue = pullDistancePx,
                targetValue = target,
                initialVelocity = velocityY,
                animationSpec = spring(stiffness = Spring.StiffnessMediumLow),
            ) { value, _ -> pullDistancePx = value }
            onClose()
        } else {
            animate(
                initialValue = pullDistancePx,
                targetValue = 0f,
                initialVelocity = velocityY,
                animationSpec = spring(stiffness = Spring.StiffnessMedium),
            ) { value, _ -> pullDistancePx = value }
        }
    }

    Box(
        Modifier
            .fillMaxSize()
            .onSizeChanged { size -> screenHeightPx = size.height.toFloat() }
            .graphicsLayer {
                translationY = pullDistancePx
                val progress = if (screenHeightPx > 0f) (pullDistancePx / screenHeightPx).coerceIn(0f, 1f) else 0f
                alpha = 1f - progress * 0.4f
                scaleX = 1f - progress * 0.05f
                scaleY = 1f - progress * 0.05f
            }.background(Color.Black.copy(alpha = 0.94f)),
    ) {
        Row(
            Modifier
                .fillMaxWidth()
                .align(Alignment.TopCenter)
                .padding(horizontal = 4.dp, vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            IconButton(onClick = onClose) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = tr("buttons.back"), tint = Color.White)
            }
            Text(
                if (images.size > 1) "${index + 1} / ${images.size}" else "",
                color = Color.White.copy(alpha = 0.86f),
                fontSize = 12.sp,
                modifier = Modifier.weight(1f),
            )
            Box {
                IconButton(onClick = { menuOpen = true }) {
                    Icon(Icons.Filled.MoreVert, contentDescription = tr("common.more"), tint = Color.White)
                }
                DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                    DropdownMenuItem(
                        text = { Text(tr("common.share")) },
                        leadingIcon = { Icon(Icons.Filled.Share, contentDescription = null) },
                        onClick = {
                            menuOpen = false
                            onShare(current.attachment)
                        },
                    )
                    DropdownMenuItem(
                        text = { Text(tr("common.copy")) },
                        leadingIcon = { Icon(Icons.Filled.ContentCopy, contentDescription = null) },
                        onClick = {
                            menuOpen = false
                            onCopy(current.attachment)
                        },
                    )
                    DropdownMenuItem(
                        text = { Text(tr("buttons.save")) },
                        leadingIcon = { Icon(Icons.Filled.Save, contentDescription = null) },
                        onClick = {
                            menuOpen = false
                            onSave(current.attachment)
                        },
                    )
                }
            }
        }

        Box(
            Modifier
                .fillMaxSize()
                .padding(horizontal = 12.dp, vertical = 64.dp)
                .pointerInput(current.ref, scale) {
                    if (scale > 1.02f) return@pointerInput
                    detectDragGestures(
                        onDragStart = {
                            dragMode = 0
                            swipeDistance = 0f
                        },
                        onDragEnd = {
                            if (dragMode == 1 && abs(swipeDistance) > 80f) {
                                if (swipeDistance > 0f) goPrev() else goNext()
                            } else if (dragMode == 2) {
                                coroutineScope.launch { settleOrDismissPull(0f) }
                            }
                            swipeDistance = 0f
                            dragMode = 0
                        },
                        onDragCancel = {
                            if (dragMode == 2) coroutineScope.launch { settleOrDismissPull(0f) }
                            swipeDistance = 0f
                            dragMode = 0
                        },
                    ) { _, dragAmount ->
                        if (dragMode == 0) {
                            dragMode =
                                when {
                                    abs(dragAmount.x) > abs(dragAmount.y) -> 1
                                    dragAmount.y > 0f -> 2
                                    else -> 0
                                }
                        }
                        when (dragMode) {
                            1 -> {
                                swipeDistance += dragAmount.x
                            }

                            2 -> {
                                val resistance =
                                    if (screenHeightPx > 0f) {
                                        (1f - (pullDistancePx / screenHeightPx).coerceIn(0f, 1f)).coerceAtLeast(0.3f)
                                    } else {
                                        0.8f
                                    }
                                pullDistancePx = (pullDistancePx + dragAmount.y * resistance).coerceAtLeast(0f)
                            }
                        }
                    }
                }.pointerInput(current.ref) {
                    // Only track multi-touch here so a single-finger drag is left
                    // unconsumed for the horizontal-drag detector above to see —
                    // detectTransformGestures reacts to one finger too and would
                    // otherwise swallow the swipe-to-navigate gesture.
                    awaitEachGesture {
                        awaitFirstDown(requireUnconsumed = false)
                        do {
                            val event = awaitPointerEvent()
                            if (event.changes.size >= 2) {
                                val zoomChange = event.calculateZoom()
                                val panChange = event.calculatePan()
                                val nextScale = (scale * zoomChange).coerceIn(1f, 6f)
                                scale = nextScale
                                offset = if (nextScale == 1f) Offset.Zero else offset + panChange
                                event.changes.forEach { it.consume() }
                            }
                        } while (event.changes.any { it.pressed })
                    }
                },
            contentAlignment = Alignment.Center,
        ) {
            AsyncAttachmentImage(
                attachment = current.attachment,
                ref = current.ref,
                loadImageAttachment = loadImageAttachment,
                contentDescription = current.filename,
                modifier =
                    Modifier
                        .fillMaxSize()
                        .graphicsLayer {
                            scaleX = scale
                            scaleY = scale
                            translationX = offset.x
                            translationY = offset.y
                        },
                contentScale = ContentScale.Fit,
            )
        }

        Row(
            Modifier
                .align(Alignment.Center)
                .fillMaxWidth()
                .padding(horizontal = 4.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(
                enabled = canPrev,
                onClick = ::goPrev,
            ) {
                Icon(
                    Icons.Filled.KeyboardArrowLeft,
                    contentDescription = tr("chat.previousImage"),
                    tint = if (canPrev) Color.White.copy(alpha = 0.55f) else Color.White.copy(alpha = 0.18f),
                    modifier = Modifier.size(24.dp),
                )
            }
            IconButton(
                enabled = canNext,
                onClick = ::goNext,
            ) {
                Icon(
                    Icons.Filled.KeyboardArrowRight,
                    contentDescription = tr("chat.nextImage"),
                    tint = if (canNext) Color.White.copy(alpha = 0.55f) else Color.White.copy(alpha = 0.18f),
                    modifier = Modifier.size(24.dp),
                )
            }
        }

        Text(
            current.filename,
            color = Color.White.copy(alpha = 0.78f),
            fontSize = 12.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier =
                Modifier
                    .align(Alignment.BottomCenter)
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 18.dp),
        )
    }
}

@Composable
internal fun ImagePreviewDialog(
    preview: ImagePreview,
    onShare: () -> Unit,
    onCopy: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Text(preview.title, maxLines = 1, overflow = TextOverflow.Ellipsis)
        },
        text = {
            Box(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 220.dp, max = 520.dp)
                    .background(Color.Black, RoundedCornerShape(8.dp)),
                contentAlignment = Alignment.Center,
            ) {
                Image(
                    bitmap = preview.image,
                    contentDescription = preview.title,
                    modifier = Modifier.fillMaxSize().clip(RoundedCornerShape(8.dp)),
                    contentScale = ContentScale.Fit,
                )
            }
        },
        dismissButton = {
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                TextButton(onClick = onShare) { Text(tr("common.share")) }
                TextButton(onClick = onCopy) { Text(tr("common.copy")) }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.done")) } },
    )
}

@Composable
internal fun AttachmentImageGrid(
    images: List<MessageAttachment>,
    loadImageAttachment: suspend (MessageAttachment) -> ImageBitmap? = { null },
    onOpen: (MessageAttachment) -> Unit,
) {
    val rows = remember(images) { images.chunked(3) }
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        rows.forEach { row ->
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                row.forEach { attachment ->
                    Surface(
                        onClick = { onOpen(attachment) },
                        modifier = Modifier.weight(1f).aspectRatio(1f),
                        shape = RoundedCornerShape(8.dp),
                        color = Color.Black.copy(alpha = 0.08f),
                    ) {
                        AsyncAttachmentImage(
                            attachment = attachment,
                            ref = attachmentMediaRef(attachment),
                            loadImageAttachment = loadImageAttachment,
                            contentDescription = attachment.filename.ifBlank { "Image" },
                            modifier = Modifier.fillMaxSize(),
                            contentScale = ContentScale.Crop,
                        )
                    }
                }
                repeat(3 - row.size) { Spacer(Modifier.weight(1f)) }
            }
        }
    }
}

@Composable
internal fun AttachmentRow(
    attachment: MessageAttachment,
    textColor: Color,
    onOpen: () -> Unit,
    onSave: () -> Unit,
) {
    Surface(
        onClick = onOpen,
        shape = RoundedCornerShape(8.dp),
        color = textColor.copy(alpha = 0.08f),
        contentColor = textColor,
    ) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 10.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Icon(Icons.Filled.AttachFile, contentDescription = null, modifier = Modifier.size(18.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    attachment.filename.ifBlank { "Attachment" },
                    fontSize = 12.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    listOf(attachment.mimeType, formatBytes(attachment.sizeBytes)).filter { it.isNotBlank() }.joinToString(" · "),
                    fontSize = 10.5.sp,
                    color = textColor.copy(alpha = 0.7f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            TextButton(onClick = onSave) {
                Text(tr("buttons.save"))
            }
        }
    }
}
