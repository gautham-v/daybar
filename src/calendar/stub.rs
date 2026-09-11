//! Deterministic in-memory calendar for development and tests.
//!
//! Generates a repeatable set of events from the date itself, so the popover
//! has something to render without Calendar.app access.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};

use crate::calendar::CalendarSource;
use crate::model::{sort_attendees, Event};

/// Stub calendar colours, roughly the macOS Calendar palette.
const BLUE: (u8, u8, u8) = (10, 122, 255);
const GREEN: (u8, u8, u8) = (52, 199, 89);
const ORANGE: (u8, u8, u8) = (255, 149, 0);
const PURPLE: (u8, u8, u8) = (175, 82, 222);
const RED: (u8, u8, u8) = (255, 59, 48);

/// Attendee names, normalized the way the EventKit mapping normalizes them.
fn people(names: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = names.iter().map(|n| n.to_string()).collect();
    sort_attendees(&mut out);
    out
}

/// A [`CalendarSource`] that synthesizes events deterministically from the date.
///
/// The same date always produces the same events, so snapshots and tests are
/// stable.
#[derive(Debug, Default, Clone)]
pub struct StubSource;

impl StubSource {
    pub fn new() -> Self {
        Self
    }

    fn at(day: NaiveDate, hour: u32, minute: u32) -> NaiveDateTime {
        day.and_hms_opt(hour, minute, 0)
            .expect("hard-coded stub times are valid")
    }

    /// The events for one day. Pure and deterministic.
    fn events_on(&self, day: NaiveDate) -> Vec<Event> {
        let dom = day.day();
        let dow = day.weekday().num_days_from_sunday();
        let mut events = Vec::new();

        // Weekends are quiet apart from the occasional all-day item.
        if dow == 0 || dow == 6 {
            if dom.is_multiple_of(7) {
                events.push(Event {
                    id: format!("stub-{day}-offsite"),
                    title: "Offsite".into(),
                    location: Some("Point Reyes".into()),
                    start: Self::at(day, 0, 0),
                    end: Self::at(day, 23, 59),
                    all_day: true,
                    calendar_color: Some(GREEN),
                    notes: Some("Vans leave at 8. Bring a jacket.".into()),
                    url: None,
                    attendees: people(&["you", "Chetan", "Priya", "Marta"]),
                });
            }
            return events;
        }

        events.push(Event {
            id: format!("stub-{day}-standup"),
            title: "Standup".into(),
            location: Some("Zoom".into()),
            start: Self::at(day, 9, 30),
            end: Self::at(day, 9, 45),
            all_day: false,
            calendar_color: Some(BLUE),
            notes: Some(
                "Standing agenda: yesterday, today, blockers.\nhttps://acme.zoom.us/j/8410293\nPasscode 4821."
                    .into(),
            ),
            url: Some("https://acme.zoom.us/j/8410293".into()),
            attendees: people(&["you", "Priya", "Chetan", "Dan", "Marta"]),
        });

        if dom % 2 == 1 {
            events.push(Event {
                id: format!("stub-{day}-1on1"),
                title: "1:1 with Priya".into(),
                location: None,
                start: Self::at(day, 11, 0),
                end: Self::at(day, 11, 30),
                all_day: false,
                calendar_color: Some(PURPLE),
                notes: Some("Career conversation — pick up the thread from last month.".into()),
                // A plain doc link, not a meeting: exercises "Open link".
                url: Some("https://example.com/1-1-agenda".into()),
                attendees: people(&["Priya", "you"]),
            });
        }

        if dom.is_multiple_of(3) {
            events.push(Event {
                id: format!("stub-{day}-review"),
                title: "Design review — v2 popover, inline expand, native material".into(),
                location: Some("Room 4 — Kestrel".into()),
                start: Self::at(day, 14, 0),
                end: Self::at(day, 15, 0),
                all_day: false,
                calendar_color: Some(ORANGE),
                notes: Some(
                    "Walk through the v2 popover: inline expand, native material, calendar colours.\nDeck in the shared drive; 20 minutes of review, 40 of discussion."
                        .into(),
                ),
                url: None,
                attendees: people(&["Chetan", "you", "Marta", "Sam"]),
            });
        }

        if dom.is_multiple_of(5) {
            events.push(Event {
                id: format!("stub-{day}-holiday"),
                title: "Company holiday".into(),
                location: None,
                start: Self::at(day, 0, 0),
                end: Self::at(day, 23, 59),
                all_day: true,
                calendar_color: Some(RED),
                notes: None,
                url: None,
                attendees: Vec::new(),
            });
        }

        if dow == 5 {
            events.push(Event {
                id: format!("stub-{day}-demo"),
                title: "Demo & drinks".into(),
                location: Some("Kitchen".into()),
                start: Self::at(day, 16, 30),
                end: Self::at(day, 18, 0),
                all_day: false,
                calendar_color: Some(GREEN),
                notes: Some(
                    "Demos first, drinks after. Hybrid: https://meet.google.com/xkc-dpvq-mzu"
                        .into(),
                ),
                url: None,
                attendees: people(&["you", "Dan", "Sam"]),
            });
        }

        events
    }
}

impl CalendarSource for StubSource {
    fn events_between(&self, from: NaiveDate, to: NaiveDate) -> Vec<Event> {
        let mut out = Vec::new();
        let mut day = from;
        while day <= to {
            out.extend(self.events_on(day));
            day += Duration::days(1);
        }
        out
    }

    fn name(&self) -> &'static str {
        "stub"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn is_deterministic() {
        let s = StubSource::new();
        let a = s.events_between(d(2026, 9, 1), d(2026, 9, 30));
        let b = s.events_between(d(2026, 9, 1), d(2026, 9, 30));
        assert_eq!(a, b);
        assert!(!a.is_empty());
    }

    #[test]
    fn respects_the_range_bounds() {
        let s = StubSource::new();
        let events = s.events_between(d(2026, 9, 7), d(2026, 9, 9));
        assert!(events
            .iter()
            .all(|e| { e.start.date() >= d(2026, 9, 7) && e.start.date() <= d(2026, 9, 9) }));
    }

    #[test]
    fn empty_range_when_to_is_before_from() {
        let s = StubSource::new();
        assert!(s.events_between(d(2026, 9, 9), d(2026, 9, 7)).is_empty());
    }

    #[test]
    fn stub_carries_v2_detail_fields() {
        let s = StubSource::new();
        let events = s.events_between(d(2026, 9, 1), d(2026, 9, 30));
        assert!(events.iter().any(|e| e.calendar_color.is_some()));
        assert!(events.iter().any(|e| e.notes.is_some()));
        assert!(events.iter().any(|e| !e.attendees.is_empty()));
        let standup = events
            .iter()
            .find(|e| e.title == "Standup")
            .expect("weekdays have a standup");
        assert_eq!(
            standup.join_url().as_deref(),
            Some("https://acme.zoom.us/j/8410293")
        );
        assert_eq!(standup.attendees.last().map(String::as_str), Some("you"));
    }

    #[test]
    fn ids_are_unique() {
        let s = StubSource::new();
        let events = s.events_between(d(2026, 9, 1), d(2026, 10, 31));
        let mut ids: Vec<&str> = events.iter().map(|e| e.id.as_str()).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count);
    }
}
