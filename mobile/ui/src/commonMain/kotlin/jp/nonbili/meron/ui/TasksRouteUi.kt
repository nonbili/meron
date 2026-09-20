package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.AlertDialog
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
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
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
                    onSelectUnified = {
                        screen = Screen.Mail
                        if (selectedCoreAccountId != UNIFIED_ACCOUNT_ID) {
                            selectCoreMailbox(UNIFIED_ACCOUNT_ID, INBOX_FOLDER)
                            syncCoreThreads(
                                accountOverride = UNIFIED_ACCOUNT_ID,
                                folderOverride = INBOX_FOLDER,
                                syncFirst = false,
                            )
                        }
                        scope.launch { drawerState.close() }
                    },
                    onSelectAccount = { account ->
                        screen = Screen.Mail
                        if (selectedCoreAccountId != account.id) {
                            selectCoreMailbox(account.id, INBOX_FOLDER)
                            syncCoreThreads(
                                accountOverride = account.id,
                                folderOverride = INBOX_FOLDER,
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
                    onSelectTasks = { scope.launch { drawerState.close() } },
                    onAddAccount = {
                        resetPasswordAccountForm()
                        addSection = 0
                        previousTopScreen = Screen.Tasks
                        screen = Screen.AddAccount
                        scope.launch { drawerState.close() }
                    },
                    onOpenSettings = {
                        previousTopScreen = screen
                        screen = Screen.Settings
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
            },
        ) {
            Scaffold(
                // Deleting a task or a list offers an Undo here, so the host has
                // to be mounted on this screen too.
                snackbarHost = { SnackbarHost(snackbarHost) },
                topBar = {
                    TopAppBar(
                        title = { Text(activeList?.title ?: tr("tasks.title")) },
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
                                    // Switching lists lives in the same menu as
                                    // managing them: a picker of its own would
                                    // be a second control for one short list.
                                    taskLists.forEach { list ->
                                        DropdownMenuItem(
                                            text = { Text(list.title) },
                                            onClick = {
                                                listMenuOpen = false
                                                scope.launch { selectTaskList(list.id) }
                                            },
                                        )
                                    }
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
                                    DropdownMenuItem(
                                        text = {
                                            Text(
                                                if (showCompletedTasks) {
                                                    tr("tasks.hideCompleted")
                                                } else {
                                                    tr("tasks.showCompleted")
                                                },
                                            )
                                        },
                                        onClick = {
                                            listMenuOpen = false
                                            scope.launch { setShowCompletedTasks(!showCompletedTasks) }
                                        },
                                    )
                                    if (showCompletedTasks && tasks.any { it.done }) {
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
                    showCompleted = showCompletedTasks,
                    onAddTask = { title -> scope.launch { addTask(title) } },
                    onToggleDone = { task, done -> scope.launch { setTaskDone(task.id, done) } },
                    onEditTask = { editing = it },
                    onDeleteTask = { task -> scope.launch { deleteTask(task.id) } },
                    onMoveTask = { task, delta -> scope.launch { moveTask(task.id, delta) } },
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
                onSave = { title, notes, dueAt ->
                    editing = null
                    scope.launch { updateTask(task.id, title = title, notes = notes, dueAt = dueAt) }
                },
            )
        }
    }
}

/**
 * Edit one task's title, notes and due date.
 *
 * The due date is typed as `YYYY-MM-DD` rather than picked from a calendar:
 * nothing else in this app shows a date picker, and a wrong-looking one is
 * worse than a field that says exactly what it wants.
 */
@Composable
private fun TaskEditDialog(
    task: TaskSummary,
    onDismiss: () -> Unit,
    onSave: (title: String, notes: String, dueAt: Long) -> Unit,
) {
    var title by remember(task.id) { mutableStateOf(task.title) }
    var notes by remember(task.id) { mutableStateOf(task.notes) }
    var due by remember(task.id) { mutableStateOf(formatTaskDueInput(task.dueAt)) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("buttons.edit")) },
        text = {
            androidx.compose.foundation.layout.Column(
                verticalArrangement =
                    androidx.compose.foundation.layout.Arrangement
                        .spacedBy(8.dp),
            ) {
                OutlinedTextField(
                    value = title,
                    onValueChange = { title = it },
                    placeholder = { Text(tr("tasks.titlePlaceholder")) },
                    singleLine = true,
                )
                OutlinedTextField(
                    value = notes,
                    onValueChange = { notes = it },
                    placeholder = { Text(tr("tasks.notesPlaceholder")) },
                )
                OutlinedTextField(
                    value = due,
                    onValueChange = { due = it },
                    label = { Text(tr("tasks.dueDate")) },
                    placeholder = { Text("YYYY-MM-DD") },
                    singleLine = true,
                )
            }
        },
        confirmButton = {
            TextButton(onClick = { onSave(title, notes, parseTaskDueInput(due)) }) {
                Text(tr("buttons.save"))
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) } },
    )
}
