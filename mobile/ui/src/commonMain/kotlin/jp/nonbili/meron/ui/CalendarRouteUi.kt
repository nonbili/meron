package jp.nonbili.meron.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.Repeat
import androidx.compose.material.icons.filled.TaskAlt
import androidx.compose.material.icons.filled.Today
import androidx.compose.material3.DrawerState
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.CalendarItem
import kotlinx.coroutines.launch
import kotlin.math.abs

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

        // Days with something on: the kinds, tasks apart from events, so the
        // grid can mark them in their own colors.
        val marks =
            remember(calendarItems) {
                { day: CivilDate -> calendarItems.filter { occursOn(it, day, ::localMidnight) }.map { it.source == "task" } }
            }

        val listState = rememberLazyListState()
        val scrolled by remember { derivedStateOf { listState.canScrollBackward } }

        fun pageMonth(months: Int) {
            selectedDay = null
            scope.launch { shiftCalendarMonth(months) }
        }

        ModalNavigationDrawer(
            drawerState = drawerState,
            drawerContent = { ViewsDrawer(state, drawerState, drawerFolders) },
        ) {
            Scaffold(
                // The canvas shows through between the cards.
                containerColor = MaterialTheme.colorScheme.background,
                // Deleting an event offers an Undo here.
                snackbarHost = { SnackbarHost(snackbarHost) },
                topBar = {
                    TopAppBar(
                        colors = canvasAppBarColors(scrolled),
                        title = {
                            // The size and weight of the Mail and Kanban titles.
                            Text(
                                formatDate(epochSecondsForLocalDate(calendarYear, calendarMonth, 1) * 1000, DateStyle.MonthYear),
                                fontSize = 18.sp,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
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
                            IconButton(onClick = { pageMonth(-1) }) {
                                Icon(Icons.AutoMirrored.Filled.KeyboardArrowLeft, contentDescription = tr("calendar.previousMonth"))
                            }
                            IconButton(onClick = { pageMonth(1) }) {
                                Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = tr("calendar.nextMonth"))
                            }
                        },
                    )
                },
                floatingActionButton = {
                    ExtendedFloatingActionButton(
                        onClick = { editor = CalendarEditorTarget.New(visibleDay) },
                        icon = { Icon(Icons.Filled.Add, contentDescription = null) },
                        text = { Text(tr("calendar.newEvent")) },
                    )
                },
            ) { padding ->
                // The agenda scrolls on under the navigation bar.
                LazyColumn(
                    modifier = Modifier.padding(padding.withoutBottom()),
                    state = listState,
                    contentPadding =
                        PaddingValues(start = ViewCardMargin, end = ViewCardMargin, top = 12.dp, bottom = 88.dp + navigationBarInset()),
                ) {
                    item(key = "grid") {
                        MonthCard(
                            year = calendarYear,
                            month = calendarMonth,
                            weekStart = firstDayOfWeek(),
                            today = today,
                            selected = visibleDay,
                            marks = marks,
                            onSelect = { selectedDay = it },
                            onShiftMonth = ::pageMonth,
                        )
                    }
                    item(key = "day") {
                        Row(
                            modifier = Modifier.padding(start = 8.dp, end = 8.dp, top = 20.dp, bottom = 10.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Text(
                                formatDate(localMidnight(visibleDay) * 1000, DateStyle.WeekdayDate),
                                style = MaterialTheme.typography.titleMedium,
                            )
                            if (visibleDay == today) {
                                Text(
                                    tr("calendar.today"),
                                    style = MaterialTheme.typography.labelMedium,
                                    color = MaterialTheme.colorScheme.onPrimaryContainer,
                                    modifier =
                                        Modifier
                                            .clip(RoundedCornerShape(8.dp))
                                            .background(MaterialTheme.colorScheme.primaryContainer)
                                            .padding(horizontal = 8.dp, vertical = 3.dp),
                                )
                            }
                        }
                    }
                    if (dayItems.isEmpty()) {
                        item(key = "empty") {
                            Surface(
                                color = MaterialTheme.colorScheme.surface,
                                shape = RoundedCornerShape(ViewCardRadius),
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Text(
                                    tr("calendar.nothingScheduled"),
                                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 18.dp),
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    }
                    itemsIndexed(dayItems, key = { _, item -> item.key }) { index, item ->
                        Surface(
                            color = MaterialTheme.colorScheme.surface,
                            shape = groupedRowShape(index, dayItems.size),
                            modifier = Modifier.padding(top = if (index > 0) GroupedRowGap else 0.dp),
                        ) {
                            Column {
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

/**
 * The month as a card. It pages with a sideways swipe as well as the bar's
 * arrows, the incoming month sliding in from the side it lies on.
 */
@Composable
private fun MonthCard(
    year: Int,
    month: Int,
    weekStart: Int,
    today: CivilDate,
    selected: CivilDate,
    marks: (CivilDate) -> List<Boolean>,
    onSelect: (CivilDate) -> Unit,
    onShiftMonth: (Int) -> Unit,
) {
    val shift by rememberUpdatedState(onShiftMonth)
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    Surface(
        color = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(ViewCardRadius),
        modifier =
            Modifier.fillMaxWidth().pointerInput(rtl) {
                val threshold = 56.dp.toPx()
                var dragged = 0f
                detectHorizontalDragGestures(
                    onDragStart = { dragged = 0f },
                    onDragEnd = {
                        // Dragging towards the start brings the next month in.
                        if (abs(dragged) > threshold) shift(if ((dragged < 0) != rtl) 1 else -1)
                    },
                ) { change, amount ->
                    change.consume()
                    dragged += amount
                }
            },
    ) {
        AnimatedContent(
            targetState = year * 12 + (month - 1),
            transitionSpec = {
                val forward = (targetState > initialState) != rtl
                val side = if (forward) 1 else -1
                (slideInHorizontally { it * side } + fadeIn())
                    .togetherWith(slideOutHorizontally { -it * side } + fadeOut())
                    .using(SizeTransform(clip = false))
            },
        ) { index ->
            val shownMonth = index.mod(12) + 1
            MonthGrid(
                days = monthGrid(index.floorDiv(12), shownMonth, weekStart),
                month = shownMonth,
                today = today,
                selected = selected,
                marks = marks,
                onSelect = onSelect,
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
    marks: (CivilDate) -> List<Boolean>,
    onSelect: (CivilDate) -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val muted = colors.onSurfaceVariant
    Column(Modifier.padding(horizontal = 8.dp).padding(top = 8.dp, bottom = 10.dp)) {
        Row {
            days.take(7).forEach { day ->
                Text(
                    formatDate(localMidnight(day) * 1000, DateStyle.Weekday),
                    modifier = Modifier.weight(1f).padding(vertical = 8.dp),
                    textAlign = TextAlign.Center,
                    style = MaterialTheme.typography.labelMedium,
                    color = muted,
                )
            }
        }
        // The grid is six weeks for the data's sake; a week wholly in the next
        // month is not worth a row.
        days.chunked(7).filter { week -> week.any { it.month == month } }.forEach { week ->
            Row {
                week.forEach { day ->
                    val isSelected = day == selected
                    val isToday = day == today
                    val outside = day.month != month
                    Column(
                        modifier =
                            Modifier
                                .weight(1f)
                                .height(50.dp)
                                .clip(RoundedCornerShape(12.dp))
                                .clickable { onSelect(day) },
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.Center,
                    ) {
                        Box(
                            modifier =
                                Modifier
                                    .size(34.dp)
                                    .clip(CircleShape)
                                    .then(
                                        when {
                                            isSelected -> Modifier.background(colors.primary)
                                            isToday -> Modifier.background(colors.primaryContainer)
                                            else -> Modifier
                                        },
                                    ),
                            contentAlignment = Alignment.Center,
                        ) {
                            Text(
                                day.day.toString(),
                                style = MaterialTheme.typography.bodyMedium,
                                fontWeight = if (isToday || isSelected) FontWeight.SemiBold else FontWeight.Normal,
                                color =
                                    when {
                                        isSelected -> colors.onPrimary
                                        isToday -> colors.onPrimaryContainer
                                        outside -> muted.copy(alpha = 0.45f)
                                        else -> colors.onSurface
                                    },
                            )
                        }
                        // Up to three marks, events before tasks; the row keeps
                        // its height on an empty day so the numbers line up.
                        Row(
                            modifier = Modifier.padding(top = 3.dp).height(4.dp),
                            horizontalArrangement = Arrangement.spacedBy(3.dp),
                        ) {
                            marks(day).sorted().take(3).forEach { task ->
                                Box(
                                    Modifier
                                        .size(4.dp)
                                        .clip(CircleShape)
                                        .background((if (task) colors.tertiary else colors.primary).copy(alpha = if (outside) 0.4f else 1f)),
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

/**
 * One item of the chosen day: when, a bar in its kind's color, then what and
 * where. A task due carries a tick mark in place of an end time.
 */
@Composable
private fun CalendarItemRow(
    item: CalendarItem,
    onClick: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val muted = colors.onSurfaceVariant
    val task = item.source == "task"
    val tint = if (task) colors.tertiary else colors.primary.copy(alpha = if (item.response == "tentative") 0.4f else 1f)
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .clickable(onClick = onClick)
                .heightIn(min = 64.dp)
                .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Column(Modifier.width(64.dp)) {
            if (item.allDay) {
                Text(tr("invite.allDay"), style = MaterialTheme.typography.labelLarge, color = muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
            } else {
                Text(formatDate(item.start * 1000, DateStyle.Time), style = MaterialTheme.typography.labelLarge, maxLines = 1)
                // An end on another day would read as before the start.
                if (!task && item.end > item.start && isSameLocalDate(item.start * 1000, item.end * 1000)) {
                    Text(formatDate(item.end * 1000, DateStyle.Time), style = MaterialTheme.typography.labelMedium, color = muted, maxLines = 1)
                }
            }
        }
        Box(Modifier.size(width = 4.dp, height = 36.dp).clip(RoundedCornerShape(2.dp)).background(tint))
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(
                    item.title.ifBlank { tr("invite.untitled") },
                    modifier = Modifier.weight(1f, fill = false),
                    style = MaterialTheme.typography.bodyLarge,
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
                Text(detail, style = MaterialTheme.typography.bodyMedium, color = muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        if (task) Icon(Icons.Filled.TaskAlt, contentDescription = tr("tasks.title"), modifier = Modifier.size(20.dp), tint = tint)
    }
}
