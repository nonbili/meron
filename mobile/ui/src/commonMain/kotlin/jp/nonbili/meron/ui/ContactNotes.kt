package jp.nonbili.meron.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch

@Composable
internal fun ContactNotes(
    state: ContactsScreenState,
    addr: String,
) {
    val note = state.note(addr)
    val scope = rememberCoroutineScope()
    val colors = MaterialTheme.colorScheme
    var preview by remember(addr) { mutableStateOf(false) }
    val noteFocusRequester = remember { FocusRequester() }
    val editorOpen = note.editing || note.dirty
    val showPreview = preview && note.text.isNotBlank()
    LaunchedEffect(note.editing, showPreview) {
        if (note.editing && !showPreview) {
            runCatching { noteFocusRequester.requestFocus() }
        }
    }
    Column(
        Modifier
            .fillMaxWidth()
            .padding(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = 28.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                tr("contacts.notes"),
                style = MaterialTheme.typography.labelMedium,
                fontWeight = FontWeight.SemiBold,
                color = colors.onSurfaceVariant,
                modifier = Modifier.weight(1f),
            )
            if (note.loaded && !editorOpen && note.text.isNotEmpty()) {
                Row(
                    Modifier
                        .clip(RoundedCornerShape(6.dp))
                        .clickable {
                            note.editing = true
                            preview = false
                        }.padding(horizontal = 8.dp, vertical = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    Icon(
                        Icons.Filled.Edit,
                        contentDescription = null,
                        tint = colors.onSurfaceVariant,
                        modifier = Modifier.size(13.dp),
                    )
                    Text(
                        tr("contacts.notesEdit"),
                        style = MaterialTheme.typography.labelSmall,
                        fontWeight = FontWeight.SemiBold,
                        color = colors.onSurfaceVariant,
                    )
                }
            }
        }
        when {
            !note.loaded && note.error -> {
                ContactFailure { scope.launch { state.loadNote(addr) } }
            }

            !note.loaded -> {
                ContactLoading()
            }

            editorOpen -> {
                // No WebView/HTML, image fetching or automatic links in private notes.
                Surface(
                    shape = RoundedCornerShape(12.dp),
                    color = colors.surface,
                    border = BorderStroke(1.dp, colors.outlineVariant),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Column(Modifier.fillMaxWidth()) {
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 8.dp, vertical = 6.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(4.dp),
                        ) {
                            ContactNoteTabButton(
                                label = tr("contacts.notesWrite"),
                                active = !showPreview,
                                enabled = true,
                                onClick = { preview = false },
                            )
                            ContactNoteTabButton(
                                label = tr("contacts.notesPreview"),
                                active = showPreview,
                                enabled = note.text.isNotBlank(),
                                onClick = { preview = true },
                            )
                            Spacer(Modifier.weight(1f))
                            Icon(
                                MarkdownMarkIcon,
                                contentDescription = tr("contacts.markdownHint"),
                                tint = colors.onSurfaceVariant.copy(alpha = 0.7f),
                                modifier = Modifier.padding(end = 4.dp).size(18.dp),
                            )
                        }
                        HorizontalDivider(color = colors.outlineVariant.copy(alpha = 0.5f))
                        if (showPreview) {
                            Box(
                                Modifier
                                    .fillMaxWidth()
                                    .heightIn(min = 104.dp)
                                    .padding(12.dp),
                            ) {
                                ContactMarkdown(note.text)
                            }
                        } else {
                            val notesLabel = tr("contacts.notes")
                            BasicTextField(
                                value = note.text,
                                onValueChange = { note.text = it },
                                cursorBrush = SolidColor(colors.primary),
                                textStyle =
                                    MaterialTheme.typography.bodyMedium.copy(
                                        color = colors.onSurface,
                                        lineHeight = 20.sp,
                                    ),
                                modifier =
                                    Modifier
                                        .fillMaxWidth()
                                        .heightIn(min = 104.dp, max = 240.dp)
                                        .padding(12.dp)
                                        .focusRequester(noteFocusRequester)
                                        .semantics { contentDescription = notesLabel },
                                decorationBox = { innerTextField ->
                                    Box(Modifier.fillMaxWidth()) {
                                        if (note.text.isEmpty()) {
                                            Text(
                                                tr("contacts.notesPlaceholder"),
                                                style = MaterialTheme.typography.bodyMedium,
                                                color = colors.onSurfaceVariant.copy(alpha = 0.7f),
                                            )
                                        }
                                        innerTextField()
                                    }
                                },
                            )
                        }
                        HorizontalDivider(color = colors.outlineVariant.copy(alpha = 0.5f))
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .background(colors.surfaceContainer.copy(alpha = 0.5f))
                                .padding(horizontal = 8.dp, vertical = 6.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.End),
                        ) {
                            TextButton(
                                onClick = {
                                    note.text = note.saved
                                    note.editing = false
                                    note.error = false
                                    preview = false
                                },
                                enabled = !note.saving,
                                contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp),
                                modifier = Modifier.height(32.dp),
                            ) {
                                Text(tr("buttons.cancel"), style = MaterialTheme.typography.labelMedium)
                            }
                            Button(
                                onClick = { scope.launch { state.saveNote(addr) } },
                                enabled = note.dirty && !note.saving,
                                shape = RoundedCornerShape(8.dp),
                                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 4.dp),
                                colors = ButtonDefaults.buttonColors(containerColor = colors.primary, contentColor = colors.onPrimary),
                                modifier = Modifier.height(32.dp),
                            ) {
                                Text(
                                    tr(if (note.saving) "contacts.notesSaving" else "contacts.notesSave"),
                                    style = MaterialTheme.typography.labelMedium,
                                )
                            }
                        }
                    }
                }
            }

            note.text.isNotEmpty() -> {
                Surface(
                    shape = RoundedCornerShape(10.dp),
                    color = colors.surfaceContainer.copy(alpha = 0.55f),
                    border = BorderStroke(1.dp, colors.outlineVariant.copy(alpha = 0.5f)),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Box(Modifier.fillMaxWidth().padding(12.dp)) {
                        ContactMarkdown(note.text)
                    }
                }
            }

            else -> {
                Surface(
                    onClick = {
                        note.editing = true
                        preview = false
                    },
                    shape = RoundedCornerShape(10.dp),
                    color = colors.surfaceContainer.copy(alpha = 0.4f),
                    border = BorderStroke(1.dp, colors.outlineVariant.copy(alpha = 0.7f)),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 12.dp, vertical = 10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Icon(
                            Icons.Filled.Add,
                            contentDescription = null,
                            tint = colors.onSurfaceVariant,
                            modifier = Modifier.size(16.dp),
                        )
                        Text(
                            tr("contacts.notesPlaceholder"),
                            style = MaterialTheme.typography.bodyMedium,
                            color = colors.onSurfaceVariant,
                        )
                    }
                }
            }
        }
        if (note.loaded && note.error) {
            Text(
                tr("contacts.notesSaveError"),
                style = MaterialTheme.typography.bodySmall,
                color = colors.error,
            )
        }
    }
}

@Composable
private fun ContactNoteTabButton(
    label: String,
    active: Boolean,
    enabled: Boolean,
    onClick: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    Box(
        Modifier
            .clip(RoundedCornerShape(6.dp))
            .background(if (active) colors.surfaceContainerHigh else colors.surface)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 10.dp, vertical = 5.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            label,
            style = MaterialTheme.typography.labelMedium,
            fontWeight = if (active) FontWeight.SemiBold else FontWeight.Medium,
            color =
                when {
                    !enabled -> colors.onSurfaceVariant.copy(alpha = 0.4f)
                    active -> colors.onSurface
                    else -> colors.onSurfaceVariant
                },
        )
    }
}

/** The Markdown mark (CC0, dcurtis/markdown-mark), matching desktop's MarkdownIcon. */
private val MarkdownMarkIcon: ImageVector by lazy {
    ImageVector
        .Builder(
            name = "MarkdownMark",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            val stroke = SolidColor(androidx.compose.ui.graphics.Color.Black)
            path(
                stroke = stroke,
                strokeLineWidth = 2f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            ) {
                // Rounded rect x=2, y=5, width=20, height=14, rx=2
                moveTo(4f, 5f)
                lineTo(20f, 5f)
                quadTo(22f, 5f, 22f, 7f)
                lineTo(22f, 17f)
                quadTo(22f, 19f, 20f, 19f)
                lineTo(4f, 19f)
                quadTo(2f, 19f, 2f, 17f)
                lineTo(2f, 7f)
                quadTo(2f, 5f, 4f, 5f)
                close()
                // "M": M6 15V9l3 3 3-3v6
                moveTo(6f, 15f)
                lineTo(6f, 9f)
                lineTo(9f, 12f)
                lineTo(12f, 9f)
                lineTo(12f, 15f)
                // Down arrow: M17 9v6m-2-2 2 2 2-2
                moveTo(17f, 9f)
                lineTo(17f, 15f)
                moveTo(15f, 13f)
                lineTo(17f, 15f)
                lineTo(19f, 13f)
            }
        }.build()
}
