//! Small formatting helpers the views need.
//!
//! NOTE(integrator): these live here because `src/model.rs` was owned by
//! another agent while this was written. They are pure and belong in
//! `model.rs` once the branches are consolidated.

use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, Timelike};

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
