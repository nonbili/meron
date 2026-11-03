package jp.nonbili.meron.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Event
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.outlined.Email
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.DrawerState
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TimePicker
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.rememberTimePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.TaskSummary
import kotlinx.coroutines.launch

/**
 * The Tasks route: drawer, top bar with the list picker and list actions, and
 * the task list itself. Mirrors [KanbanRouteContent]'s shape so the two
 * top-level non-mail screens behave the same way.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun TasksRouteContent(
    state: MeronMobileState,
    drawerState: DrawerState,
    drawerFolders: List<jp.nonbili.meron.shared.FolderSummary>,
) {
    with(state) {
        var listMenuOpen by remember { mutableStateOf(false) }
        var listSwitcherOpen by remember { mutableStateOf(false) }
        var renamingList by remember { mutableStateOf(false) }
        var listNameDraft by remember { mutableStateOf("") }
        var deletingList by remember { mutableStateOf(false) }
        var editing by remember { mutableStateOf<TaskSummary?>(null) }

        // The screen can be reached with no lists loaded — a process restart
        // straight onto the remembered route — so make sure there is one.
        LaunchedEffect(Unit) { if (taskLists.isEmpty()) openTasks(activeTaskListId) }

        val activeList = taskLists.firstOrNull { it.id == activeTaskListId }

        ModalNavigationDrawer(
            drawerState = drawerState,
            drawerContent = {
                ViewsDrawer(state, drawerState, drawerFolders)
            },
        ) {
            Scaffold(
                // Deleting a task or a list offers an Undo here, so the host has
                // to be mounted on this screen too.
                snackbarHost = { SnackbarHost(snackbarHost) },
                topBar = {
                    TopAppBar(
                        title = {
                            // Tapping the title switches lists; the overflow
                            // menu keeps only the actions on the active list.
                            Box {
                                Row(
                                    modifier = Modifier.clickable(enabled = taskLists.size > 1) { listSwitcherOpen = true },
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    Text(activeList?.title ?: tr("tasks.title"))
                                    if (taskLists.size > 1) {
                                        Icon(Icons.Filled.ArrowDropDown, contentDescription = null)
                                    }
                                }
                                DropdownMenu(
                                    expanded = listSwitcherOpen,
                                    onDismissRequest = { listSwitcherOpen = false },
                                ) {
                                    taskLists.forEach { list ->
                                        DropdownMenuItem(
                                            text = { Text(list.title) },
                                            trailingIcon = {
                                                if (list.id == activeTaskListId) Icon(Icons.Filled.Check, contentDescription = null)
                                            },
                                            onClick = {
                                                listSwitcherOpen = false
                                                scope.launch { selectTaskList(list.id) }
                                            },
                                        )
                                    }
                                }
                            }
                        },
                        navigationIcon = {
                            IconButton(onClick = { scope.launch { drawerState.open() } }) {
                                Icon(Icons.Filled.Menu, contentDescription = tr("mobile.actions.openNavigation"))
                            }
                        },
                        actions = {
                            Box {
                                IconButton(onClick = { listMenuOpen = true }) {
                                    Icon(Icons.Filled.MoreVert, contentDescription = tr("chat.moreActions"))
                                }
                                DropdownMenu(
                                    expanded = listMenuOpen,
                                    onDismissRequest = { listMenuOpen = false },
                                ) {
                                    DropdownMenuItem(
                                        text = { Text(tr("tasks.newList")) },
                                        leadingIcon = { Icon(Icons.Filled.Add, contentDescription = null) },
                                        onClick = {
                                            listMenuOpen = false
                                            listNameDraft = ""
                                            renamingList = false
                                            scope.launch { createTaskList(trs("tasks.newList")) }
                                        },
                                    )
                                    DropdownMenuItem(
                                        text = { Text(tr("tasks.renameList")) },
                                        leadingIcon = { Icon(Icons.Filled.Edit, contentDescription = null) },
                                        onClick = {
                                            listMenuOpen = false
                                            listNameDraft = activeList?.title.orEmpty()
                                            renamingList = true
                                        },
                                    )
                                    if (tasks.any { it.done }) {
                                        DropdownMenuItem(
                                            text = { Text(tr("tasks.clearCompleted")) },
                                            onClick = {
                                                listMenuOpen = false
                                                scope.launch { clearCompletedTasks() }
                                            },
                                        )
                                    }
                                    DropdownMenuItem(
                                        text = { Text(tr("tasks.deleteList")) },
                                        leadingIcon = { Icon(Icons.Filled.Delete, contentDescription = null) },
                                        onClick = {
                                            listMenuOpen = false
                                            deletingList = true
                                        },
                                    )
                                }
                            }
                        },
                    )
                },
            ) { padding ->
                TasksScreen(
                    tasks = tasks,
                    loading = tasksLoading,
                    onAddTask = { title -> scope.launch { addTask(title) } },
                    onToggleDone = { task, done -> scope.launch { setTaskDone(task.id, done) } },
                    onEditTask = { editing = it },
                    onDeleteTask = { task -> scope.launch { deleteTask(task.id) } },
                    onReorderTasks = { ids -> scope.launch { reorderTasks(ids) } },
                    onOpenMessage = ::openTaskThread,
                    modifier = Modifier.fillMaxSize().padding(padding),
                )
            }
        }

        if (renamingList) {
            AlertDialog(
                onDismissRequest = { renamingList = false },
                title = { Text(tr("tasks.renameList")) },
                text = {
                    OutlinedTextField(
                        value = listNameDraft,
                        onValueChange = { listNameDraft = it },
                        placeholder = { Text(tr("tasks.listNamePlaceholder")) },
                        singleLine = true,
                    )
                },
                confirmButton = {
                    TextButton(onClick = {
                        val name = listNameDraft
                        renamingList = false
                        scope.launch { renameTaskList(activeTaskListId, name) }
                    }) { Text(tr("buttons.save")) }
                },
                dismissButton = {
                    TextButton(onClick = { renamingList = false }) { Text(tr("buttons.cancel")) }
                },
            )
        }

        if (deletingList) {
            AlertDialog(
                onDismissRequest = { deletingList = false },
                title = { Text(tr("tasks.deleteList")) },
                text = { Text(tr("tasks.deleteListConfirm", mapOf("name" to activeList?.title.orEmpty()))) },
                confirmButton = {
                    TextButton(onClick = {
                        val listId = activeTaskListId
                        deletingList = false
                        scope.launch { deleteTaskList(listId) }
                    }) { Text(tr("tasks.deleteList")) }
                },
                dismissButton = {
                    TextButton(onClick = { deletingList = false }) { Text(tr("buttons.cancel")) }
                },
            )
        }

        editing?.let { task ->
            TaskEditDialog(
                task = task,
                onDismiss = { editing = null },
                onSave = { title, notes, dueAt, dueHasTime ->
                    editing = null
                    scope.launch { updateTask(task.id, title = title, notes = notes, dueAt = dueAt, dueHasTime = dueHasTime) }
                },
                onOpenMessage = {
                    editing = null
                    openTaskThread(task)
                },
            )
        }
    }
}

/**
 * Edit one task's title, notes and due date. The due date opens the platform
 * date picker; a linked message can be opened from here too.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun TaskEditDialog(
    task: TaskSummary,
    onDismiss: () -> Unit,
    onSave: (title: String, notes: String, dueAt: Long, dueHasTime: Boolean) -> Unit,
    onOpenMessage: () -> Unit,
) {
    var title by remember(task.id) { mutableStateOf(task.title) }
    var notes by remember(task.id) { mutableStateOf(task.notes) }
    var dueAt by remember(task.id) { mutableStateOf(task.dueAt) }
    var pickingDate by remember { mutableStateOf(false) }
    var pickingTime by remember { mutableStateOf(false) }
    var dueHasTime by remember(task.id) { mutableStateOf(task.dueHasTime) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("buttons.edit")) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(
                    value = title,
                    onValueChange = { title = it },
                    placeholder = { Text(tr("tasks.titlePlaceholder")) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = notes,
                    onValueChange = { notes = it },
                    placeholder = { Text(tr("tasks.notesPlaceholder")) },
                    minLines = 3,
                    maxLines = 6,
                    modifier = Modifier.fillMaxWidth(),
                )
                Row(verticalAlignment = Alignment.CenterVertically) {
                    AssistChip(
                        onClick = { pickingDate = true },
                        label = {
                            Text(if (dueAt > 0) formatTaskDueDate(dueAt) else tr("tasks.noDueDate"))
                        },
                        leadingIcon = { Icon(Icons.Filled.Event, contentDescription = null, Modifier.size(18.dp)) },
                    )
                    if (dueAt > 0) {
                        IconButton(onClick = {
                            dueAt = 0
                            dueHasTime = false
                        }) {
                            Icon(Icons.Filled.Close, contentDescription = tr("tasks.clearDueDate"), Modifier.size(18.dp))
                        }
                    }
                }
                if (dueAt > 0) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        AssistChip(
                            onClick = { pickingTime = true },
                            label = { Text(if (dueHasTime) formatDate(dueAt * 1000, DateStyle.Time) else tr("tasks.dueTime")) },
                        )
                        if (dueHasTime) {
                            IconButton(onClick = {
                                dueHasTime = false
                                dueAt = parseTaskDueInput(formatTaskDueInput(dueAt))
                            }) { Icon(Icons.Filled.Close, contentDescription = tr("tasks.clearDueTime"), Modifier.size(18.dp)) }
                        }
                    }
                }
                if (task.threadId.isNotBlank()) {
                    TextButton(onClick = onOpenMessage, contentPadding = PaddingValues(horizontal = 4.dp)) {
                        Icon(Icons.Outlined.Email, contentDescription = null, Modifier.size(18.dp))
                        Spacer(Modifier.width(8.dp))
                        Text(tr("tasks.openMessage"))
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { onSave(title, notes, dueAt, dueHasTime) }, enabled = title.isNotBlank()) {
                Text(tr("buttons.save"))
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) } },
    )

    if (pickingTime) {
        val initial = localDateParts(dueAt * 1000)
        val picker = rememberTimePickerState(initialHour = if (dueHasTime) initial.hour else 9, initialMinute = if (dueHasTime) initial.minute else 0)
        AlertDialog(
            onDismissRequest = { pickingTime = false },
            title = { Text(tr("tasks.dueTime")) },
            text = { TimePicker(state = picker) },
            confirmButton = {
                TextButton(onClick = {
                    dueAt = epochSecondsForLocalDateTime(initial.year, initial.month, initial.day, picker.hour, picker.minute)
                    dueHasTime = true
                    pickingTime = false
                }) { Text(tr("buttons.save")) }
            },
            dismissButton = { TextButton(onClick = { pickingTime = false }) { Text(tr("buttons.cancel")) } },
        )
    }
    if (pickingDate) {
        val pickerState = rememberDatePickerState(initialSelectedDateMillis = taskDueToPickerMillis(dueAt))
        DatePickerDialog(
            onDismissRequest = { pickingDate = false },
            confirmButton = {
                TextButton(onClick = {
                    pickerState.selectedDateMillis?.let {
                        val date = localDateParts(pickerMillisToTaskDue(it) * 1000)
                        val time = localDateParts(dueAt * 1000)
                        dueAt = epochSecondsForLocalDateTime(date.year, date.month, date.day, if (dueHasTime) time.hour else 0, if (dueHasTime) time.minute else 0)
                    }
                    pickingDate = false
                }) { Text(tr("buttons.save")) }
            },
            dismissButton = { TextButton(onClick = { pickingDate = false }) { Text(tr("buttons.cancel")) } },
        ) {
            DatePicker(state = pickerState)
        }
    }
}

/**
 * The drawer of the top-level screens that are not mail — Tasks and Calendar —
 * where picking a mailbox or a board leaves for it.
 */
@Composable
internal fun ViewsDrawer(
    state: MeronMobileState,
    drawerState: DrawerState,
    drawerFolders: List<jp.nonbili.meron.shared.FolderSummary>,
) {
    with(state) {
        MailDrawer(
            accounts = coreAccounts.filterNot { it.id in hiddenNavigationAccountIds },
            selectedAccountId = selectedCoreAccountId,
            folders = drawerFolders,
            currentScreen = screen,
            showUnreadBadges = showUnreadBadges,
            showUnifiedInboxNav = showUnifiedInboxNav,
            kanbanBoards = kanbanBoards,
            activeKanbanBoardId = activeKanbanBoardId,
            tasksEnabled = tasksEnabled,
            calendarEnabled = calendarEnabled,
            onSelectUnified = {
                screen = Screen.Mail
                if (selectedCoreAccountId != UNIFIED_ACCOUNT_ID) {
                    selectCoreMailbox(UNIFIED_ACCOUNT_ID)
                    syncCoreThreads(
                        accountOverride = UNIFIED_ACCOUNT_ID,
                        folderOverride = selectedCoreFolder,
                        syncFirst = false,
                    )
                }
                scope.launch { drawerState.close() }
            },
            onSelectAccount = { account ->
                screen = Screen.Mail
                if (selectedCoreAccountId != account.id) {
                    selectCoreMailbox(account.id)
                    syncCoreThreads(
                        accountOverride = account.id,
                        folderOverride = selectedCoreFolder,
                        syncFirst = false,
                    )
                }
                scope.launch { drawerState.close() }
            },
            onSelectKanbanBoard = { board ->
                screen = Screen.Kanban
                if (activeKanbanBoardId != board.id) {
                    activeKanbanBoardId = board.id
                    saveActiveKanbanBoardId(kanbanPrefs, board.id)
                    loadKanbanBoard(refresh = false)
                }
                scope.launch { drawerState.close() }
            },
            onSelectTasks = {
                scope.launch {
                    if (screen != Screen.Tasks) openTasks()
                    drawerState.close()
                }
            },
            onSelectCalendar = {
                scope.launch {
                    if (screen != Screen.Calendar) openCalendar()
                    drawerState.close()
                }
            },
            onAddAccount = {
                resetPasswordAccountForm()
                addSection = 0
                previousTopScreen = screen
                screen = Screen.AddAccount
                scope.launch { drawerState.close() }
            },
            onOpenSettings = {
                previousTopScreen = screen
                screen = Screen.Settings
                scope.launch { drawerState.close() }
            },
            onOpenContacts = {
                previousTopScreen = screen
                screen = Screen.Contacts
                scope.launch { drawerState.close() }
            },
            onShowAbout = {
                showAboutDialog = true
                scope.launch { drawerState.close() }
            },
            googleReauthAccountId = googleReauthAccountId,
            onReconnectGoogle = {
                connectGoogleDeviceAccount()
                scope.launch { drawerState.close() }
            },
        )
    }
}
