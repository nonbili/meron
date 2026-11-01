package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.DoneAll
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
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
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.Rule
import kotlinx.coroutines.launch

private val NEW_RULE = Rule(query = "")

@Composable
private fun moveLabel(moveTo: String): String =
    when (moveTo) {
        "archive" -> tr("folders.roles.archive")
        "trash" -> tr("folders.roles.trash")
        "folder" -> tr("rules.moveFolder")
        else -> tr("rules.moveNone")
    }

@Composable
private fun ruleSummary(rule: Rule): String =
    listOfNotNull(
        rule.tags.joinToString(", ").takeIf { it.isNotEmpty() },
        tr("notification.actionMarkRead").takeIf { rule.markRead },
        tr("chat.star").takeIf { rule.star },
        (if (rule.moveTo == "folder") rule.folder else moveLabel(rule.moveTo)).takeIf { rule.moveTo.isNotEmpty() },
    ).joinToString(" · ")

/** What to do with new mail that matches a search, in the search box's own syntax. */
@Composable
internal fun MeronMobileState.RulesPage(modifier: Modifier = Modifier) {
    // Each save replaces the whole list, so nothing may start from a stale one:
    // not before the list has loaded, and not while another save is on its way.
    var rules by remember { mutableStateOf<List<Rule>?>(null) }
    var saving by remember { mutableStateOf(false) }
    var draft by remember { mutableStateOf(NEW_RULE) }
    var tags by remember { mutableStateOf("") }
    var moveMenuOpen by remember { mutableStateOf(false) }
    var invalid by remember { mutableStateOf(false) }
    val coroutines = rememberCoroutineScope()
    LaunchedEffect(Unit) { rules = loadRules() }

    fun reset() {
        draft = NEW_RULE
        tags = ""
        invalid = false
    }

    fun save(
        next: List<Rule>,
        onSaved: () -> Unit,
    ) {
        if (saving) return
        saving = true
        coroutines.launch {
            val saved = saveRules(next)
            saving = false
            invalid = saved == null
            if (saved != null) {
                rules = saved
                onSaved()
            }
        }
    }

    fun change(next: Rule) {
        draft = next
        invalid = false
    }

    val editing = draft.id.isNotEmpty()
    Column(
        modifier.verticalScroll(rememberScrollState()).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(tr("rules.hint"), fontSize = 12.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
        if (rules?.isEmpty() == true) {
            Text(tr("rules.empty"), fontSize = 13.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        rules.orEmpty().forEach { rule ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(rule.query, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(
                        ruleSummary(rule),
                        fontSize = 12.sp,
                        color = MaterialTheme.colorScheme.primary,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                TextButton(
                    enabled = !saving,
                    onClick = {
                        draft = rule
                        tags = rule.tags.joinToString(", ")
                        invalid = false
                    },
                ) { Text(tr("buttons.edit")) }
                TextButton(
                    enabled = !saving,
                    onClick = {
                        save(rules.orEmpty().filter { it.id != rule.id }) {
                            if (draft.id == rule.id) reset()
                        }
                    },
                ) { Text(tr("buttons.delete")) }
            }
        }

        Text(if (editing) tr("rules.edit") else tr("rules.add"), style = MaterialTheme.typography.titleSmall)
        OutlinedTextField(
            value = draft.query,
            onValueChange = { change(draft.copy(query = it)) },
            singleLine = true,
            label = { Text(tr("rules.query")) },
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = tags,
            onValueChange = {
                tags = it
                invalid = false
            },
            singleLine = true,
            label = { Text(tr("rules.tags")) },
            modifier = Modifier.fillMaxWidth(),
        )
        SettingsToggleRow(
            icon = Icons.Filled.DoneAll,
            title = tr("notification.actionMarkRead"),
            checked = draft.markRead,
            onToggle = { change(draft.copy(markRead = !draft.markRead)) },
        )
        SettingsToggleRow(
            icon = Icons.Filled.Star,
            title = tr("chat.star"),
            checked = draft.star,
            onToggle = { change(draft.copy(star = !draft.star)) },
        )
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(tr("rules.moveTo"), Modifier.weight(1f))
            TextButton(onClick = { moveMenuOpen = true }) { Text(moveLabel(draft.moveTo)) }
            DropdownMenu(expanded = moveMenuOpen, onDismissRequest = { moveMenuOpen = false }) {
                listOf("", "archive", "trash", "folder").forEach { option ->
                    DropdownMenuItem(
                        text = { Text(moveLabel(option)) },
                        onClick = {
                            moveMenuOpen = false
                            change(draft.copy(moveTo = option))
                        },
                    )
                }
            }
        }
        if (draft.moveTo == "folder") {
            OutlinedTextField(
                value = draft.folder,
                onValueChange = { change(draft.copy(folder = it)) },
                singleLine = true,
                label = { Text(tr("rules.folder")) },
                modifier = Modifier.fillMaxWidth(),
            )
        }
        if (draft.moveTo == "trash") {
            Text(tr("rules.trashHint"), fontSize = 12.sp, color = MaterialTheme.colorScheme.error)
        }
        if (invalid) {
            Text(tr("rules.invalid"), fontSize = 12.sp, color = MaterialTheme.colorScheme.error)
        }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
            if (editing) {
                TextButton(onClick = ::reset) { Text(tr("buttons.cancel")) }
            }
            TextButton(
                enabled = !saving && rules != null,
                onClick = {
                    val rule = draft.copy(tags = tags.split(",").map(String::trim).filter(String::isNotEmpty))
                    val current = rules.orEmpty()
                    save(if (editing) current.map { if (it.id == rule.id) rule else it } else current + rule, ::reset)
                },
            ) { Text(if (editing) tr("buttons.save") else tr("rules.add")) }
        }
    }
}
