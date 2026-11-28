package jp.nonbili.meron.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.OpenInFull
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.chrisbanes.haze.HazeInput
import dev.chrisbanes.haze.HazeState
import dev.chrisbanes.haze.blur.HazeBlurStyle
import dev.chrisbanes.haze.blur.HazeColorEffect
import dev.chrisbanes.haze.blur.hazeBlur
import dev.chrisbanes.haze.hazeSource
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.ReplyRecipients
import jp.nonbili.meron.shared.SendIdentity
import jp.nonbili.meron.shared.formatSendIdentity

internal val REPLY_CARD_TOP_MARGIN = 8.dp
internal val REPLY_CARD_RADIUS = 24.dp

// The reply card and the pinned header ride over the conversation, so they
// sit higher than the bubbles (3.dp) that scroll under them.
internal val FLOATING_CARD_ELEVATION = 4.dp
internal val FLOATING_BAR_CHIP_HEIGHT = 48.dp

// A card floating on content that scrolls on under it, drawn like the reply
// card and the pinned header.
@Composable
internal fun FloatingCard(
    shape: Shape,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    Surface(
        color = MaterialTheme.colorScheme.surface,
        shape = shape,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.6f)),
        shadowElevation = FLOATING_CARD_ELEVATION,
        modifier = modifier,
        content = content,
    )
}

// One of an app bar's controls, floating on the content that scrolls on under
// the bar.
@Composable
internal fun FloatingBarChip(
    shape: Shape,
    modifier: Modifier = Modifier,
    content: @Composable RowScope.() -> Unit,
) {
    FloatingCard(shape, modifier.height(FLOATING_BAR_CHIP_HEIGHT)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.Center, content = content)
    }
}

/** WebViews must not be captured into Haze's offscreen layers on Android. */
internal fun Modifier.readerHazeSource(
    hazeState: HazeState,
    zIndex: Float = 0f,
): Modifier = if (platformName == "android") this else hazeSource(hazeState, zIndex = zIndex)

/** Frosts the reader on iOS; Android uses a veil without capturing its WebViews. */
@Composable
internal fun FrostedBand(
    hazeState: HazeState,
    heightPx: Int,
    modifier: Modifier = Modifier,
    fadeAtTopPx: Float = 0f,
    fadeAtBottomPx: Float = 0f,
    // Strong enough to keep lines of text from reading as text, light
    // enough that they still show moving underneath.
    tintAlpha: Float = 0.3f,
) {
    if (heightPx <= 0) return
    val background = MaterialTheme.colorScheme.background
    if (platformName == "android") {
        // Keep the bands legible without drawing the message list into an
        // offscreen layer: native WebView GL functors can crash inside one.
        val veil = background.copy(alpha = 0.7f)
        val height = heightPx.toFloat()
        Box(
            modifier
                .fillMaxWidth()
                .height(with(LocalDensity.current) { heightPx.toDp() })
                .background(
                    Brush.verticalGradient(
                        0f to if (fadeAtTopPx > 0f) Color.Transparent else veil,
                        (fadeAtTopPx / height).coerceIn(0f, 0.5f) to veil,
                        (1f - fadeAtBottomPx / height).coerceIn(0.5f, 1f) to veil,
                        1f to if (fadeAtBottomPx > 0f) Color.Transparent else veil,
                    ),
                ),
        )
        return
    }
    val style =
        remember(background, heightPx, fadeAtTopPx, fadeAtBottomPx, tintAlpha) {
            val height = heightPx.toFloat()
            HazeBlurStyle {
                backgroundColor(background)
                blurRadius(12.dp)
                noiseFactor(0f)
                colorEffects(listOf(HazeColorEffect.tint(background.copy(alpha = tintAlpha))))
                // Keep the controls readable where blur is unavailable.
                fallbackColorEffect(HazeColorEffect.tint(background.copy(alpha = 0.7f)))
                if (fadeAtTopPx > 0f || fadeAtBottomPx > 0f) {
                    mask(
                        Brush.verticalGradient(
                            0f to if (fadeAtTopPx > 0f) Color.Transparent else Color.Black,
                            (fadeAtTopPx / height).coerceIn(0f, 0.5f) to Color.Black,
                            (1f - fadeAtBottomPx / height).coerceIn(0.5f, 1f) to Color.Black,
                            1f to if (fadeAtBottomPx > 0f) Color.Transparent else Color.Black,
                        ),
                    )
                }
            }
        }
    Box(
        modifier
            .fillMaxWidth()
            .height(with(LocalDensity.current) { heightPx.toDp() })
            .hazeBlur(HazeInput.Sources(hazeState), style),
    )
}

@Composable
internal fun ReplyBar(
    value: String,
    onChange: (String) -> Unit,
    attachments: List<DraftAttachment>,
    failureMessage: String,
    sendShortcutMode: SendShortcutMode,
    onAttach: () -> Unit,
    onRemoveAttachment: (DraftAttachment) -> Unit,
    onOpenFullEditor: () -> Unit,
    onFocusChange: (Boolean) -> Unit = {},
    onSend: () -> Unit,
    onRetry: () -> Unit = onSend,
    // Whether the bar holds anything worth sending. Passed in rather than read
    // off `value`, because a bar seeded with the account's signature is not
    // blank yet holds nothing the user wrote.
    hasContent: Boolean = value.isNotBlank() || attachments.isNotEmpty(),
    sending: Boolean = false,
    fromIdentities: List<SendIdentity> = emptyList(),
    recipients: ReplyRecipients = ReplyRecipients("", ""),
    selectedFrom: SendIdentity? = null,
    onSelectFrom: (SendIdentity) -> Unit = {},
) {
    // One rounded card floating on the conversation, like desktop's composer:
    // the text and its controls read as a single box rather than as a field
    // set into a bar.
    Surface(
        modifier = Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, top = REPLY_CARD_TOP_MARGIN, bottom = 12.dp),
        shape = RoundedCornerShape(REPLY_CARD_RADIUS),
        // The surface the message cards are drawn on, so the card reads as
        // one of them rather than as a pressed or disabled slab.
        color = MaterialTheme.colorScheme.surface,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.6f)),
        shadowElevation = FLOATING_CARD_ELEVATION,
    ) {
        Column(Modifier.fillMaxWidth().padding(4.dp)) {
            if (attachments.isNotEmpty()) {
                LazyRow(
                    contentPadding = PaddingValues(start = 10.dp, end = 10.dp, top = 6.dp),
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    items(attachments, key = { it.id }) { attachment ->
                        FilterChip(
                            selected = false,
                            onClick = { onRemoveAttachment(attachment) },
                            label = {
                                Text(
                                    attachment.displayName,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            },
                            leadingIcon = {
                                Icon(Icons.Filled.AttachFile, contentDescription = null, modifier = Modifier.size(16.dp))
                            },
                        )
                    }
                }
            }
            if (failureMessage.isNotBlank()) {
                Row(
                    Modifier.fillMaxWidth().padding(start = 12.dp, top = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Icon(Icons.Filled.ErrorOutline, contentDescription = null, tint = MaterialTheme.colorScheme.error)
                    Text(
                        tr("reply.failedDraftKept"),
                        modifier = Modifier.weight(1f),
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodySmall,
                    )
                    TextButton(onClick = onRetry, enabled = !sending) {
                        Text(tr("chat.retry"))
                    }
                }
            }
            val canSend = !sending && hasContent
            // The text leads and has the card's full width; everything else
            // shares the one line under it, so a phone's width goes to the
            // reply rather than to the buttons beside it.
            // A bare field rather than Material's, whose 56dp minimum made the
            // card half as tall again as it needs to be. The padding is inside
            // the field, so a tap anywhere on the row focuses it.
            val replyTextStyle = MaterialTheme.typography.bodyLarge.copy(color = MaterialTheme.colorScheme.onSurface)
            // The field's own value, kept as BasicTextField's String overload keeps
            // it, so a focus request can also place the caret. Text replaced from
            // outside (a send, a quote) drops the IME's composition over the old.
            var fieldState by remember { mutableStateOf(TextFieldValue(value)) }
            val fieldValue = if (fieldState.text == value) fieldState else TextFieldValue(value, fieldState.selection)
            SideEffect {
                if (fieldValue.selection != fieldState.selection || fieldValue.composition != fieldState.composition) {
                    fieldState = fieldValue
                }
            }
            var lastText by remember(value) { mutableStateOf(value) }
            val focusRequester = remember { FocusRequester() }
            // Null on a conversation page that isn't the open one: the pages
            // beside it are composed too, and must not take the focus.
            val focusRequest = LocalQuickReplyFocusRequest.current
            // Only requests made while this bar is the open one's: one made before
            // (in another conversation, or before it was swiped to) isn't for it.
            var handledFocusRequest by remember { mutableStateOf(focusRequest?.sequence) }
            LaunchedEffect(focusRequest) {
                if (focusRequest == null) {
                    handledFocusRequest = null
                    return@LaunchedEffect
                }
                val handled = handledFocusRequest
                handledFocusRequest = focusRequest.sequence
                if (handled == null || handled == focusRequest.sequence) return@LaunchedEffect
                fieldState = TextFieldValue(value, TextRange(focusRequest.caret.coerceIn(0, value.length)))
                focusRequester.requestFocus()
            }
            BasicTextField(
                value = fieldValue,
                onValueChange = {
                    fieldState = it
                    val changed = it.text != lastText
                    lastText = it.text
                    if (changed) onChange(it.text)
                },
                textStyle = replyTextStyle,
                cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                maxLines = 5,
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .focusRequester(focusRequester)
                        .onFocusChanged { onFocusChange(it.isFocused) }
                        .onPreviewKeyEvent { event ->
                            if (shouldSendFromEditor(event, sendShortcutMode) && canSend) {
                                onSend()
                                true
                            } else {
                                false
                            }
                        },
                decorationBox = { field ->
                    Box(Modifier.padding(start = 14.dp, end = 14.dp, top = 12.dp, bottom = 4.dp)) {
                        if (value.isEmpty()) {
                            Text(
                                tr("composer.placeholders.quickMessage"),
                                style = replyTextStyle,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        field()
                    }
                },
            )
            Row(Modifier.fillMaxWidth().padding(start = 2.dp, end = 4.dp, bottom = 2.dp), verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = onAttach, modifier = Modifier.size(40.dp)) {
                    Icon(
                        Icons.Filled.AttachFile,
                        contentDescription = tr("composer.actions.attachFiles"),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(20.dp),
                    )
                }
                IconButton(onClick = onOpenFullEditor, modifier = Modifier.size(40.dp)) {
                    Icon(
                        Icons.Filled.OpenInFull,
                        contentDescription = tr("composer.actions.openFullEditor"),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(20.dp),
                    )
                }
                // The recipients sit against the send button and are measured
                // first — they change from thread to thread — so the send-as
                // address after the buttons is the one that truncates, down to
                // a floor that keeps its picker reachable.
                val showRecipients = addressChipItems(recipients.to).isNotEmpty()
                val showFrom = selectedFrom != null && fromIdentities.size > 1
                BoxWithConstraints(Modifier.weight(1f)) {
                    val recipientsMaxWidth = if (showFrom) (maxWidth - REPLY_FROM_MIN_WIDTH).coerceAtLeast(0.dp) else maxWidth
                    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                        Box(Modifier.weight(1f)) {
                            if (showFrom) {
                                ReplyFromRow(
                                    identities = fromIdentities,
                                    selected = selectedFrom,
                                    onSelect = onSelectFrom,
                                )
                            }
                        }
                        if (showRecipients) {
                            ReplyRecipientsRow(
                                recipients = recipients,
                                onOpenFullEditor = onOpenFullEditor,
                                modifier = Modifier.widthIn(max = recipientsMaxWidth),
                            )
                        }
                    }
                }
                FilledIconButton(
                    onClick = onSend,
                    enabled = canSend,
                    modifier = Modifier.padding(start = 4.dp).size(40.dp),
                ) {
                    Icon(Icons.AutoMirrored.Filled.Send, contentDescription = tr("reply.send"), modifier = Modifier.size(20.dp))
                }
            }
        }
    }
}

// The least the send-as address keeps beside long recipients: its label and
// the dropdown arrow.
internal val REPLY_FROM_MIN_WIDTH = 72.dp

// Who the reply goes to. The bar has no address fields, and a plain reply keeps
// the Cc while dropping the other To recipients — a recipient set worth
// disclosing rather than leaving to be discovered in the Sent copy. Read-only:
// tapping opens the full editor, where the addresses can actually be edited.
@Composable
internal fun ReplyRecipientsRow(
    recipients: ReplyRecipients,
    onOpenFullEditor: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val to = addressChipItems(recipients.to)
    if (to.isEmpty()) return
    val cc = addressChipItems(recipients.cc)
    Row(
        modifier
            .clip(RoundedCornerShape(8.dp))
            .clickable(onClick = onOpenFullEditor)
            .padding(horizontal = 6.dp, vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(
            tr("composer.fields.to"),
            style = MaterialTheme.typography.labelSmall,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            summarizeRecipients(to),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f, fill = false),
        )
        if (cc.isNotEmpty()) {
            Text(
                tr("composer.fields.cc"),
                style = MaterialTheme.typography.labelSmall,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                summarizeRecipients(cc),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f, fill = false),
            )
        }
    }
}

// How many names the recipient row spells out before collapsing the rest.
internal const val VISIBLE_RECIPIENT_NAMES = 2

internal fun summarizeRecipients(items: List<AddressChipItem>): String =
    if (items.size <= VISIBLE_RECIPIENT_NAMES) {
        items.joinToString(", ") { it.display }
    } else {
        val shown = items.take(VISIBLE_RECIPIENT_NAMES).joinToString(", ") { it.display }
        "$shown +${items.size - VISIBLE_RECIPIENT_NAMES}"
    }

// The reply bar's send-as identity: shows which address the reply goes out from,
// and opens a picker to override it. Only rendered when the account has more
// than one identity — with a single address there is nothing to disclose.
@Composable
internal fun ReplyFromRow(
    identities: List<SendIdentity>,
    selected: SendIdentity,
    onSelect: (SendIdentity) -> Unit,
    modifier: Modifier = Modifier,
) {
    var expanded by remember { mutableStateOf(false) }
    Box(modifier) {
        Row(
            Modifier
                .clip(RoundedCornerShape(8.dp))
                .clickable { expanded = true }
                .padding(horizontal = 6.dp, vertical = 2.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(
                tr("composer.fields.from"),
                style = MaterialTheme.typography.labelSmall,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                formatSendIdentity(selected),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f, fill = false),
            )
            Icon(
                Icons.Filled.ArrowDropDown,
                contentDescription = tr("composer.actions.chooseSendAddress"),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(16.dp),
            )
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            identities.forEach { identity ->
                DropdownMenuItem(
                    text = {
                        Text(formatSendIdentity(identity), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    },
                    leadingIcon = {
                        if (identity.email.equals(selected.email, ignoreCase = true)) {
                            Icon(Icons.Filled.Check, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                        }
                    },
                    onClick = {
                        expanded = false
                        onSelect(identity)
                    },
                )
            }
        }
    }
}
