package jp.nonbili.meron.ui

/*
 * The repeat rules the custom repeat editor can say, as the core keeps them in
 * an event's `repeat` (see meron-core/src/calendar/rrule.rs): a plain choice
 * (`weekly`), or an RRULE in a fixed canonical form, so a rule saved without
 * changes compares equal to the one the editor opened with. A port of
 * desktop's lib/repeatRule.ts; weekdays count 0 Monday … 6 Sunday, as RRULE
 * orders them.
 */

internal enum class RepeatFreq { Daily, Weekly, Monthly, Yearly }

internal sealed interface RepeatEnd {
    data object Never : RepeatEnd

    data class Count(
        val count: Int,
    ) : RepeatEnd

    /** The last day. */
    data class Until(
        val date: CivilDate,
    ) : RepeatEnd
}

/** A monthly rule's day of the month, or its nth weekday (-1 the last). */
internal sealed interface MonthlyOn {
    data class Day(
        val day: Int,
    ) : MonthlyOn

    data class Weekday(
        val nth: Int,
        val weekday: Int,
    ) : MonthlyOn
}

internal data class RepeatRule(
    val freq: RepeatFreq,
    val interval: Int = 1,
    /** A weekly rule's days, sorted. */
    val weekdays: List<Int> = emptyList(),
    val monthly: MonthlyOn? = null,
    /** The day weeks start on, which picks the weeks an every-other-week rule
     *  on several days skips; Monday, RFC 5545's default, otherwise. */
    val weekStart: Int? = null,
    val end: RepeatEnd = RepeatEnd.Never,
)

internal const val MAX_REPEAT_INTERVAL = 999
internal const val MAX_REPEAT_COUNT = 999
private val CODES = listOf("MO", "TU", "WE", "TH", "FR", "SA", "SU")
private val PLAIN = mapOf("daily" to RepeatFreq.Daily, "weekly" to RepeatFreq.Weekly, "monthly" to RepeatFreq.Monthly, "yearly" to RepeatFreq.Yearly)

internal fun isPlainRepeat(value: String): Boolean = value in PLAIN

/** A stored repeat (plain or RRULE) as a rule, or null for none or for one
 *  outside what the editor says. */
internal fun parseRepeat(value: String): RepeatRule? {
    PLAIN[value]?.let { return RepeatRule(it) }
    if ('=' !in value) return null
    val parts = mutableMapOf<String, String>()
    for (part in value.split(';')) {
        if (part.isEmpty()) continue
        val pieces = part.split('=')
        if (pieces.size != 2 || pieces[0].isEmpty()) return null
        parts[pieces[0].uppercase()] = pieces[1].uppercase()
    }
    val freq = PLAIN[parts["FREQ"]?.lowercase()] ?: return null
    val interval = (parts["INTERVAL"] ?: "1").toIntOrNull()?.takeIf { it >= 1 } ?: return null
    var rule = RepeatRule(freq, interval)
    val byDay = parts["BYDAY"]
    val monthDay = parts["BYMONTHDAY"]
    when {
        byDay != null && freq == RepeatFreq.Weekly -> {
            val days = byDay.split(',').map { CODES.indexOf(it) }
            if (days.any { it < 0 }) return null
            val weekStart = parts["WKST"]?.let { code -> CODES.indexOf(code).takeIf { it >= 0 } ?: return null }
            rule = rule.copy(weekdays = days.distinct().sorted(), weekStart = weekStart)
        }

        byDay != null && freq == RepeatFreq.Monthly -> {
            val match = Regex("^([+-]?\\d)(MO|TU|WE|TH|FR|SA|SU)$").find(byDay) ?: return null
            rule = rule.copy(monthly = MonthlyOn.Weekday(match.groupValues[1].toInt(), CODES.indexOf(match.groupValues[2])))
        }

        monthDay != null && freq == RepeatFreq.Monthly -> {
            rule = rule.copy(monthly = MonthlyOn.Day(monthDay.toIntOrNull() ?: return null))
        }

        byDay != null || monthDay != null -> {
            return null
        }
    }
    val count = parts["COUNT"]
    val until = parts["UNTIL"]
    rule =
        when {
            count != null -> {
                rule.copy(end = RepeatEnd.Count(count.toIntOrNull() ?: return null))
            }

            until != null && Regex("^\\d{8}$").matches(until) -> {
                rule.copy(end = RepeatEnd.Until(CivilDate(until.substring(0, 4).toInt(), until.substring(4, 6).toInt(), until.substring(6, 8).toInt())))
            }

            until != null -> {
                return null
            }

            else -> {
                rule
            }
        }
    return rule
}

private fun pad(
    value: Int,
    width: Int,
) = value.toString().padStart(width, '0')

/** `YYYY-MM-DD`, as the core names a series' first day. */
internal fun CivilDate.isoString(): String = "${pad(year, 4)}-${pad(month, 2)}-${pad(day, 2)}"

internal fun parseIsoDate(value: String): CivilDate? {
    val parts = value.split('-')
    if (parts.size != 3) return null
    val year = parts[0].toIntOrNull() ?: return null
    val month = parts[1].toIntOrNull()?.takeIf { it in 1..12 } ?: return null
    val day = parts[2].toIntOrNull()?.takeIf { it in 1..31 } ?: return null
    return CivilDate(year, month, day)
}

/** The rule as the core keeps it: a plain choice when it says no more. */
internal fun formatRepeat(rule: RepeatRule): String {
    val weekdays = if (rule.freq == RepeatFreq.Weekly) rule.weekdays else emptyList()
    val monthly = if (rule.freq == RepeatFreq.Monthly) rule.monthly else null
    val freq = rule.freq.name.lowercase()
    if (rule.interval == 1 && weekdays.isEmpty() && monthly == null && rule.end == RepeatEnd.Never) return freq
    val parts = mutableListOf("FREQ=${freq.uppercase()}")
    if (rule.interval != 1) parts += "INTERVAL=${rule.interval}"
    if (weekdays.isNotEmpty()) parts += "BYDAY=" + weekdays.sorted().joinToString(",") { CODES[it] }
    when (monthly) {
        is MonthlyOn.Weekday -> {
            parts += "BYDAY=${monthly.nth}${CODES[monthly.weekday]}"
        }

        is MonthlyOn.Day -> {
            parts += "BYMONTHDAY=${monthly.day}"
        }

        null -> {}
    }
    // Written only where it picks the weeks, as the core keeps it.
    val weekStart = rule.weekStart
    if (weekStart != null && weekStart != 0 && rule.interval > 1 && weekdays.size > 1) parts += "WKST=${CODES[weekStart]}"
    when (val end = rule.end) {
        is RepeatEnd.Count -> {
            parts += "COUNT=${end.count}"
        }

        is RepeatEnd.Until -> {
            parts += "UNTIL=${end.date.isoString().replace("-", "")}"
        }

        RepeatEnd.Never -> {}
    }
    return parts.joinToString(";")
}

/** A date's weekday, Monday 0. */
internal fun ruleWeekday(date: CivilDate): Int = (date.weekday + 6) % 7

private fun daysInMonth(
    year: Int,
    month: Int,
): Int {
    val next = if (month == 12) CivilDate(year + 1, 1, 1) else CivilDate(year, month + 1, 1)
    return (next.epochDay - CivilDate(year, month, 1).epochDay).toInt()
}

/** Which of its weekday in the month a date is (1–5), and whether it is the
 *  last of them. */
internal fun weekdayPlace(date: CivilDate): Pair<Int, Boolean> = ((date.day + 6) / 7) to (date.day + 7 > daysInMonth(date.year, date.month))

/** A stored repeat moved with its series, from starting on [from] to [to]:
 *  weekly days move by as many days, a monthly day or weekday becomes the new
 *  start's, as the core moves an existing series'. */
internal fun alignRepeat(
    value: String,
    from: CivilDate,
    to: CivilDate,
): String {
    var rule = (if (isPlainRepeat(value)) null else parseRepeat(value)) ?: return value
    val weekday = ruleWeekday(to)
    if (rule.freq == RepeatFreq.Weekly && rule.weekdays.isNotEmpty()) {
        val shift = (to.epochDay - from.epochDay).mod(7L).toInt()
        val days = rule.weekdays.map { (it + shift) % 7 }.toMutableSet()
        // The weeks move with the days, so an every-other-week rule keeps the
        // same weeks.
        if (rule.interval > 1 && rule.weekdays.size > 1) rule = rule.copy(weekStart = ((rule.weekStart ?: 0) + shift) % 7)
        days += weekday
        rule = rule.copy(weekdays = days.sorted())
    }
    val monthly = rule.monthly
    if (rule.freq == RepeatFreq.Monthly && monthly is MonthlyOn.Day) rule = rule.copy(monthly = MonthlyOn.Day(to.day))
    if (rule.freq == RepeatFreq.Monthly && monthly is MonthlyOn.Weekday) {
        val (nth, last) = weekdayPlace(to)
        rule = rule.copy(monthly = MonthlyOn.Weekday(if (monthly.nth == -1 && last) -1 else nth, weekday))
    }
    return formatRepeat(rule)
}

/** A rule to start the custom editor from: the one given, or one repeating
 *  as the plain choice does, from the event's first day. */
internal fun initialRule(
    value: String,
    first: CivilDate,
): RepeatRule {
    var rule = parseRepeat(value) ?: RepeatRule(RepeatFreq.Weekly)
    if (rule.freq == RepeatFreq.Weekly && rule.weekdays.isEmpty()) rule = rule.copy(weekdays = listOf(ruleWeekday(first)))
    if (rule.freq == RepeatFreq.Monthly && rule.monthly == null) rule = rule.copy(monthly = MonthlyOn.Day(first.day))
    return rule
}

/** Weekday names, Monday first: 5 January 1970 was a Monday. */
internal fun weekdayNames(style: DateStyle): List<String> = (0 until 7).map { formatDate(localMidnight(CivilDate(1970, 1, 5 + it)) * 1000, style) }

/** The word for which of its weekday in the month: "third", "last". */
internal fun nthKey(nth: Int): String = if (nth in 1..5) "calendar.repeat.nth$nth" else "calendar.repeat.nthLast"

private fun everyKey(freq: RepeatFreq): String =
    when (freq) {
        RepeatFreq.Daily -> "calendar.repeat.everyDays"
        RepeatFreq.Weekly -> "calendar.repeat.everyWeeks"
        RepeatFreq.Monthly -> "calendar.repeat.everyMonths"
        RepeatFreq.Yearly -> "calendar.repeat.everyYears"
    }

/** The rule in words: "Every 2 weeks on Mon, Wed, until Jun 30, 2027". */
internal fun describeRepeat(
    rule: RepeatRule,
    t: (String, Map<String, Any?>) -> String,
): String {
    var text = t(everyKey(rule.freq), mapOf("count" to rule.interval))
    val monthly = rule.monthly
    if (rule.freq == RepeatFreq.Weekly && rule.weekdays.isNotEmpty()) {
        val names = weekdayNames(DateStyle.Weekday)
        text = t("calendar.repeat.summaryOnDays", mapOf("rule" to text, "days" to rule.weekdays.sorted().joinToString(", ") { names[it] }))
    }
    if (rule.freq == RepeatFreq.Monthly && monthly is MonthlyOn.Day) {
        text = t("calendar.repeat.summaryOnMonthDay", mapOf("rule" to text, "day" to monthly.day))
    }
    if (rule.freq == RepeatFreq.Monthly && monthly is MonthlyOn.Weekday) {
        text =
            t(
                "calendar.repeat.summaryOnWeekday",
                mapOf("rule" to text, "nth" to t(nthKey(monthly.nth), emptyMap()), "weekday" to weekdayNames(DateStyle.WeekdayLong)[monthly.weekday]),
            )
    }
    when (val end = rule.end) {
        is RepeatEnd.Count -> {
            text = t("calendar.repeat.summaryCount", mapOf("rule" to text, "count" to end.count))
        }

        is RepeatEnd.Until -> {
            text = t("calendar.repeat.summaryUntil", mapOf("rule" to text, "date" to formatDate(localMidnight(end.date) * 1000, DateStyle.MonthDayYear)))
        }

        RepeatEnd.Never -> {}
    }
    return text
}
