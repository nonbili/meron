package jp.nonbili.meron.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.DoneAll
import androidx.compose.material.icons.filled.DragHandle
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.FilterList
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.Rule
import jp.nonbili.meron.shared.RuleStatus
import jp.nonbili.meron.shared.SearchFields
import jp.nonbili.meron.shared.accountSummaryIsRss
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.launch
import sh.calvin.reorderable.ReorderableColumn

private val NEW_RULE = Rule(query = "")

private const val RUN_POLL_MS = 1500L
private const val ACTIVITY_SHOWN = 20

// "now" is arrival with no delay, kept apart so that choosing "After it
// arrives" doesn't fall back to it while the amount is still empty.
private val TIMINGS = listOf("now", "arrival", "read", "age")
private val UNITS = listOf("minutes", "hours", "days")
private val UNIT_SECONDS = mapOf("minutes" to 60L, "hours" to 3600L, "days" to 86400L)
private const val MAX_DELAY_SECONDS = 315_360_000L

/** The largest unit that holds the delay whole, so 30 days isn't shown as 43200 minutes. */
private fun splitDelay(seconds: Long): Pair<String, String> {
    val unit = listOf("days", "hours").firstOrNull { seconds > 0 && seconds % UNIT_SECONDS.getValue(it) == 0L } ?: "minutes"
    return (seconds.toDouble() / UNIT_SECONDS.getValue(unit)).toString().removeSuffix(".0") to unit
}

/** The delay in seconds, or null while the amount isn't a usable one. */
private fun delaySeconds(
    timing: String,
    amount: String,
    unit: String,
): Long? {
    if (timing == "now") return 0
    val value = amount.trim().toDoubleOrNull()?.takeIf { it.isFinite() && it >= 0 } ?: return null
    return kotlin.math
        .round(value * UNIT_SECONDS.getValue(unit))
        .toLong()
        .takeIf { it <= MAX_DELAY_SECONDS }
}

/** What the settings bar shows while a rule is open: its title, and how back closes it. */
internal class RuleEditorBar(
    val title: String,
    val close: () -> Unit,
)

private enum class ConnectedPosition {
    Single,
    First,
    Middle,
    Last,
}

private fun connectedShape(
    position: ConnectedPosition,
    corner: Dp = 16.dp,
    innerCorner: Dp = 4.dp,
): RoundedCornerShape =
    when (position) {
        ConnectedPosition.Single -> RoundedCornerShape(corner)
        ConnectedPosition.First -> RoundedCornerShape(topStart = corner, topEnd = corner, bottomStart = innerCorner, bottomEnd = innerCorner)
        ConnectedPosition.Middle -> RoundedCornerShape(innerCorner)
        ConnectedPosition.Last -> RoundedCornerShape(topStart = innerCorner, topEnd = innerCorner, bottomStart = corner, bottomEnd = corner)
    }

private val CONNECTED_GAP = 2.dp

private fun accountLabel(account: AccountSummary): String = account.displayName.ifEmpty { account.email }

/** How a rule's target reads: its account too when there is more than one. */
private fun MeronMobileState.targetLabel(
    accounts: List<AccountSummary>,
    accountId: String,
    folder: String,
): String {
    val name = foldersByAccount[accountId]?.firstOrNull { it.name == folder }?.displayName ?: folder
    val account = accounts.firstOrNull { it.id == accountId }
    return if (accounts.size > 1 && account != null) "${accountLabel(account)} / $name" else name
}

@Composable
private fun MeronMobileState.moveLabel(
    accounts: List<AccountSummary>,
    rule: Rule,
): String =
    when (rule.moveTo) {
        "archive" -> tr("folders.roles.archive")
        "trash" -> tr("folders.roles.trash")
        "folder" -> if (rule.folderAccount.isEmpty()) rule.folder else targetLabel(accounts, rule.folderAccount, rule.folder)
        else -> tr("rules.moveNone")
    }

@Composable
private fun MeronMobileState.ruleSummary(
    accounts: List<AccountSummary>,
    rule: Rule,
): String =
    listOfNotNull(
        tr("rules.sentMail").takeIf { rule.sent },
        if (rule.trigger == "arrival" && rule.delaySeconds == 0L) {
            null
        } else {
            val (amount, unit) = splitDelay(rule.delaySeconds)
            tr("rules.summary.${rule.trigger}", mapOf("duration" to tr("rules.duration.$unit", mapOf("count" to amount))))
        },
        tr("rules.summaryTags", mapOf("tags" to rule.tags.joinToString(", "))).takeIf { rule.tags.isNotEmpty() },
        tr("notification.actionMarkRead").takeIf { rule.markRead },
        tr("chat.star").takeIf { rule.star },
        tr("rules.summaryCopy", mapOf("folder" to targetLabel(accounts, rule.copyAccount, rule.copyFolder)))
            .takeIf { rule.copyFolder.isNotEmpty() },
        tr("rules.summaryMove", mapOf("folder" to moveLabel(accounts, rule))).takeIf { rule.moveTo.isNotEmpty() },
    ).joinToString(" · ")

/** A rule's target: `options` first, then every folder of every account. */
@Composable
private fun MeronMobileState.RuleTargetRow(
    title: String,
    value: String,
    accounts: List<AccountSummary>,
    options: List<Pair<String, () -> Unit>>,
    onPick: (accountId: String, folder: String) -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    // With several accounts, the one whose folders are showing.
    var opened by remember { mutableStateOf<AccountSummary?>(null) }

    fun close() {
        open = false
        opened = null
    }
    val folderAccount = opened ?: accounts.singleOrNull()
    Box(Modifier.fillMaxWidth()) {
        Row(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(8.dp))
                    .clickable { open = true }
                    .padding(vertical = 6.dp, horizontal = 2.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                title,
                style = MaterialTheme.typography.bodyLarge,
                modifier = Modifier.weight(1f),
            )
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                Text(
                    value,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Icon(
                    Icons.Filled.ArrowDropDown,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        DropdownMenu(expanded = open, onDismissRequest = ::close) {
            if (opened != null) {
                DropdownMenuItem(text = { Text(tr("buttons.back")) }, onClick = { opened = null })
            } else {
                options.forEach { (label, pick) ->
                    DropdownMenuItem(
                        text = { Text(label) },
                        onClick = {
                            close()
                            pick()
                        },
                    )
                }
            }
            if (folderAccount != null) {
                foldersByAccount[folderAccount.id].orEmpty().forEach { folder ->
                    DropdownMenuItem(
                        text = { Text(folder.displayName) },
                        onClick = {
                            close()
                            onPick(folderAccount.id, folder.name)
                        },
                    )
                }
            } else {
                accounts.forEach { account ->
                    DropdownMenuItem(
                        text = { Text(accountLabel(account)) },
                        leadingIcon = {
                            AccountBadgeAvatar(
                                label = accountLabel(account),
                                avatarUrl = account.avatarUrl,
                                size = 20.dp,
                                fallbackIcon = accountAvatarFallbackIcon(account),
                            )
                        },
                        onClick = { opened = account },
                    )
                }
            }
        }
    }
}

/** A rule's conditions in the form's own words, not as search text. */
@Composable
private fun ruleCondition(fields: SearchFields): String {
    @Composable
    fun valued(
        key: String,
        value: String,
    ) = "${tr(key)}: ${value.trim()}".takeIf { value.isNotBlank() }
    return listOfNotNull(
        tr("rules.allMail").takeIf { fields == SearchFields() },
        valued("composer.fields.from", fields.from),
        valued("composer.fields.to", fields.to),
        valued("composer.fields.subject", fields.subject),
        valued("tags.title", fields.tags),
        valued("search.hasWords", fields.words),
        valued("search.after", fields.after),
        valued("search.before", fields.before),
        tr("filters.unread").takeIf { fields.unread },
        tr("filters.starred").takeIf { fields.starred },
        tr("filters.hasAttachment").takeIf { fields.hasAttachment },
    ).joinToString(" · ")
}

/**
 * What to do with new mail that matches some conditions, filled in as fields. A
 * rule stores them as search text, which the page never shows; conditions it has
 * no field for stay in that text across an edit.
 */
@Composable
internal fun MeronMobileState.RulesPage(
    modifier: Modifier = Modifier,
    onEditor: (RuleEditorBar?) -> Unit = {},
) {
    // Each save replaces the whole list, so nothing may start from a stale one:
    // not before the list has loaded, and not while another save is on its way.
    var rules by remember { mutableStateOf<List<Rule>?>(null) }
    var saving by remember { mutableStateOf(false) }
    var draft by remember { mutableStateOf(NEW_RULE) }
    var fields by remember { mutableStateOf(SearchFields()) }
    var conditions by remember { mutableStateOf(emptyMap<String, SearchFields>()) }
    var tags by remember { mutableStateOf("") }
    var timing by remember { mutableStateOf("now") }
    var amount by remember { mutableStateOf("") }
    var unit by remember { mutableStateOf("minutes") }
    var adding by remember { mutableStateOf(false) }
    // Apart from `draft.accounts`: turning "all" off starts with none picked,
    // which as a list would mean all of them.
    var scoped by remember { mutableStateOf(false) }
    // The rule waiting for a yes, and how much mail it would run on.
    var confirmRun by remember { mutableStateOf<Pair<Rule, Int>?>(null) }
    var confirmDelete by remember { mutableStateOf<Rule?>(null) }
    // Opening a rule waits on the core: only the last thing asked for may land.
    var opening by remember { mutableStateOf<Job?>(null) }
    var runNotice by remember { mutableStateOf("") }
    var runStarted by remember { mutableStateOf<Int?>(null) }
    var status by remember { mutableStateOf(RuleStatus()) }
    var invalid by remember { mutableStateOf(false) }
    val coroutines = rememberCoroutineScope()
    val haptics = LocalHapticFeedback.current
    LaunchedEffect(Unit) {
        rules = loadRules()
        loadRuleStatus()?.let { status = it }
    }
    // Only while a run has work left.
    val runActive = status.running.isNotEmpty()
    LaunchedEffect(runActive) {
        while (runActive) {
            delay(RUN_POLL_MS)
            loadRuleStatus()?.let { status = it }
        }
    }
    val mailAccounts = coreAccounts.filterNot { accountSummaryIsRss(it) || it.needsReconnect }
    LaunchedEffect(mailAccounts.size) { mailAccounts.forEach { ensureAccountFolders(it.id) } }
    LaunchedEffect(rules) {
        conditions = rules.orEmpty().associate { it.query to (parseSearchQuery(it.query) ?: SearchFields(words = it.query)) }
    }

    fun reset() {
        opening?.cancel()
        draft = NEW_RULE
        fields = SearchFields()
        tags = ""
        timing = "now"
        amount = ""
        unit = "minutes"
        scoped = false
        adding = false
        invalid = false
    }

    fun save(
        next: suspend () -> List<Rule>?,
        onFailed: () -> Unit = {},
        onSaved: () -> Unit,
    ) {
        if (saving) return
        saving = true
        coroutines.launch {
            val saved = next()?.let { saveRules(it) }
            saving = false
            invalid = saved == null
            if (saved != null) {
                rules = saved
                onSaved()
            } else {
                onFailed()
            }
        }
    }

    fun changeFields(next: SearchFields) {
        fields = next
        invalid = false
    }

    // Order matters: the first rule to move a message decides where it goes. The
    // new order shows at once, or a dropped row would jump back until the save.
    fun move(
        from: Int,
        to: Int,
    ) {
        val before = rules.orEmpty()
        if (saving || from == to || to !in before.indices) return
        val next = before.toMutableList().apply { add(to, removeAt(from)) }
        rules = next
        save({ next }, onFailed = { rules = before }) {}
    }

    fun change(next: Rule) {
        draft = next
        invalid = false
    }

    // A delayed choice starts from a typical amount rather than an empty, unsavable one.
    fun pickTiming(next: String) {
        if (next == "now") {
            amount = ""
        } else if (amount.isEmpty()) {
            amount = if (next == "age") "30" else "5"
            unit = if (next == "age") "days" else "minutes"
        }
        timing = next
        invalid = false
    }

    val failed = tr("rules.runFailed")
    val nothing = tr("rules.runNothing")
    val nothingSent = tr("rules.runSentNothing")
    confirmRun?.let { (rule, count) ->
        val message =
            when {
                rule.sent -> "rules.runSentConfirm"
                rule.moveTo == "trash" -> "rules.runTrashConfirm"
                else -> "rules.runConfirm"
            }
        AlertDialog(
            onDismissRequest = { confirmRun = null },
            title = { Text(tr("rules.run")) },
            text = { Text(tr(message, mapOf("count" to count.toString()))) },
            confirmButton = {
                TextButton(
                    onClick = {
                        confirmRun = null
                        coroutines.launch {
                            val ran = runRule(rule.id)
                            if (ran == null) runNotice = failed
                            runStarted = ran
                            loadRuleStatus()?.let { status = it }
                        }
                    },
                ) { Text(tr("rules.runAction")) }
            },
            dismissButton = { TextButton(onClick = { confirmRun = null }) { Text(tr("buttons.cancel")) } },
        )
    }

    val editing = draft.id.isNotEmpty()
    val chosenAccounts = draft.accounts.filter { id -> coreAccounts.any { it.id == id } }
    confirmDelete?.let { rule ->
        AlertDialog(
            onDismissRequest = { confirmDelete = null },
            title = { Text(tr("rules.delete")) },
            text = {
                val condition = conditions[rule.query]?.let { ruleCondition(it) } ?: rule.query.ifEmpty { tr("rules.allMail") }
                Text("$condition\n${tr("rules.deleteConfirm")}")
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        confirmDelete = null
                        save({ rules.orEmpty().filter { it.id != rule.id } }) {
                            if (draft.id == rule.id) reset()
                        }
                    },
                ) { Text(tr("buttons.delete")) }
            },
            dismissButton = { TextButton(onClick = { confirmDelete = null }) { Text(tr("buttons.cancel")) } },
        )
    }
    val formOpen = adding || editing
    // Adding or editing takes over the page, with the settings bar's back closing it.
    val editorTitle = if (formOpen) tr(if (editing) "rules.edit" else "rules.add") else null
    LaunchedEffect(editorTitle) { onEditor(editorTitle?.let { RuleEditorBar(it) { reset() } }) }
    DisposableEffect(Unit) { onDispose { onEditor(null) } }
    val delay = delaySeconds(timing, amount, unit)
    val scroll = rememberScrollState()
    // The editor and the list are different pages: neither opens partway down.
    LaunchedEffect(formOpen) { scroll.scrollTo(0) }
    val openEdit: (Rule) -> Unit = { rule ->
        opening?.cancel()
        opening =
            coroutines.launch {
                val parsed = parseSearchQuery(rule.query)
                // Its failure handling also swallows being cancelled.
                ensureActive()
                fields = parsed ?: SearchFields(words = rule.query)
                draft = rule
                val (ruleAmount, ruleUnit) = splitDelay(rule.delaySeconds)
                timing = if (rule.trigger == "arrival" && rule.delaySeconds == 0L) "now" else rule.trigger
                amount = if (timing == "now") "" else ruleAmount
                unit = ruleUnit
                tags = rule.tags.joinToString(", ")
                scoped = rule.accounts.isNotEmpty()
                adding = false
                invalid = false
            }
    }

    Column(
        modifier.verticalScroll(scroll).padding(horizontal = 16.dp).padding(top = if (formOpen) 0.dp else 8.dp, bottom = 16.dp),
        // Connected rows supply their own thin gaps; page spacing would also
        // be inserted on both sides of every divider Spacer.
        verticalArrangement = Arrangement.spacedBy(if (formOpen) 0.dp else 10.dp),
    ) {
        if (!formOpen) {
            if (rules?.isEmpty() == true) {
                Surface(
                    shape = RoundedCornerShape(12.dp),
                    color = MaterialTheme.colorScheme.surface,
                    tonalElevation = 1.dp,
                    modifier = Modifier.fillMaxWidth().padding(vertical = 12.dp),
                ) {
                    Column(
                        modifier = Modifier.fillMaxWidth().padding(24.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Icon(
                            Icons.Filled.FilterList,
                            contentDescription = null,
                            modifier = Modifier.size(40.dp),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f),
                        )
                        Text(
                            tr("rules.empty"),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
            val notice = runStarted?.let { tr("rules.runStarted", mapOf("count" to it.toString())) } ?: runNotice
            if (notice.isNotEmpty()) {
                Surface(
                    shape = RoundedCornerShape(8.dp),
                    color = MaterialTheme.colorScheme.primaryContainer,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(
                        notice,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onPrimaryContainer,
                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
                    )
                }
            }
            val known = { id: String -> coreAccounts.any { it.id == id } }
            // The labels are read here: accessibility actions run outside composition.
            val moveUp = tr("rules.moveUp")
            val moveDown = tr("rules.moveDown")
            val reorder = tr("rules.reorder")
            ReorderableColumn(
                list = rules.orEmpty(),
                onSettle = ::move,
                onMove = { haptics.performHapticFeedback(HapticFeedbackType.SegmentFrequentTick) },
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) { at, rule, dragging ->
                key(rule.id) {
                    val left = status.running[rule.id]
                    val failure = status.log.firstOrNull { it.ruleId == rule.id }?.takeIf { it.failed }
                    // A rule keeps a removed account until edited, so that it says what
                    // is wrong instead of quietly becoming a different rule.
                    val lost = (rule.accounts + rule.copyAccount + rule.folderAccount).any { it.isNotEmpty() && !known(it) }
                    ReorderableItem {
                        Column(modifier = Modifier.fillMaxWidth()) {
                            Surface(
                                shape = connectedShape(ConnectedPosition.First),
                                color = if (dragging) MaterialTheme.colorScheme.surfaceContainerHigh else MaterialTheme.colorScheme.surface,
                                tonalElevation = if (dragging) 6.dp else 1.dp,
                                shadowElevation = if (dragging) 4.dp else 0.dp,
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Row(
                                    modifier = Modifier.fillMaxWidth().padding(start = 6.dp, end = 14.dp, top = 8.dp, bottom = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    Icon(
                                        Icons.Filled.DragHandle,
                                        contentDescription = null,
                                        tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
                                        modifier =
                                            Modifier
                                                .draggableHandle(
                                                    enabled = !saving,
                                                    onDragStarted = {
                                                        haptics.performHapticFeedback(HapticFeedbackType.GestureThresholdActivate)
                                                    },
                                                ).semantics {
                                                    contentDescription = reorder
                                                    // A drag can't be performed from a screen reader.
                                                    customActions =
                                                        listOfNotNull(
                                                            CustomAccessibilityAction(moveUp) {
                                                                move(at, at - 1)
                                                                true
                                                            }.takeIf { at > 0 },
                                                            CustomAccessibilityAction(moveDown) {
                                                                move(at, at + 1)
                                                                true
                                                            }.takeIf { at < rules.orEmpty().lastIndex },
                                                        )
                                                }.padding(start = 4.dp, end = 8.dp, top = 8.dp, bottom = 8.dp),
                                    )
                                    Column(
                                        modifier =
                                            Modifier
                                                .weight(1f)
                                                .clip(RoundedCornerShape(8.dp))
                                                .clickable(enabled = !saving) { openEdit(rule) }
                                                .padding(vertical = 2.dp),
                                        verticalArrangement = Arrangement.spacedBy(3.dp),
                                    ) {
                                        Row(
                                            horizontalArrangement = Arrangement.spacedBy(6.dp),
                                            verticalAlignment = Alignment.CenterVertically,
                                        ) {
                                            rule.accounts.forEach { id ->
                                                val account = coreAccounts.firstOrNull { it.id == id }
                                                AccountBadgeAvatar(
                                                    label = account?.let(::accountLabel) ?: id,
                                                    avatarUrl = account?.avatarUrl.orEmpty(),
                                                    size = 18.dp,
                                                )
                                            }
                                            Text(
                                                conditions[rule.query]?.let { ruleCondition(it) } ?: rule.query.ifEmpty { tr("rules.allMail") },
                                                style = MaterialTheme.typography.titleSmall,
                                                maxLines = 1,
                                                overflow = TextOverflow.Ellipsis,
                                            )
                                        }
                                        Text(
                                            ruleSummary(mailAccounts, rule),
                                            style = MaterialTheme.typography.bodySmall,
                                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                                            maxLines = 1,
                                            overflow = TextOverflow.Ellipsis,
                                        )
                                        if (left != null) {
                                            Text(
                                                tr("rules.runLeft", mapOf("count" to left.toString())),
                                                style = MaterialTheme.typography.labelSmall,
                                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                            )
                                        }
                                        if (lost) {
                                            Text(
                                                tr("rules.accountGone"),
                                                style = MaterialTheme.typography.labelSmall,
                                                color = MaterialTheme.colorScheme.error,
                                            )
                                        }
                                        if (failure != null) {
                                            Text(
                                                "${tr("rules.lastFailed")}: ${failure.detail}",
                                                style = MaterialTheme.typography.labelSmall,
                                                color = MaterialTheme.colorScheme.error,
                                                maxLines = 2,
                                                overflow = TextOverflow.Ellipsis,
                                            )
                                        }
                                    }
                                    Switch(
                                        checked = !rule.disabled,
                                        enabled = !saving && !(rule.disabled && lost),
                                        onCheckedChange = { on ->
                                            save({ rules.orEmpty().map { if (it.id == rule.id) it.copy(disabled = !on) else it } }) {}
                                        },
                                        colors = settingsSwitchColors(),
                                        modifier = Modifier.padding(start = 8.dp),
                                    )
                                }
                            }
                            Spacer(Modifier.height(CONNECTED_GAP))
                            Surface(
                                shape = connectedShape(ConnectedPosition.Last),
                                color = if (dragging) MaterialTheme.colorScheme.surfaceContainerHigh else MaterialTheme.colorScheme.surface,
                                tonalElevation = if (dragging) 6.dp else 1.dp,
                                shadowElevation = if (dragging) 4.dp else 0.dp,
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Row(
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.End,
                                    modifier = Modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 2.dp),
                                ) {
                                    if (left != null) {
                                        TextButton(
                                            onClick = {
                                                coroutines.launch {
                                                    if (!stopRule(rule.id)) runNotice = failed
                                                    loadRuleStatus()?.let { status = it }
                                                }
                                            },
                                        ) {
                                            Icon(Icons.Filled.Stop, contentDescription = null, modifier = Modifier.size(16.dp))
                                            Spacer(Modifier.width(4.dp))
                                            Text(tr("rules.stop"))
                                        }
                                    } else {
                                        TextButton(
                                            enabled = !saving && !rule.disabled,
                                            onClick = {
                                                coroutines.launch {
                                                    val count = ruleMatches(rule.id)
                                                    runStarted = null
                                                    when (count) {
                                                        null -> runNotice = failed
                                                        0 -> runNotice = if (rule.sent) nothingSent else nothing
                                                        else -> confirmRun = rule to count
                                                    }
                                                }
                                            },
                                        ) {
                                            Icon(Icons.Filled.PlayArrow, contentDescription = null, modifier = Modifier.size(16.dp))
                                            Spacer(Modifier.width(4.dp))
                                            Text(tr("rules.runAction"))
                                        }
                                    }
                                    TextButton(
                                        enabled = !saving,
                                        onClick = { openEdit(rule) },
                                    ) {
                                        Icon(Icons.Filled.Edit, contentDescription = null, modifier = Modifier.size(16.dp))
                                        Spacer(Modifier.width(4.dp))
                                        Text(tr("buttons.edit"))
                                    }
                                    TextButton(
                                        enabled = !saving,
                                        onClick = { confirmDelete = rule },
                                    ) {
                                        Icon(
                                            Icons.Filled.Delete,
                                            contentDescription = null,
                                            modifier = Modifier.size(16.dp),
                                            tint = MaterialTheme.colorScheme.error,
                                        )
                                        Spacer(Modifier.width(4.dp))
                                        Text(tr("buttons.delete"), color = MaterialTheme.colorScheme.error)
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Surface(
                onClick = {
                    opening?.cancel()
                    adding = true
                },
                enabled = rules != null,
                shape = RoundedCornerShape(16.dp),
                color = MaterialTheme.colorScheme.surface,
                tonalElevation = 1.dp,
                modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(
                        Icons.Filled.Add,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSurface,
                        modifier = Modifier.size(20.dp),
                    )
                    Spacer(Modifier.width(16.dp))
                    Text(
                        tr("rules.add"),
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                }
            }
        } else {
            SettingsSectionLabel(tr("rules.when"), hint = tr("rules.queryHint"), first = true)
            if (mailAccounts.size > 1 || scoped) {
                val allPos = if (scoped) ConnectedPosition.First else ConnectedPosition.Single
                Surface(
                    shape = connectedShape(allPos),
                    color = MaterialTheme.colorScheme.surface,
                    tonalElevation = 1.dp,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    SettingsToggleRow(
                        icon = Icons.Filled.Person,
                        title = tr("kanban.allAccounts"),
                        checked = !scoped,
                        onToggle = {
                            scoped = !scoped
                            change(draft.copy(accounts = emptyList()))
                        },
                    )
                }
                if (scoped) {
                    mailAccounts.forEachIndexed { index, account ->
                        Spacer(Modifier.height(CONNECTED_GAP))
                        val picked = account.id in draft.accounts
                        val pos = if (index == mailAccounts.lastIndex) ConnectedPosition.Last else ConnectedPosition.Middle
                        Surface(
                            shape = connectedShape(pos),
                            color = MaterialTheme.colorScheme.surface,
                            tonalElevation = 1.dp,
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            SettingsToggleRow(
                                icon = Icons.Filled.Person,
                                title = accountLabel(account),
                                checked = picked,
                                onToggle = {
                                    change(draft.copy(accounts = if (picked) draft.accounts - account.id else draft.accounts + account.id))
                                },
                                leading = {
                                    AccountBadgeAvatar(
                                        label = accountLabel(account),
                                        avatarUrl = account.avatarUrl,
                                        size = 24.dp,
                                        fallbackIcon = accountAvatarFallbackIcon(account),
                                    )
                                },
                            )
                        }
                    }
                }
                Spacer(Modifier.height(16.dp))
            }

            Surface(
                shape = connectedShape(ConnectedPosition.Single),
                color = MaterialTheme.colorScheme.surface,
                tonalElevation = 1.dp,
                modifier = Modifier.fillMaxWidth(),
            ) {
                SettingsToggleRow(
                    icon = Icons.AutoMirrored.Filled.Send,
                    title = tr("rules.direction.sent"),
                    checked = draft.sent,
                    onToggle = {
                        // Sent mail stays in Sent and is acted on at once: what such a
                        // rule can't do is dropped, not just hidden, or the save is refused.
                        if (draft.sent) {
                            change(draft.copy(sent = false))
                        } else {
                            pickTiming("now")
                            change(draft.copy(sent = true, markRead = false, moveTo = "", folder = "", folderAccount = ""))
                        }
                    },
                )
            }
            if (draft.sent) {
                Text(
                    tr("rules.sentHint"),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 4.dp, vertical = 6.dp),
                )
            }
            Spacer(Modifier.height(16.dp))

            Surface(
                shape = connectedShape(ConnectedPosition.First),
                color = MaterialTheme.colorScheme.surface,
                tonalElevation = 1.dp,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Column(
                    modifier = Modifier.fillMaxWidth().padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    SearchOptionsAddressField(this@RulesPage, "", tr("composer.fields.from"), fields.from, {}) {
                        changeFields(fields.copy(from = it))
                    }
                    SearchOptionsAddressField(this@RulesPage, "", tr("composer.fields.to"), fields.to, {}) {
                        changeFields(fields.copy(to = it))
                    }
                    SearchOptionsTextField(tr("composer.fields.subject"), fields.subject, {}) {
                        changeFields(fields.copy(subject = it))
                    }
                    SearchOptionsTextField(tr("search.hasWords"), fields.words, {}) { changeFields(fields.copy(words = it)) }
                }
            }
            Spacer(Modifier.height(CONNECTED_GAP))
            Surface(
                onClick = { changeFields(fields.copy(hasAttachment = !fields.hasAttachment)) },
                shape = connectedShape(ConnectedPosition.Last),
                color = MaterialTheme.colorScheme.surface,
                tonalElevation = 1.dp,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(
                        Icons.Filled.AttachFile,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(Modifier.width(12.dp))
                    Text(
                        tr("filters.hasAttachment"),
                        style = MaterialTheme.typography.bodyLarge,
                        modifier = Modifier.weight(1f),
                    )
                    Switch(
                        checked = fields.hasAttachment,
                        onCheckedChange = { changeFields(fields.copy(hasAttachment = it)) },
                        colors = settingsSwitchColors(),
                    )
                }
            }

            if (!draft.sent) {
                SettingsSectionLabel(tr("rules.timing"), hint = tr("rules.timingHint"))
                Surface(
                    shape = connectedShape(if (timing == "now") ConnectedPosition.Single else ConnectedPosition.First),
                    color = MaterialTheme.colorScheme.surface,
                    tonalElevation = 1.dp,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Box(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp)) {
                        RuleTargetRow(
                            title = tr("rules.runWhen"),
                            value = tr("rules.timing.$timing"),
                            accounts = emptyList(),
                            options = TIMINGS.map { option -> tr("rules.timing.$option") to { pickTiming(option) } },
                            onPick = { _, _ -> },
                        )
                    }
                }
                if (timing != "now") {
                    Spacer(Modifier.height(CONNECTED_GAP))
                    Surface(
                        shape = connectedShape(ConnectedPosition.Last),
                        color = MaterialTheme.colorScheme.surface,
                        tonalElevation = 1.dp,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Row(
                            modifier = Modifier.fillMaxWidth().padding(16.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            OutlinedTextField(
                                value = amount,
                                onValueChange = {
                                    amount = it
                                    invalid = false
                                },
                                isError = delay == null,
                                singleLine = true,
                                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                                label = { Text(tr(if (timing == "age") "rules.age" else "rules.wait")) },
                                modifier = Modifier.weight(1f),
                            )
                            var unitMenu by remember { mutableStateOf(false) }
                            Box {
                                OutlinedButton(
                                    onClick = { unitMenu = true },
                                    shape = RoundedCornerShape(8.dp),
                                    modifier = Modifier.height(56.dp),
                                ) {
                                    Text(tr("rules.unit.$unit"))
                                    Spacer(Modifier.width(4.dp))
                                    Icon(Icons.Filled.ArrowDropDown, contentDescription = null)
                                }
                                DropdownMenu(expanded = unitMenu, onDismissRequest = { unitMenu = false }) {
                                    UNITS.forEach { option ->
                                        DropdownMenuItem(
                                            text = { Text(tr("rules.unit.$option")) },
                                            onClick = {
                                                unit = option
                                                unitMenu = false
                                            },
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
            }

            SettingsSectionLabel(tr("rules.then"))
            Surface(
                shape = connectedShape(ConnectedPosition.First),
                color = MaterialTheme.colorScheme.surface,
                tonalElevation = 1.dp,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Box(Modifier.fillMaxWidth().padding(16.dp)) {
                    OutlinedTextField(
                        value = tags,
                        onValueChange = {
                            tags = it
                            invalid = false
                        },
                        singleLine = true,
                        label = { Text(tr("tags.title")) },
                        placeholder = { Text(tr("rules.tags")) },
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            }
            if (!draft.sent) {
                Spacer(Modifier.height(CONNECTED_GAP))
                Surface(
                    onClick = { change(draft.copy(markRead = !draft.markRead)) },
                    shape = connectedShape(ConnectedPosition.Middle),
                    color = MaterialTheme.colorScheme.surface,
                    tonalElevation = 1.dp,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Row(
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(
                            Icons.Filled.DoneAll,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Spacer(Modifier.width(12.dp))
                        Text(
                            tr("notification.actionMarkRead"),
                            style = MaterialTheme.typography.bodyLarge,
                            modifier = Modifier.weight(1f),
                        )
                        Switch(
                            checked = draft.markRead,
                            onCheckedChange = { change(draft.copy(markRead = it)) },
                            colors = settingsSwitchColors(),
                        )
                    }
                }
            }
            Spacer(Modifier.height(CONNECTED_GAP))
            Surface(
                onClick = { change(draft.copy(star = !draft.star)) },
                shape = connectedShape(ConnectedPosition.Middle),
                color = MaterialTheme.colorScheme.surface,
                tonalElevation = 1.dp,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(
                        Icons.Filled.Star,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(Modifier.width(12.dp))
                    Text(
                        tr("chat.star"),
                        style = MaterialTheme.typography.bodyLarge,
                        modifier = Modifier.weight(1f),
                    )
                    Switch(
                        checked = draft.star,
                        onCheckedChange = { change(draft.copy(star = it)) },
                        colors = settingsSwitchColors(),
                    )
                }
            }
            Spacer(Modifier.height(CONNECTED_GAP))
            Surface(
                shape = connectedShape(if (draft.sent) ConnectedPosition.Last else ConnectedPosition.Middle),
                color = MaterialTheme.colorScheme.surface,
                tonalElevation = 1.dp,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Box(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp)) {
                    RuleTargetRow(
                        title = tr("rules.copyTo"),
                        value =
                            if (draft.copyFolder.isEmpty()) {
                                tr("rules.copyNone")
                            } else {
                                targetLabel(mailAccounts, draft.copyAccount, draft.copyFolder)
                            },
                        accounts = mailAccounts,
                        options = listOf(tr("rules.copyNone") to { change(draft.copy(copyAccount = "", copyFolder = "")) }),
                    ) { accountId, folder -> change(draft.copy(copyAccount = accountId, copyFolder = folder)) }
                }
            }
            if (!draft.sent) {
                Spacer(Modifier.height(CONNECTED_GAP))
                Surface(
                    shape = connectedShape(ConnectedPosition.Last),
                    color = MaterialTheme.colorScheme.surface,
                    tonalElevation = 1.dp,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Box(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp)) {
                        RuleTargetRow(
                            title = tr("rules.moveTo"),
                            value = moveLabel(mailAccounts, draft),
                            accounts = mailAccounts,
                            options =
                                listOf("", "archive", "trash").map { option ->
                                    moveLabel(mailAccounts, Rule(query = "", moveTo = option)) to {
                                        change(draft.copy(moveTo = option, folder = "", folderAccount = ""))
                                    }
                                },
                        ) { accountId, folder -> change(draft.copy(moveTo = "folder", folder = folder, folderAccount = accountId)) }
                    }
                }
            }

            if (draft.moveTo == "trash") {
                Text(
                    tr(if (fields == SearchFields()) "rules.trashAllHint" else "rules.trashHint"),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(horizontal = 4.dp),
                )
            }
            if (invalid) {
                Text(
                    tr("rules.invalid"),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(horizontal = 4.dp),
                )
            }
            Spacer(Modifier.height(16.dp))
            Row(
                Modifier.fillMaxWidth().padding(vertical = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                OutlinedButton(
                    onClick = ::reset,
                    modifier = Modifier.weight(1f),
                    shape = RoundedCornerShape(16.dp),
                ) {
                    Text(tr("buttons.cancel"))
                }
                Button(
                    enabled = !saving && rules != null && !(scoped && chosenAccounts.isEmpty()) && delay != null,
                    onClick = {
                        // As it stands now: Cancel stays available during the save.
                        val submitted =
                            draft.copy(
                                trigger = if (timing == "now") "arrival" else timing,
                                delaySeconds = delay ?: return@Button,
                            )
                        val submittedFields = fields
                        val accounts = chosenAccounts
                        val tagList = tags.split(",").map(String::trim).filter(String::isNotEmpty)
                        val current = rules.orEmpty()
                        save({
                            buildSearchQuery(submittedFields)?.let { query ->
                                val rule = submitted.copy(query = query, tags = tagList, accounts = accounts)
                                // Editing doesn't turn a rule on or off.
                                val live = rules.orEmpty().firstOrNull { it.id == rule.id }
                                val edited = rule.copy(disabled = live?.disabled ?: rule.disabled)
                                if (editing) current.map { if (it.id == rule.id) edited else it } else current + rule
                            }
                        }, onSaved = ::reset)
                    },
                    modifier = Modifier.weight(1f),
                    shape = RoundedCornerShape(16.dp),
                ) {
                    Text(if (editing) tr("buttons.save") else tr("rules.add"))
                }
            }
        }

        if (!formOpen && status.log.isNotEmpty()) {
            val logItems = status.log.take(ACTIVITY_SHOWN)
            Column {
                SettingsSectionLabel(tr("rules.activity"))
                logItems.forEachIndexed { index, entry ->
                    if (index > 0) Spacer(Modifier.height(CONNECTED_GAP))
                    val pos =
                        when {
                            logItems.size == 1 -> ConnectedPosition.Single
                            index == 0 -> ConnectedPosition.First
                            index == logItems.lastIndex -> ConnectedPosition.Last
                            else -> ConnectedPosition.Middle
                        }
                    Surface(
                        shape = connectedShape(pos),
                        color = MaterialTheme.colorScheme.surface,
                        tonalElevation = 1.dp,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Row(
                            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp),
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(
                                entry.subject.ifEmpty { tr("rules.activityRun", mapOf("count" to entry.count.toString())) },
                                modifier = Modifier.weight(1f),
                                style = MaterialTheme.typography.bodyMedium,
                                color = if (entry.failed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            Text(
                                formatRelativeTime(entry.at),
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }
            }
        }
    }
}
