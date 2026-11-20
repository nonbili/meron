//! The repeat rules the editor can say, between how `events.rrule` keeps
//! them and an iCalendar RRULE (RFC 5545 §3.3.10).
//!
//! `events.rrule` holds one of the plain choices (`daily`, `weekly`,
//! `monthly`, `yearly`), or an RRULE the custom repeat editor can show and
//! change: how often (every N days, weeks, months or years), on which
//! weekdays for a weekly one, on which day or nth weekday for a monthly one,
//! and when it ends (never, after N times, or on a date). Anything else a
//! server says is kept only by the server's own object, with `rrule` empty.
//!
//! The stored form is canonical, so two rules that repeat alike compare
//! equal: parts in a fixed order, a one interval left out, and an end date
//! as the event's own last day (`UNTIL=YYYYMMDD`) rather than the instant
//! RFC 5545 asks a timed series for, which is written only into iCalendar.

use anyhow::{Result, anyhow, bail};
use calcard::common::timezone::Tz;
use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};

/// The plain choices, as `events.rrule` stores them and the RRULE FREQ each is.
const PLAIN: [(&str, Freq); 4] = [
    ("daily", Freq::Daily),
    ("weekly", Freq::Weekly),
    ("monthly", Freq::Monthly),
    ("yearly", Freq::Yearly),
];

const WEEKDAYS: [&str; 7] = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];

/// Bounds on what the editor accepts, well past any real use.
const MAX_INTERVAL: u32 = 999;
const MAX_COUNT: u32 = 999;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

impl Freq {
    fn name(self) -> &'static str {
        match self {
            Freq::Daily => "DAILY",
            Freq::Weekly => "WEEKLY",
            Freq::Monthly => "MONTHLY",
            Freq::Yearly => "YEARLY",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum End {
    Never,
    Count(u32),
    /// The last date an occurrence may start on, in the series' own zone.
    Until(NaiveDate),
    /// As a server wrote it, before it is read as a date.
    UntilRaw(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Rule {
    freq: Freq,
    interval: u32,
    /// A weekly rule's days, Monday first (indices into [`WEEKDAYS`]).
    weekdays: Vec<usize>,
    /// A monthly rule's day of the month.
    month_day: Option<u32>,
    /// A monthly rule's nth weekday: 1–5, or -1 for the last.
    nth_weekday: Option<(i32, usize)>,
    /// The day its weeks start on (an index into [`WEEKDAYS`]), which picks
    /// the weeks an every-other-week rule on several days skips. Kept only
    /// for such a rule, so it is Monday, RFC 5545's default, for any other.
    week_start: usize,
    end: End,
}

impl Rule {
    fn plain(&self) -> Option<&'static str> {
        let only_freq = self.interval == 1
            && self.weekdays.is_empty()
            && self.month_day.is_none()
            && self.nth_weekday.is_none()
            && self.end == End::Never;
        only_freq.then(|| PLAIN.iter().find(|(_, f)| *f == self.freq).map(|(n, _)| *n))?
    }

    /// The RRULE value, with `until` saying the end date.
    fn write(&self, until: impl Fn(NaiveDate) -> String) -> String {
        let mut parts = vec![format!("FREQ={}", self.freq.name())];
        if self.interval != 1 {
            parts.push(format!("INTERVAL={}", self.interval));
        }
        if !self.weekdays.is_empty() {
            let days: Vec<_> = self.weekdays.iter().map(|&d| WEEKDAYS[d]).collect();
            parts.push(format!("BYDAY={}", days.join(",")));
        }
        if let Some((nth, day)) = self.nth_weekday {
            parts.push(format!("BYDAY={nth}{}", WEEKDAYS[day]));
        }
        if let Some(day) = self.month_day {
            parts.push(format!("BYMONTHDAY={day}"));
        }
        if self.week_start != 0 {
            parts.push(format!("WKST={}", WEEKDAYS[self.week_start]));
        }
        match &self.end {
            End::Never => {}
            End::Count(count) => parts.push(format!("COUNT={count}")),
            End::Until(date) => parts.push(format!("UNTIL={}", until(*date))),
            End::UntilRaw(raw) => parts.push(format!("UNTIL={raw}")),
        }
        parts.join(";")
    }

    /// How `events.rrule` keeps it.
    fn stored(&self) -> String {
        match self.plain() {
            Some(plain) => plain.to_string(),
            None => self.write(|date| date.format("%Y%m%d").to_string()),
        }
    }
}

/// Read an RRULE value, or `None` when it says more than the editor can.
fn parse(value: &str) -> Option<Rule> {
    let mut freq = None;
    let mut interval = 1;
    let mut by_day: Option<&str> = None;
    let mut month_day = None;
    let mut count = None;
    let mut until = None;
    let mut week_start = None;
    for part in value.trim().split(';').filter(|p| !p.trim().is_empty()) {
        let (key, value) = part.split_once('=')?;
        let value = value.trim();
        match key.trim().to_ascii_uppercase().as_str() {
            "FREQ" => {
                freq = PLAIN
                    .iter()
                    .map(|(_, f)| *f)
                    .find(|f| f.name().eq_ignore_ascii_case(value));
                freq?;
            }
            "INTERVAL" => {
                interval = value
                    .parse()
                    .ok()
                    .filter(|n| (1..=MAX_INTERVAL).contains(n))?
            }
            "BYDAY" => by_day = Some(value),
            "BYMONTHDAY" => {
                month_day = Some(value.parse::<u32>().ok().filter(|d| (1..=31).contains(d))?)
            }
            "COUNT" => {
                count = Some(
                    value
                        .parse::<u32>()
                        .ok()
                        .filter(|n| (1..=MAX_COUNT).contains(n))?,
                )
            }
            "UNTIL" => until = Some(value.to_ascii_uppercase()),
            "WKST" => week_start = Some(value.to_ascii_uppercase()),
            _ => return None,
        }
    }
    let freq = freq?;
    let end = match (count, until) {
        (Some(_), Some(_)) => return None,
        (Some(count), None) => End::Count(count),
        (None, Some(until)) => End::UntilRaw(until),
        (None, None) => End::Never,
    };
    let weekday = |token: &str| WEEKDAYS.iter().position(|d| d.eq_ignore_ascii_case(token));
    let mut rule = Rule {
        freq,
        interval,
        weekdays: Vec::new(),
        month_day: None,
        nth_weekday: None,
        week_start: 0,
        end,
    };
    match (freq, by_day, month_day) {
        (_, None, None) => {}
        (Freq::Weekly, Some(days), None) => {
            let mut weekdays = days
                .split(',')
                .map(|d| weekday(d.trim()))
                .collect::<Option<Vec<_>>>()?;
            weekdays.sort_unstable();
            weekdays.dedup();
            if interval > 1 && weekdays.len() > 1 {
                rule.week_start = match week_start.as_deref() {
                    None => 0,
                    Some(day) => weekday(day)?,
                };
            }
            rule.weekdays = weekdays;
        }
        (Freq::Monthly, Some(day), None) => {
            // Sliced by bytes below: a server's malformed value must not
            // split a character.
            if !day.is_ascii() {
                return None;
            }
            let split = day.len().checked_sub(2)?;
            let (nth, name) = day.split_at(split);
            let nth = nth.trim_start_matches('+').parse::<i32>().ok()?;
            if !(nth == -1 || (1..=5).contains(&nth)) {
                return None;
            }
            rule.nth_weekday = Some((nth, weekday(name)?));
        }
        (Freq::Monthly, None, Some(day)) => rule.month_day = Some(day),
        _ => return None,
    }
    Some(rule)
}

/// The repeat the editor sent, as `events.rrule` keeps it: empty for none, a
/// plain choice, or a canonical rule. Whether it suits the event's dates is
/// [`check`]'s to say, once the series' zone is settled.
pub fn from_editor(repeat: &str) -> Result<String> {
    let repeat = repeat.trim();
    if repeat.is_empty() {
        return Ok(String::new());
    }
    if let Some((plain, _)) = PLAIN
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(repeat))
    {
        return Ok(plain.to_string());
    }
    let mut rule = parse(repeat).ok_or_else(|| anyhow!("unknown repeat: {repeat}"))?;
    if let End::UntilRaw(raw) = &rule.end {
        let date = NaiveDate::parse_from_str(raw, "%Y%m%d")
            .map_err(|_| anyhow!("a repeat's end is a date: {raw}"))?;
        rule.end = End::Until(date);
    }
    Ok(rule.stored())
}

/// Whether a series starting on `first` is one of its rule's own dates:
/// RFC 5545 leaves a start the rule does not match to each client to read
/// its own way, so the editor never makes one.
fn fits(rule: &Rule, first: NaiveDate) -> bool {
    let weekday = first.weekday().num_days_from_monday() as usize;
    if !rule.weekdays.is_empty() && !rule.weekdays.contains(&weekday) {
        return false;
    }
    if rule.month_day.is_some_and(|day| day != first.day()) {
        return false;
    }
    match rule.nth_weekday {
        Some((-1, day)) => day == weekday && is_last(first),
        Some((nth, day)) => day == weekday && nth == nth_of(first),
        None => true,
    }
}

/// Which of its weekday in the month a date is, 1–5.
fn nth_of(date: NaiveDate) -> i32 {
    (date.day() as i32 - 1) / 7 + 1
}

/// The last of its weekday in the month.
fn is_last(date: NaiveDate) -> bool {
    (date + chrono::Days::new(7)).month() != date.month()
}

/// A rule the editor made or kept, fit to go with a series starting on
/// `first`: one that ends before it, or leaves it out, is refused.
pub fn check(stored: &str, first: NaiveDate) -> Result<()> {
    let Some(rule) = parse(stored) else {
        return Ok(());
    };
    if let End::UntilRaw(raw) = &rule.end
        && NaiveDate::parse_from_str(raw, "%Y%m%d").is_ok_and(|date| date < first)
    {
        bail!("the repeat ends before the event starts");
    }
    if !fits(&rule, first) {
        bail!("the repeat leaves out the day the event starts on");
    }
    Ok(())
}

/// A rule moved with its series, from starting on `from` to `to`: weekly
/// days move by as many days, and a monthly day or weekday becomes the new
/// start's. The editor keeps an existing series' rule as of its first day,
/// so it is moved here once, by the move in the series' own zone.
pub fn align(stored: &str, from: NaiveDate, to: NaiveDate) -> String {
    let Some(mut rule) = parse(stored) else {
        return stored.to_string();
    };
    let weekday = to.weekday().num_days_from_monday() as usize;
    if !rule.weekdays.is_empty() {
        let shift = (to - from).num_days().rem_euclid(7) as usize;
        // The weeks move with the days, so an every-other-week rule keeps
        // the same weeks: the whole pattern moves by as many days.
        if rule.interval > 1 && rule.weekdays.len() > 1 {
            rule.week_start = (rule.week_start + shift) % 7;
        }
        let mut days: Vec<usize> = rule.weekdays.iter().map(|d| (d + shift) % 7).collect();
        if !days.contains(&weekday) {
            days.push(weekday);
        }
        days.sort_unstable();
        days.dedup();
        rule.weekdays = days;
    }
    if rule.month_day.is_some() {
        rule.month_day = Some(to.day());
    }
    if let Some((nth, _)) = rule.nth_weekday {
        let nth = if nth == -1 && is_last(to) {
            -1
        } else {
            nth_of(to)
        };
        rule.nth_weekday = Some((nth, weekday));
    }
    rule.stored()
}

/// How a series written into iCalendar names its times: dates for an
/// all-day one, wall-clock times with no zone for a floating one, and
/// otherwise instants, whose dates are read in `tz`.
#[derive(Debug, Clone, Copy)]
pub enum Timing<'a> {
    AllDay,
    Floating,
    Zoned(&'a Tz),
}

/// The RRULE value for a stored repeat: an end date becomes the end of that
/// day, as the series' DTSTART says times (RFC 5545 asks UNTIL to match it).
pub fn to_ics(stored: &str, timing: Timing) -> String {
    if let Some((_, freq)) = PLAIN.iter().find(|(name, _)| *name == stored) {
        return format!("FREQ={}", freq.name());
    }
    let Some(mut rule) = parse(stored) else {
        return stored.to_string();
    };
    if let End::UntilRaw(raw) = &rule.end
        && let Ok(date) = NaiveDate::parse_from_str(raw, "%Y%m%d")
    {
        rule.end = End::Until(date);
    }
    let last_second = NaiveTime::from_hms_opt(23, 59, 59).unwrap_or_default();
    rule.write(|date| match timing {
        Timing::AllDay => date.format("%Y%m%d").to_string(),
        Timing::Floating => date
            .and_time(last_second)
            .format("%Y%m%dT%H%M%S")
            .to_string(),
        Timing::Zoned(tz) => tz
            .from_local_datetime(&date.and_time(last_second))
            .latest()
            .map(|at| at.naive_utc())
            .unwrap_or_else(|| date.and_time(last_second))
            .format("%Y%m%dT%H%M%SZ")
            .to_string(),
    })
}

/// A server's RRULE as `events.rrule` keeps it, or `None` when the editor
/// cannot say it. `first` is the series' first start, as `timing` reads it,
/// for an end given as an instant: its date is the last day an occurrence
/// still starts on, which is the day before when the instant falls earlier
/// in the day than the series' time.
pub fn from_ics(value: &str, timing: Timing, first: i64) -> Option<String> {
    let mut rule = parse(value)?;
    if let End::UntilRaw(raw) = &rule.end {
        rule.end = End::Until(until_date(raw, timing, first)?);
    }
    Some(rule.stored())
}

fn until_date(raw: &str, timing: Timing, first: i64) -> Option<NaiveDate> {
    if let Ok(date) = NaiveDate::parse_from_str(raw, "%Y%m%d") {
        return Some(date);
    }
    let (instant, utc) = match raw.strip_suffix('Z') {
        Some(instant) => (instant, true),
        None => (raw, false),
    };
    let instant = NaiveDateTime::parse_from_str(instant, "%Y%m%dT%H%M%S").ok()?;
    let start = chrono::DateTime::from_timestamp(first, 0)?;
    let (until, start) = match timing {
        Timing::AllDay => return Some(instant.date()),
        Timing::Zoned(tz) if utc => (
            tz.from_utc_datetime(&instant).naive_local(),
            tz.from_utc_datetime(&start.naive_utc()).naive_local(),
        ),
        // A floating series' start is read in the device's zone; so is a
        // floating end.
        _ => {
            let tz = super::local_tz();
            (
                instant,
                tz.from_utc_datetime(&start.naive_utc()).naive_local(),
            )
        }
    };
    if until.time() < start.time() {
        until.date().pred_opt()
    } else {
        Some(until.date())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(value: &str) -> NaiveDate {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn editor_rules_are_kept_canonical_and_plain_ones_by_name() {
        let first = day("2026-11-20");
        assert_eq!(from_editor("").unwrap(), "");
        assert_eq!(from_editor("Weekly").unwrap(), "weekly");
        assert_eq!(from_editor("FREQ=WEEKLY;INTERVAL=1").unwrap(), "weekly");
        assert_eq!(
            from_editor("freq=weekly;byday=we,mo,we;interval=2").unwrap(),
            "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE"
        );
        assert_eq!(
            from_editor("FREQ=MONTHLY;BYDAY=-1FR;COUNT=6").unwrap(),
            "FREQ=MONTHLY;BYDAY=-1FR;COUNT=6"
        );
        assert_eq!(
            from_editor("FREQ=MONTHLY;BYMONTHDAY=20;UNTIL=20270630").unwrap(),
            "FREQ=MONTHLY;BYMONTHDAY=20;UNTIL=20270630"
        );
        for refused in [
            "custom",
            "FREQ=HOURLY",
            "FREQ=DAILY;BYDAY=MO",
            "FREQ=WEEKLY;BYMONTHDAY=3",
            "FREQ=MONTHLY;BYDAY=6TU",
            "FREQ=WEEKLY;COUNT=2;UNTIL=20270101",
            "FREQ=WEEKLY;COUNT=0",
            "FREQ=WEEKLY;UNTIL=20270101T000000Z",
            "FREQ=YEARLY;BYMONTH=3",
        ] {
            assert!(from_editor(refused).is_err(), "{refused}");
        }
        // Friday 20 November 2026.
        let early = check("FREQ=DAILY;UNTIL=20261119", first).unwrap_err();
        assert!(early.to_string().contains("ends before"), "{early}");
        let skipped = check("FREQ=WEEKLY;BYDAY=MO", first).unwrap_err();
        assert!(skipped.to_string().contains("leaves out"), "{skipped}");
        for fits in [
            "weekly",
            "FREQ=WEEKLY;BYDAY=MO,FR;UNTIL=20261120",
            "FREQ=MONTHLY;BYMONTHDAY=20",
            "FREQ=MONTHLY;BYDAY=3FR",
        ] {
            assert!(check(fits, first).is_ok(), "{fits}");
        }
        assert!(check("FREQ=MONTHLY;BYDAY=-1FR", first).is_err());
    }

    #[test]
    fn a_rule_moves_with_its_series() {
        let friday = day("2026-11-20");
        let tuesday = day("2026-11-24");
        // Friday and Monday, moved four days on: Tuesday and Friday.
        assert_eq!(
            align("FREQ=WEEKLY;BYDAY=MO,FR;COUNT=4", friday, tuesday),
            "FREQ=WEEKLY;BYDAY=TU,FR;COUNT=4"
        );
        // Every day moves, even one the new start already falls on.
        assert_eq!(
            align("FREQ=WEEKLY;BYDAY=TU,FR", friday, tuesday),
            "FREQ=WEEKLY;BYDAY=TU,SA"
        );
        assert_eq!(
            align("FREQ=MONTHLY;BYMONTHDAY=20", friday, tuesday),
            "FREQ=MONTHLY;BYMONTHDAY=24"
        );
        // The third Friday to the fourth Tuesday; the last stays the last.
        assert_eq!(
            align("FREQ=MONTHLY;BYDAY=3FR", friday, tuesday),
            "FREQ=MONTHLY;BYDAY=4TU"
        );
        assert_eq!(
            align("FREQ=MONTHLY;BYDAY=-1FR", day("2026-11-27"), tuesday),
            "FREQ=MONTHLY;BYDAY=-1TU"
        );
        assert_eq!(align("daily", friday, tuesday), "daily");
    }

    #[test]
    fn an_end_date_is_written_as_the_series_says_times() {
        let rule = "FREQ=WEEKLY;BYDAY=MO;UNTIL=20270630";
        assert_eq!(to_ics("weekly", Timing::AllDay), "FREQ=WEEKLY");
        assert_eq!(to_ics(rule, Timing::AllDay), rule);
        assert_eq!(
            to_ics(rule, Timing::Floating),
            "FREQ=WEEKLY;BYDAY=MO;UNTIL=20270630T235959"
        );
        let tokyo: Tz = "Asia/Tokyo".parse().unwrap();
        assert_eq!(
            to_ics(rule, Timing::Zoned(&tokyo)),
            "FREQ=WEEKLY;BYDAY=MO;UNTIL=20270630T145959Z"
        );
    }

    #[test]
    fn a_malformed_monthly_weekday_is_left_to_the_server() {
        for rule in [
            "FREQ=MONTHLY;BYDAY=1éA",
            "FREQ=MONTHLY;BYDAY=éA",
            "FREQ=MONTHLY;BYDAY=7MO",
        ] {
            assert_eq!(from_ics(rule, Timing::AllDay, 0), None, "{rule}");
        }
    }

    #[test]
    fn a_servers_end_instant_reads_as_the_last_day_an_occurrence_starts() {
        let tokyo: Tz = "Asia/Tokyo".parse().unwrap();
        // 09:00 Tokyo on 2026-11-20.
        let first = tokyo
            .from_local_datetime(&day("2026-11-20").and_hms_opt(9, 0, 0).unwrap())
            .unwrap()
            .timestamp();
        // 2027-06-30 09:00 Tokyo, the last occurrence itself.
        assert_eq!(
            from_ics(
                "FREQ=WEEKLY;UNTIL=20270630T000000Z",
                Timing::Zoned(&tokyo),
                first
            )
            .as_deref(),
            Some("FREQ=WEEKLY;UNTIL=20270630")
        );
        // 2027-06-30 08:00 Tokyo: that day's 09:00 is past the end.
        assert_eq!(
            from_ics(
                "FREQ=WEEKLY;UNTIL=20270629T230000Z",
                Timing::Zoned(&tokyo),
                first
            )
            .as_deref(),
            Some("FREQ=WEEKLY;UNTIL=20270629")
        );
        assert_eq!(
            from_ics("FREQ=YEARLY;UNTIL=20300101", Timing::AllDay, first).as_deref(),
            Some("FREQ=YEARLY;UNTIL=20300101")
        );
        assert_eq!(
            from_ics("FREQ=DAILY;INTERVAL=1;WKST=SU", Timing::AllDay, first).as_deref(),
            Some("daily")
        );
        assert_eq!(
            from_ics(
                "FREQ=MONTHLY;BYSETPOS=-1;BYDAY=MO,TU",
                Timing::AllDay,
                first
            ),
            None
        );
        // A week start the editor keeps where it picks the weeks, and drops
        // where it does not.
        assert_eq!(
            from_ics(
                "FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,TH;WKST=SU",
                Timing::AllDay,
                first
            )
            .as_deref(),
            Some("FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,TH;WKST=SU")
        );
        assert_eq!(
            from_ics("FREQ=WEEKLY;BYDAY=TU,TH;WKST=SU", Timing::AllDay, first).as_deref(),
            Some("FREQ=WEEKLY;BYDAY=TU,TH")
        );
    }
}
