package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.TagCount
import jp.nonbili.meron.shared.ThreadSummary
import kotlinx.coroutines.launch

private const val MAX_ROW_TAGS = 2

/** A thread's tags in a list row: the first few, then a count of the rest. */
@Composable
internal fun MailRowTagChips(
    tags: List<String>,
    modifier: Modifier = Modifier,
) {
    if (tags.isEmpty()) return
    Row(
        modifier,
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        tags.take(MAX_ROW_TAGS).forEach { tag ->
            TagChip(tag, Modifier.widthIn(max = 96.dp).weight(1f, fill = false))
        }
        if (tags.size > MAX_ROW_TAGS) {
            Text(
                "+${tags.size - MAX_ROW_TAGS}",
                fontSize = 11.sp,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun TagChip(
    text: String,
    modifier: Modifier = Modifier,
) {
    Text(
        text,
        fontSize = 11.sp,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
        color = MaterialTheme.colorScheme.onSecondaryContainer,
        modifier =
            modifier
                .clip(RoundedCornerShape(6.dp))
                .background(MaterialTheme.colorScheme.secondaryContainer)
                .padding(horizontal = 6.dp, vertical = 2.dp),
    )
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun MeronMobileState.TagDialog(
    thread: ThreadSummary,
    onDismiss: () -> Unit,
) {
    var tags by remember(thread.id) { mutableStateOf(thread.tags) }
    var known by remember { mutableStateOf(emptyList<String>()) }
    var input by remember { mutableStateOf("") }
    var invalid by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { known = loadKnownTags() }

    fun change(
        add: List<String> = emptyList(),
        remove: List<String> = emptyList(),
    ) {
        changeThreadTags(thread, add, remove) { next ->
            invalid = next == null
            if (next != null) {
                tags = next
                input = ""
            }
        }
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("tags.title")) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    tags.forEach { tag ->
                        TagChip("$tag  \u00d7", Modifier.clickable { change(remove = listOf(tag)) })
                    }
                }
                OutlinedTextField(
                    value = input,
                    onValueChange = {
                        input = it
                        invalid = false
                    },
                    singleLine = true,
                    isError = invalid,
                    placeholder = { Text(tr("tags.placeholder")) },
                    supportingText = if (invalid) ({ Text(tr("tags.invalid")) }) else null,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                    keyboardActions =
                        KeyboardActions(onDone = { if (input.isNotBlank()) change(add = listOf(input)) }),
                    modifier = Modifier.fillMaxWidth(),
                )
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    val query = input.trim().lowercase()
                    known.filter { it !in tags && query in it }.take(12).forEach { tag ->
                        TextButton(onClick = { change(add = listOf(tag)) }) { Text(tag) }
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.close")) } },
    )
}

/** Every tag in use, each to rename or delete across all mail on this device. */
@Composable
internal fun MeronMobileState.TagsDialog(onDismiss: () -> Unit) {
    var tags by remember { mutableStateOf(emptyList<TagCount>()) }
    var editing by remember { mutableStateOf<String?>(null) }
    var name by remember { mutableStateOf("") }
    var deleting by remember { mutableStateOf<TagCount?>(null) }
    var busy by remember { mutableStateOf(false) }
    var failed by remember { mutableStateOf(false) }
    var filter by remember { mutableStateOf("") }
    val coroutines = rememberCoroutineScope()
    LaunchedEffect(Unit) { tags = loadTagCounts() }

    fun replace(
        tag: String,
        to: String?,
    ) {
        if (busy) return
        busy = true
        replaceTag(tag, to) { done ->
            coroutines.launch {
                if (done) {
                    tags = loadTagCounts()
                    editing = null
                }
                failed = !done
                busy = false
            }
        }
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("tags.title")) },
        text = {
            Column(
                Modifier.verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Text(tr("tags.manage.hint"), fontSize = 12.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
                if (tags.isNotEmpty()) {
                    OutlinedTextField(
                        value = filter,
                        onValueChange = { filter = it },
                        singleLine = true,
                        placeholder = { Text(tr("tags.manage.filter")) },
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                val query = filter.trim().lowercase()
                tags.filter { query in it.tag }.forEach { entry ->
                    if (editing == entry.tag) {
                        OutlinedTextField(
                            value = name,
                            onValueChange = {
                                name = it
                                failed = false
                            },
                            singleLine = true,
                            isError = failed,
                            supportingText = if (failed) ({ Text(tr("tags.invalid")) }) else null,
                            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                            keyboardActions = KeyboardActions(onDone = { if (name.isNotBlank()) replace(entry.tag, name) }),
                            modifier = Modifier.fillMaxWidth(),
                        )
                        Row {
                            TextButton(enabled = !busy && name.isNotBlank(), onClick = { replace(entry.tag, name) }) {
                                Text(tr("buttons.save"))
                            }
                            TextButton(onClick = { editing = null }) { Text(tr("buttons.cancel")) }
                        }
                    } else {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(
                                "${entry.tag} (${entry.count})",
                                fontSize = 13.sp,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                                modifier = Modifier.weight(1f),
                            )
                            TextButton(
                                enabled = !busy,
                                onClick = {
                                    name = entry.tag
                                    failed = false
                                    editing = entry.tag
                                },
                            ) { Text(tr("buttons.edit")) }
                            TextButton(enabled = !busy, onClick = { deleting = entry }) { Text(tr("buttons.delete")) }
                        }
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.close")) } },
    )

    deleting?.let { entry ->
        AlertDialog(
            onDismissRequest = { deleting = null },
            title = { Text(tr("tags.manage.delete")) },
            text = { Text(tr("tags.manage.deleteConfirm", mapOf("tag" to entry.tag, "count" to entry.count))) },
            confirmButton = {
                TextButton(
                    onClick = {
                        deleting = null
                        replace(entry.tag, null)
                    },
                ) { Text(tr("buttons.delete")) }
            },
            dismissButton = { TextButton(onClick = { deleting = null }) { Text(tr("buttons.cancel")) } },
        )
    }
}
