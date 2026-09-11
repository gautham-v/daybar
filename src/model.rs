//! Pure data model and date helpers. No Cocoa, no gpui — all unit-testable.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike, Weekday};

/// A single calendar event, normalized away from any particular source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// Stable identifier from the source (EventKit identifier, etc.).
    pub id: String,
    pub title: String,
    pub location: Option<String>,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub all_day: bool,
}

impl Event {
    /// Does this event touch `day` at all?
    pub fn occurs_on(&self, day: NaiveDate) -> bool {
        self.start.date() <= day && day <= self.end.date()
    }
}

/// One cell of the month grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayCell {
    pub date: NaiveDate,
    /// False for the leading/trailing days that belong to the neighbouring months.
    pub in_month: bool,
}

/// One row of the month grid: seven days plus the ISO week number for that row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeekRow {
    /// ISO-8601 week number, taken from the row's Thursday.
    pub week_number: u32,
    pub days: [DayCell; 7],
}

/// The 6x7 Sunday-start grid for the month containing `anchor`.
///
/// Always six rows, so the popover never changes height between months.
pub fn month_grid(anchor: NaiveDate) -> [WeekRow; 6] {
    let first = NaiveDate::from_ymd_opt(anchor.year(), anchor.month(), 1)
        .expect("year/month from an existing date is always valid");
    // Sunday = 0 .. Saturday = 6
    let lead = first.weekday().num_days_from_sunday() as i64;
    let origin = first - Duration::days(lead);

    let month = anchor.month();
    let rows: Vec<WeekRow> = (0..6)
        .map(|row| {
            let days: [DayCell; 7] = std::array::from_fn(|col| {
                let date = origin + Duration::days(row * 7 + col as i64);
                DayCell {
                    date,
                    in_month: date.month() == month && date.year() == anchor.year(),
                }
            });
            WeekRow {
                week_number: iso_week_number(days[0].date),
                days,
            }
        })
        .collect();

    rows.try_into().expect("built exactly 6 rows")
}

/// ISO-8601 week number of the week (Sun–Sat) that starts on/contains `date`.
///
/// A Sunday-start row straddles two ISO weeks, so we take the number from the
/// row's Thursday — the same rule ISO itself uses to assign a week to a year.
pub fn iso_week_number(date: NaiveDate) -> u32 {
    let sunday = week_start(date);
    let thursday = sunday + Duration::days(4);
    thursday.iso_week().week()
}

/// The Sunday that begins the week containing `date`.
pub fn week_start(date: NaiveDate) -> NaiveDate {
    date - Duration::days(date.weekday().num_days_from_sunday() as i64)
}

/// The seven days, Sunday through Saturday, containing `date`.
pub fn week_containing(date: NaiveDate) -> [NaiveDate; 7] {
    let start = week_start(date);
    std::array::from_fn(|i| start + Duration::days(i as i64))
}

/// Google Calendar deep link for a single day.
pub fn google_calendar_day_url(date: NaiveDate) -> String {
    format!(
        "https://calendar.google.com/calendar/r/day/{}/{}/{}",
        date.year(),
        date.month(),
        date.day()
    )
}

/// Weekday letters for the header row, Sunday first.
pub const WEEKDAY_LETTERS: [&str; 7] = ["S", "M", "T", "W", "T", "F", "S"];

/// `"September 2026"`.
pub fn month_title(date: NaiveDate) -> String {
    date.format("%B %Y").to_string()
}

/// Sort order for a day's list: all-day events first, then by start time.
pub fn sort_for_day(events: &mut [Event]) {
    events.sort_by(|a, b| {
        b.all_day
            .cmp(&a.all_day)
            .then(a.start.cmp(&b.start))
            .then(a.title.cmp(&b.title))
    });
}

/// True if `date` is a weekend day.
pub fn is_weekend(date: NaiveDate) -> bool {
    matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
}

// ── Formatting helpers ───────────────────────────────────────────────────────

/// `"9:00"`, `"12:40"` — 12-hour clock, no zero padding, no meridiem
/// (matches `docs/mockup-popover.dc.html`).
pub fn short_time(t: NaiveTime) -> String {
    let h24 = t.hour();
    let h = match h24 % 12 {
        0 => 12,
        h => h,
    };
    format!("{}:{:02}", h, t.minute())
}

/// Title for the day list: `"Today"`, otherwise `"Thursday 11"`.
pub fn day_title(date: NaiveDate, today: NaiveDate) -> String {
    if date == today {
        "Today".to_string()
    } else {
        format!("{} {}", date.format("%A"), date.day())
    }
}

/// Right-hand summary: `"3 left"` for today, `"5 events"` otherwise,
/// empty when the day has nothing.
pub fn day_summary(count: usize, remaining: usize, is_today: bool) -> String {
    if count == 0 {
        String::new()
    } else if is_today {
        format!("{remaining} left")
    } else if count == 1 {
        "1 event".to_string()
    } else {
        format!("{count} events")
    }
}

/// Uppercase three-letter weekday for the week list: `"SUN"`.
pub fn dow_abbrev(date: NaiveDate) -> String {
    date.format("%a").to_string().to_uppercase()
}

/// Has this event already finished, relative to `now`?
pub fn is_past(end: NaiveDateTime, now: NaiveDateTime) -> bool {
    end <= now
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn grid_is_always_six_by_seven() {
        for month in 1..=12 {
            let grid = month_grid(d(2026, month, 15));
            assert_eq!(grid.len(), 6);
            for row in &grid {
                assert_eq!(row.days.len(), 7);
            }
        }
    }

    #[test]
    fn grid_starts_on_sunday_and_is_contiguous() {
        let grid = month_grid(d(2026, 9, 11));
        assert_eq!(grid[0].days[0].date.weekday(), Weekday::Sun);
        let flat: Vec<NaiveDate> = grid
            .iter()
            .flat_map(|r| r.days.iter().map(|c| c.date))
            .collect();
        let mut prev = flat[0];
        for date in flat.iter().skip(1) {
            assert_eq!(*date, prev + Duration::days(1));
            prev = *date;
        }
    }

    #[test]
    fn september_2026_leading_and_trailing_days() {
        // Sep 1 2026 is a Tuesday, so the grid starts Sun Aug 30.
        let grid = month_grid(d(2026, 9, 11));
        assert_eq!(grid[0].days[0].date, d(2026, 8, 30));
        assert!(!grid[0].days[0].in_month);
        assert_eq!(grid[0].days[2].date, d(2026, 9, 1));
        assert!(grid[0].days[2].in_month);
        // 6 rows * 7 = 42 days from Aug 30 -> last cell is Oct 10.
        assert_eq!(grid[5].days[6].date, d(2026, 10, 10));
        assert!(!grid[5].days[6].in_month);
    }

    #[test]
    fn month_starting_on_sunday_has_no_lead() {
        // Feb 1 2026 is a Sunday.
        let grid = month_grid(d(2026, 2, 20));
        assert_eq!(grid[0].days[0].date, d(2026, 2, 1));
        assert!(grid[0].days[0].in_month);
    }

    #[test]
    fn in_month_does_not_leak_across_years() {
        // January grid: the trailing cells are February, the leading ones December.
        let grid = month_grid(d(2027, 1, 5));
        let leading = grid[0].days[0];
        assert_eq!(leading.date.year(), 2026);
        assert!(!leading.in_month);
    }

    #[test]
    fn iso_week_numbers_use_the_rows_thursday() {
        // Week of Sun 2026-01-04 .. Sat 2026-01-10; Thursday is Jan 8 -> ISO week 2.
        assert_eq!(iso_week_number(d(2026, 1, 4)), 2);
        // Week of Sun 2026-09-06; Thursday Sep 10 -> ISO week 37.
        assert_eq!(iso_week_number(d(2026, 9, 6)), 37);
        // Any day in the row gives the same number.
        assert_eq!(iso_week_number(d(2026, 9, 11)), 37);
        assert_eq!(iso_week_number(d(2026, 9, 12)), 37);
    }

    #[test]
    fn grid_rows_carry_ascending_week_numbers() {
        let grid = month_grid(d(2026, 9, 11));
        let nums: Vec<u32> = grid.iter().map(|r| r.week_number).collect();
        assert_eq!(nums, vec![36, 37, 38, 39, 40, 41]);
    }

    #[test]
    fn week_start_is_sunday() {
        assert_eq!(week_start(d(2026, 9, 11)), d(2026, 9, 6));
        assert_eq!(week_start(d(2026, 9, 6)), d(2026, 9, 6));
        assert_eq!(week_start(d(2026, 9, 12)), d(2026, 9, 6));
    }

    #[test]
    fn week_containing_is_sunday_to_saturday() {
        let week = week_containing(d(2026, 9, 11));
        assert_eq!(week[0], d(2026, 9, 6));
        assert_eq!(week[6], d(2026, 9, 12));
        assert_eq!(week[0].weekday(), Weekday::Sun);
        assert_eq!(week[6].weekday(), Weekday::Sat);
    }

    #[test]
    fn google_url_has_no_zero_padding() {
        assert_eq!(
            google_calendar_day_url(d(2026, 9, 1)),
            "https://calendar.google.com/calendar/r/day/2026/9/1"
        );
        assert_eq!(
            google_calendar_day_url(d(2026, 12, 25)),
            "https://calendar.google.com/calendar/r/day/2026/12/25"
        );
    }

    #[test]
    fn month_title_formats() {
        assert_eq!(month_title(d(2026, 9, 11)), "September 2026");
    }

    #[test]
    fn all_day_events_sort_first() {
        let mk = |id: &str, h: u32, all_day: bool| Event {
            id: id.into(),
            title: id.into(),
            location: None,
            start: d(2026, 9, 11).and_hms_opt(h, 0, 0).unwrap(),
            end: d(2026, 9, 11).and_hms_opt(h + 1, 0, 0).unwrap(),
            all_day,
        };
        let mut events = vec![
            mk("b", 14, false),
            mk("a", 9, false),
            mk("holiday", 0, true),
        ];
        sort_for_day(&mut events);
        assert_eq!(
            events.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            vec!["holiday", "a", "b"]
        );
    }

    #[test]
    fn occurs_on_covers_multi_day_events() {
        let e = Event {
            id: "trip".into(),
            title: "Trip".into(),
            location: None,
            start: d(2026, 9, 10).and_hms_opt(8, 0, 0).unwrap(),
            end: d(2026, 9, 12).and_hms_opt(20, 0, 0).unwrap(),
            all_day: false,
        };
        assert!(e.occurs_on(d(2026, 9, 10)));
        assert!(e.occurs_on(d(2026, 9, 11)));
        assert!(e.occurs_on(d(2026, 9, 12)));
        assert!(!e.occurs_on(d(2026, 9, 13)));
    }

    #[test]
    fn weekend_detection() {
        assert!(is_weekend(d(2026, 9, 12)));
        assert!(is_weekend(d(2026, 9, 13)));
        assert!(!is_weekend(d(2026, 9, 11)));
    }

    #[test]
    fn short_time_is_twelve_hour_unpadded() {
        assert_eq!(
            short_time(NaiveTime::from_hms_opt(9, 0, 0).unwrap()),
            "9:00"
        );
        assert_eq!(
            short_time(NaiveTime::from_hms_opt(13, 5, 0).unwrap()),
            "1:05"
        );
        assert_eq!(
            short_time(NaiveTime::from_hms_opt(0, 30, 0).unwrap()),
            "12:30"
        );
        assert_eq!(
            short_time(NaiveTime::from_hms_opt(12, 40, 0).unwrap()),
            "12:40"
        );
    }

    #[test]
    fn day_title_says_today_for_today() {
        let today = d(2026, 9, 11);
        assert_eq!(day_title(today, today), "Today");
        assert_eq!(day_title(d(2026, 9, 14), today), "Monday 14");
    }

    #[test]
    fn summaries() {
        assert_eq!(day_summary(0, 0, true), "");
        assert_eq!(day_summary(5, 3, true), "3 left");
        assert_eq!(day_summary(5, 5, false), "5 events");
        assert_eq!(day_summary(1, 1, false), "1 event");
    }

    #[test]
    fn dow_abbrev_is_upper() {
        assert_eq!(dow_abbrev(d(2026, 9, 13)), "SUN");
        assert_eq!(dow_abbrev(d(2026, 9, 11)), "FRI");
    }

    #[test]
    fn past_is_inclusive_of_the_end_instant() {
        let now = d(2026, 9, 11).and_hms_opt(12, 40, 0).unwrap();
        assert!(is_past(d(2026, 9, 11).and_hms_opt(12, 40, 0).unwrap(), now));
        assert!(!is_past(
            d(2026, 9, 11).and_hms_opt(12, 41, 0).unwrap(),
            now
        ));
    }
}
