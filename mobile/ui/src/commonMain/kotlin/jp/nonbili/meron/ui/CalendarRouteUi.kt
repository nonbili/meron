package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.Repeat
import androidx.compose.material.icons.filled.Today
import androidx.compose.material3.DrawerState
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.CalendarItem
import kotlinx.coroutines.launch

/**
 * The Calendar route: a month grid with the chosen day's events below it —
 * events made here, accepted invitations and (with Tasks on) the open tasks
 * due. Mirrors desktop's CalendarView; the month grid is too small on a phone
 * to list titles, so it marks days and the list does the telling.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun CalendarRouteContent(
    state: MeronMobileState,
    drawerState: DrawerState,
    drawerFolders: List<jp.nonbili.meron.shared.FolderSummary>,
) {
    with(state) {
        val today = remember { localDateParts(currentTimeMillis()).let { CivilDate(it.year, it.month, it.day) } }
        // Null until a day is tapped, and again after the month changes.
        var selectedDay by remember { mutableStateOf<CivilDate?>(today) }
        var editor by remember { mutableStateOf<CalendarEditorTarget?>(null) }

        // A save in flight: a second tap must not create the event twice.
        var saving by remember { mutableStateOf(false) }
        val days = calendarGrid()
        // A tapped day stays chosen while the grid shows it, the neighbouring
        // months' days included; otherwise today in its own month, or the 1st.
        val visibleDay =
            selectedDay?.takeIf { it in days }
                ?: today.takeIf { it.year == calendarYear && it.month == calendarMonth }
                ?: CivilDate(calendarYear, calendarMonth, 1)
        val dayItems = calendarItems.filter { occursOn(it, visibleDay, ::localMidnight) }

        // Also covers a process restart straight onto the remembered route, and
        // a rebuilt state, which starts with no items.
        LaunchedEffect(state) { loadCalendar() }

        ModalNavigationDrawer(
            drawerState = drawerState,
            drawerContent = { ViewsDrawer(state, drawerState, drawerFolders) },
        ) {
            Scaffold(
                // Deleting an event offers an Undo here.
                snackbarHost = { SnackbarHost(snackbarHost) },
                topBar = {
                    TopAppBar(
                        title = {
                            Text(formatDate(epochSecondsForLocalDate(calendarYear, calendarMonth, 1) * 1000, DateStyle.MonthYear))
                        },
                        navigationIcon = {
                            IconButton(onClick = { scope.launch { drawerState.open() } }) {
                                Icon(Icons.Filled.Menu, contentDescription = tr("mobile.actions.openNavigation"))
                            }
                        },
                        actions = {
                            IconButton(onClick = {
                                selectedDay = today
                                scope.launch { calendarToToday() }
                            }) {
                                Icon(Icons.Filled.Today, contentDescription = tr("calendar.today"))
                            }
                            IconButton(onClick = {
                                selectedDay = null
                                scope.launch { shiftCalendarMonth(-1) }
                            }) {
                                Icon(Icons.AutoMirrored.Filled.KeyboardArrowLeft, contentDescription = tr("calendar.previousMonth"))
                            }
                            IconButton(onClick = {
                                selectedDay = null
                                scope.launch { shiftCalendarMonth(1) }
                            }) {
                                Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = tr("calendar.nextMonth"))
                            }
                        },
                    )
                },
                floatingActionButton = {
                    FloatingActionButton(onClick = { editor = CalendarEditorTarget.New(visibleDay) }) {
                        Icon(Icons.Filled.Add, contentDescription = tr("calendar.newEvent"))
                    }
                },
            ) { padding ->
                LazyColumn(
                    modifier = Modifier.padding(padding),
                    contentPadding = PaddingValues(bottom = 88.dp),
                ) {
                    item(key = "grid") {
                        MonthGrid(
                            days = days,
                            month = calendarMonth,
                            today = today,
                            selected = visibleDay,
                            hasItems = { day -> calendarItems.any { occursOn(it, day, ::localMidnight) } },
                            onSelect = { selectedDay = it },
                        )
                    }
                    item(key = "day") {
                        Text(
                            formatDate(localMidnight(visibleDay) * 1000, DateStyle.WeekdayDate),
                            modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 4.dp),
                            style = MaterialTheme.typography.titleSmall,
                        )
                    }
                    if (calendarItems.isEmpty()) {
                        item(key = "empty") {
                            Text(
                                tr("calendar.noEvents"),
                                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                    items(dayItems, key = { it.key }) { item ->
                        CalendarItemRow(
                            item = item,
                            onClick = {
                                if (item.source == "task") {
                                    scope.launch { openCalendarTask(item) }
                                } else {
                                    editor = CalendarEditorTarget.Edit(item)
                                }
                            },
                        )
                    }
                }
            }
        }

        editor?.let { target ->
            CalendarEventEditor(
                target = target,
                defaultAlertMinutes = reminderLeadMinutes,
                defaultAllDayAlertDays = reminderAllDayDays,
                saving = saving,
                onDismiss = { if (!saving) editor = null },
                onSave = { params ->
                    if (!saving) {
                        saving = true
                        scope.launch {
                            try {
                                if (saveCalendarEvent(params)) editor = null
                            } finally {
                                saving = false
                            }
                        }
                    }
                },
                onDelete = { eventId ->
                    editor = null
                    scope.launch { deleteCalendarEvent(eventId) }
                },
                onOpenMail = { item ->
                    editor = null
                    openLinkedThread(item.threadId, item.account)
                },
            )
        }
    }
}

@Composable
private fun MonthGrid(
    days: List<CivilDate>,
    month: Int,
    today: CivilDate,
    selected: CivilDate,
    hasItems: (CivilDate) -> Boolean,
    onSelect: (CivilDate) -> Unit,
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column(Modifier.padding(horizontal = 8.dp)) {
        Row {
            days.take(7).forEach { day ->
                Text(
                    formatDate(localMidnight(day) * 1000, DateStyle.Weekday),
                    modifier = Modifier.weight(1f).padding(vertical = 6.dp),
                    textAlign = TextAlign.Center,
                    fontSize = 12.sp,
                    color = muted,
                )
            }
        }
        days.chunked(7).forEach { week ->
            Row {
                week.forEach { day ->
                    val isSelected = day == selected
                    val isToday = day == today
                    Column(
                        modifier =
                            Modifier
                                .weight(1f)
                                .height(48.dp)
                                .clip(RoundedCornerShape(10.dp))
                                .clickable { onSelect(day) },
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.Center,
                    ) {
                        Box(
                            modifier =
                                Modifier
                                    .size(32.dp)
                                    .clip(CircleShape)
                                    .then(
                                        when {
                                            isSelected -> Modifier.background(MaterialTheme.colorScheme.primary)
                                            isToday -> Modifier.border(1.5.dp, MaterialTheme.colorScheme.primary, CircleShape)
                                            else -> Modifier
                                        },
                                    ),
                            contentAlignment = Alignment.Center,
                        ) {
                            Text(
                                day.day.toString(),
                                fontSize = 14.sp,
                                fontWeight = if (isToday || isSelected) FontWeight.SemiBold else FontWeight.Normal,
                                color =
                                    when {
                                        isSelected -> MaterialTheme.colorScheme.onPrimary
                                        day.month != month -> muted.copy(alpha = 0.5f)
                                        else -> MaterialTheme.colorScheme.onSurface
                                    },
                            )
                        }
                        Box(
                            Modifier
                                .padding(top = 2.dp)
                                .size(4.dp)
                                .clip(CircleShape)
                                .then(if (hasItems(day)) Modifier.background(MaterialTheme.colorScheme.primary) else Modifier),
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun CalendarItemRow(
    item: CalendarItem,
    onClick: () -> Unit,
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val task = item.source == "task"
    Row(
        modifier = Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            if (item.allDay) tr("invite.allDay") else formatDate(item.start * 1000, DateStyle.Time),
            modifier = Modifier.width(64.dp),
            fontSize = 13.sp,
            color = muted,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Box(
            Modifier
                .size(width = 3.dp, height = 32.dp)
                .clip(RoundedCornerShape(2.dp))
                .background(
                    if (task) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.primary.copy(alpha = if (item.response == "tentative") 0.4f else 1f),
                ),
        )
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                if (task) Icon(Icons.Filled.Check, contentDescription = null, modifier = Modifier.size(14.dp), tint = MaterialTheme.colorScheme.tertiary)
                Text(
                    item.title.ifBlank { tr("invite.untitled") },
                    modifier = Modifier.weight(1f, fill = false),
                    fontSize = 15.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (item.recurring) Icon(Icons.Filled.Repeat, contentDescription = tr("invite.recurring"), modifier = Modifier.size(14.dp), tint = muted)
            }
            val detail =
                listOfNotNull(
                    item.location.takeIf { it.isNotBlank() },
                    tr("invite.youTentative").takeIf { item.response == "tentative" },
                ).joinToString(" · ")
            if (detail.isNotEmpty()) {
                Text(detail, fontSize = 13.sp, color = muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}
