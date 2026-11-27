package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.DoneAll
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.Rule

/** Every action, in the order a rule's rows show them. */
internal val RULE_ACTIONS = listOf("tags", "markRead", "star", "copy", "forward", "move")

/** What a rule for sent mail can't do: it leaves the copy in Sent, read, and sends nothing. */
internal val NOT_FOR_SENT = listOf("markRead", "forward", "move")

/** The actions a rule takes, which are the rows its editor opens with. */
internal fun ruleActions(
    rule: Rule,
    tags: String,
): List<String> =
    RULE_ACTIONS.filter { action ->
        when (action) {
            "tags" -> tags.isNotBlank()
            "markRead" -> rule.markRead
            "star" -> rule.star
            "copy" -> rule.copyFolder.isNotEmpty()
            "forward" -> rule.forwardTo.isNotBlank()
            else -> rule.moveTo.isNotEmpty()
        }
    }

@Composable
private fun actionLabel(action: String): String =
    when (action) {
        "tags" -> tr("rules.addTags")
        "markRead" -> tr("notification.actionMarkRead")
        "star" -> tr("chat.star")
        "copy" -> tr("rules.copyTo")
        "forward" -> tr("rules.forwardTo")
        else -> tr("rules.moveTo")
    }

/**
 * One row per action the rule takes, as conditions have, so a rule shows what
 * it does rather than every action it could take. A row added but left without
 * a value is no action at all.
 */
@Composable
internal fun MeronMobileState.RuleActionsEditor(
    draft: Rule,
    tags: String,
    shown: List<String>,
    accounts: List<AccountSummary>,
    onChange: (draft: Rule, tags: String, shown: List<String>) -> Unit,
) {
    // The row just asked for, whose picker opens at once.
    var added by remember { mutableStateOf<String?>(null) }
    var menu by remember { mutableStateOf(false) }
    val available = RULE_ACTIONS.filterNot { draft.sent && it in NOT_FOR_SENT }
    val rows = available.filter { it in shown }
    val addable = available.filterNot { it in shown }
    val count = rows.size + if (addable.isEmpty()) 0 else 1
    var at = 0

    fun position(): ConnectedPosition {
        val index = at++
        return when {
            count == 1 -> ConnectedPosition.Single
            index == 0 -> ConnectedPosition.First
            index == count - 1 -> ConnectedPosition.Last
            else -> ConnectedPosition.Middle
        }
    }

    fun remove(action: String) {
        val rest = shown - action
        when (action) {
            "tags" -> onChange(draft, "", rest)
            "markRead" -> onChange(draft.copy(markRead = false), tags, rest)
            "star" -> onChange(draft.copy(star = false), tags, rest)
            "copy" -> onChange(draft.copy(copyAccount = "", copyFolder = ""), tags, rest)
            "forward" -> onChange(draft.copy(forwardTo = ""), tags, rest)
            else -> onChange(draft.copy(moveTo = "", folder = "", folderAccount = ""), tags, rest)
        }
    }

    fun add(action: String) {
        menu = false
        added = action
        // Mark as read and Star have no value: the row is the action.
        val next =
            when (action) {
                "markRead" -> draft.copy(markRead = true)
                "star" -> draft.copy(star = true)
                else -> draft
            }
        onChange(next, tags, shown + action)
    }

    rows.forEach { action ->
        key(action) {
            Surface(
                shape = connectedShape(position()),
                color = MaterialTheme.colorScheme.surface,
                tonalElevation = 1.dp,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp, top = 6.dp, bottom = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(Modifier.weight(1f)) {
                        when (action) {
                            "tags" -> {
                                OutlinedTextField(
                                    value = tags,
                                    onValueChange = { onChange(draft, it, shown) },
                                    singleLine = true,
                                    label = { Text(tr("tags.title")) },
                                    placeholder = { Text(tr("rules.tags")) },
                                    modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                                )
                            }

                            "markRead", "star" -> {
                                Row(verticalAlignment = Alignment.CenterVertically) {
                                    Icon(
                                        if (action == "star") Icons.Filled.Star else Icons.Filled.DoneAll,
                                        contentDescription = null,
                                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                    Spacer(Modifier.width(12.dp))
                                    Text(actionLabel(action), style = MaterialTheme.typography.bodyLarge)
                                }
                            }

                            "copy" -> {
                                RuleTargetRow(
                                    title = actionLabel(action),
                                    value =
                                        if (draft.copyFolder.isEmpty()) {
                                            tr("rules.chooseFolder")
                                        } else {
                                            targetLabel(accounts, draft.copyAccount, draft.copyFolder)
                                        },
                                    accounts = accounts,
                                    options = emptyList(),
                                    autoOpen = added == action,
                                ) { accountId, folder -> onChange(draft.copy(copyAccount = accountId, copyFolder = folder), tags, shown) }
                            }

                            "forward" -> {
                                Box(Modifier.padding(vertical = 4.dp)) {
                                    SearchOptionsAddressField(this@RuleActionsEditor, "", actionLabel(action), draft.forwardTo, {}) {
                                        onChange(draft.copy(forwardTo = it), tags, shown)
                                    }
                                }
                            }

                            else -> {
                                RuleTargetRow(
                                    title = actionLabel(action),
                                    value = moveLabel(accounts, draft),
                                    accounts = accounts,
                                    options =
                                        listOf("archive", "trash").map { option ->
                                            moveLabel(accounts, Rule(moveTo = option)) to {
                                                onChange(draft.copy(moveTo = option, folder = "", folderAccount = ""), tags, shown)
                                            }
                                        },
                                    autoOpen = added == action,
                                ) { accountId, folder ->
                                    onChange(draft.copy(moveTo = "folder", folder = folder, folderAccount = accountId), tags, shown)
                                }
                            }
                        }
                    }
                    IconButton(onClick = { remove(action) }) {
                        Icon(
                            Icons.Filled.Close,
                            contentDescription = "${tr("rules.removeAction")}: ${actionLabel(action)}",
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
            Spacer(Modifier.height(CONNECTED_GAP))
        }
    }
    if (addable.isNotEmpty()) {
        Box {
            Surface(
                onClick = { menu = true },
                shape = connectedShape(position()),
                color = MaterialTheme.colorScheme.surface,
                tonalElevation = 1.dp,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(
                        Icons.Filled.Add,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.size(20.dp),
                    )
                    Spacer(Modifier.width(16.dp))
                    Text(
                        tr("rules.addAction"),
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.primary,
                    )
                }
            }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                addable.forEach { action ->
                    DropdownMenuItem(text = { Text(actionLabel(action)) }, onClick = { add(action) })
                }
            }
        }
    }
}
