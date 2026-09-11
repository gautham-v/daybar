//! Cached, shareable access to a [`CalendarSource`].
//!
//! The store owns the source, keeps a window of events grouped by day, and is
//! cheap to clone — the UI holds a handle, a background refresh holds another.
//! No gpui types appear here on purpose.

use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};

use chrono::{Duration, Local, NaiveDate, NaiveDateTime};

use crate::calendar::{AccessState, CalendarSource};
use crate::model::{sort_for_day, Event};

/// Days of history kept in the cache.
pub const WINDOW_BACK_DAYS: i64 = 60;
/// Days of future kept in the cache.
pub const WINDOW_AHEAD_DAYS: i64 = 90;

/// The `[from, to]` window around `today` that the store caches.
pub fn window_around(today: NaiveDate) -> (NaiveDate, NaiveDate) {
    (
        today - Duration::days(WINDOW_BACK_DAYS),
        today + Duration::days(WINDOW_AHEAD_DAYS),
    )
}

struct Cache {
    by_date: BTreeMap<NaiveDate, Vec<Event>>,
    window: (NaiveDate, NaiveDate),
    access: AccessState,
    /// Bumped on every successful refresh so the UI can tell whether the data
    /// it rendered is still current.
    generation: u64,
}

/// A shared, cached view of the calendar.
///
/// The source and the cache sit behind *separate* mutexes on purpose: fetching
/// (and, on first run, the calendar-access prompt) can block for many seconds,
/// and the UI reads the cache on the main thread on every render. Only the
/// source lock is held across the slow calls; the cache lock is taken for the
/// swap at the end.
#[derive(Clone)]
pub struct CalendarStore {
    source: Arc<Mutex<Box<dyn CalendarSource + Send>>>,
    cache: Arc<Mutex<Cache>>,
}

impl CalendarStore {
    /// Wrap a source. Does not fetch; call [`CalendarStore::refresh`].
    pub fn new(source: Box<dyn CalendarSource + Send>) -> Self {
        let today = Local::now().date_naive();
        let access = source.access_state();
        Self {
            source: Arc::new(Mutex::new(source)),
            cache: Arc::new(Mutex::new(Cache {
                by_date: BTreeMap::new(),
                window: window_around(today),
                access,
                generation: 0,
            })),
        }
    }

    /// Prompt for calendar access if the source needs it, then record the
    /// resulting state. Blocks; call it from a background thread.
    ///
    /// Holds only the source lock while the system prompt is up, so the UI can
    /// keep rendering from the cache meanwhile.
    pub fn request_access(&self) -> AccessState {
        let access = {
            let mut source = self.lock_source();
            source.ensure_access()
        };
        self.lock().access = access;
        access
    }

    /// Refetch the whole window around today.
    pub fn refresh(&self) {
        self.refresh_as_of(Local::now().date_naive());
    }

    /// Refetch the window around an explicit "today" (testable seam).
    pub fn refresh_as_of(&self, today: NaiveDate) {
        let (from, to) = window_around(today);
        // Fetch with only the source locked: EventKit can take hundreds of ms
        // and the main thread reads the cache on every frame.
        let (events, access) = {
            let mut source = self.lock_source();
            // The user can flip the permission in System Settings while we run.
            let access = source.refresh_access();
            let events = if access.can_read() {
                source.events_between(from, to)
            } else {
                Vec::new()
            };
            (events, access)
        };
        let by_date = group_by_date(events, from, to);
        let mut cache = self.lock();
        cache.access = access;
        cache.by_date = by_date;
        cache.window = (from, to);
        cache.generation += 1;
    }

    /// The events for one day, all-day first then by start time.
    ///
    /// Returns an empty `Vec` for a day outside the cached window; the caller
    /// is expected to keep navigation inside it (the popover does).
    pub fn events_on(&self, date: NaiveDate) -> Vec<Event> {
        self.lock().by_date.get(&date).cloned().unwrap_or_default()
    }

    /// Just the count, without cloning the events.
    pub fn count_on(&self, date: NaiveDate) -> usize {
        self.lock().by_date.get(&date).map_or(0, Vec::len)
    }

    /// True if `date` has at least one event — for the month grid's dot.
    pub fn has_events_on(&self, date: NaiveDate) -> bool {
        self.count_on(date) > 0
    }

    /// Events for each day of `dates`, in order — for the week list.
    pub fn events_for_days(&self, dates: &[NaiveDate]) -> Vec<(NaiveDate, Vec<Event>)> {
        let inner = self.lock();
        dates
            .iter()
            .map(|d| (*d, inner.by_date.get(d).cloned().unwrap_or_default()))
            .collect()
    }

    pub fn access_state(&self) -> AccessState {
        self.lock().access
    }

    pub fn set_access_state(&self, access: AccessState) {
        self.lock().access = access;
    }

    pub fn source_name(&self) -> &'static str {
        self.lock_source().name()
    }

    /// The cached `[from, to]` window.
    pub fn window(&self) -> (NaiveDate, NaiveDate) {
        self.lock().window
    }

    /// Increments on every refresh.
    pub fn generation(&self) -> u64 {
        self.lock().generation
    }

    /// Total cached events, for diagnostics and tests.
    pub fn len(&self) -> usize {
        self.lock().by_date.values().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Cache> {
        // A panic inside the store would leave the cache stale but valid, so
        // keep using it rather than poisoning the whole app.
        self.cache.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn lock_source(&self) -> std::sync::MutexGuard<'_, Box<dyn CalendarSource + Send>> {
        self.source.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Group events by the days they occupy, clipped to `[from, to]`, sorted
/// all-day-first then by start time.
///
/// A multi-day event appears under every day it touches — the day list should
/// show a trip on each of its days.
pub fn group_by_date(
    events: Vec<Event>,
    from: NaiveDate,
    to: NaiveDate,
) -> BTreeMap<NaiveDate, Vec<Event>> {
    let mut map: BTreeMap<NaiveDate, Vec<Event>> = BTreeMap::new();
    for event in events {
        let first = event.start.date().max(from);
        let last = event.end.date().min(to);
        let mut day = first;
        while day <= last {
            map.entry(day).or_default().push(event.clone());
            day += Duration::days(1);
        }
    }
    for events in map.values_mut() {
        sort_for_day(events);
        dedupe_identical(events);
    }
    map
}

/// Drop later duplicates of the same event within one day.
///
/// A subscribed calendar (US holidays, say) synced into several accounts hands
/// EventKit one copy per account with different identifiers, so "Labor Day"
/// shows up three times. Two events count as the same when their title, start,
/// end and all-day flag all match; the first one wins. Runs after
/// [`sort_for_day`], which already puts identical events next to each other.
fn dedupe_identical(events: &mut Vec<Event>) {
    let mut seen: HashSet<(String, NaiveDateTime, NaiveDateTime, bool)> = HashSet::new();
    events
        .retain(|event| seen.insert((event.title.clone(), event.start, event.end, event.all_day)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::stub::StubSource;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn at(day: NaiveDate, h: u32, m: u32) -> NaiveDateTime {
        day.and_hms_opt(h, m, 0).unwrap()
    }

    fn ev(id: &str, start: NaiveDateTime, end: NaiveDateTime, all_day: bool) -> Event {
        Event {
            id: id.into(),
            title: id.into(),
            location: None,
            start,
            end,
            all_day,
            ..Event::default()
        }
    }

    #[test]
    fn groups_single_day_events_under_their_day() {
        let a = ev(
            "a",
            at(d(2026, 9, 11), 9, 0),
            at(d(2026, 9, 11), 10, 0),
            false,
        );
        let b = ev(
            "b",
            at(d(2026, 9, 12), 9, 0),
            at(d(2026, 9, 12), 10, 0),
            false,
        );
        let map = group_by_date(vec![a, b], d(2026, 9, 1), d(2026, 9, 30));
        assert_eq!(map[&d(2026, 9, 11)].len(), 1);
        assert_eq!(map[&d(2026, 9, 12)].len(), 1);
        assert!(!map.contains_key(&d(2026, 9, 13)));
    }

    #[test]
    fn multi_day_events_appear_on_every_day_they_touch() {
        let trip = ev(
            "trip",
            at(d(2026, 9, 10), 8, 0),
            at(d(2026, 9, 12), 20, 0),
            false,
        );
        let map = group_by_date(vec![trip], d(2026, 9, 1), d(2026, 9, 30));
        for day in 10..=12 {
            assert_eq!(map[&d(2026, 9, day)].len(), 1, "missing day {day}");
        }
        assert!(!map.contains_key(&d(2026, 9, 9)));
        assert!(!map.contains_key(&d(2026, 9, 13)));
    }

    #[test]
    fn events_are_clipped_to_the_window() {
        let trip = ev(
            "trip",
            at(d(2026, 8, 28), 8, 0),
            at(d(2026, 9, 3), 20, 0),
            true,
        );
        let map = group_by_date(vec![trip], d(2026, 9, 1), d(2026, 9, 2));
        assert_eq!(
            map.keys().copied().collect::<Vec<_>>(),
            vec![d(2026, 9, 1), d(2026, 9, 2)]
        );
    }

    #[test]
    fn all_day_events_come_first_then_start_time() {
        let day = d(2026, 9, 11);
        let events = vec![
            ev("afternoon", at(day, 14, 0), at(day, 15, 0), false),
            ev("morning", at(day, 9, 0), at(day, 9, 30), false),
            ev("holiday", at(day, 0, 0), at(day, 23, 59), true),
            ev("early", at(day, 8, 0), at(day, 8, 15), false),
        ];
        let map = group_by_date(events, day, day);
        let order: Vec<&str> = map[&day].iter().map(|e| e.id.as_str()).collect();
        assert_eq!(order, vec!["holiday", "early", "morning", "afternoon"]);
    }

    #[test]
    fn ties_on_start_break_by_title_so_order_is_stable() {
        let day = d(2026, 9, 11);
        let events = vec![
            ev("zeta", at(day, 9, 0), at(day, 10, 0), false),
            ev("alpha", at(day, 9, 0), at(day, 10, 0), false),
        ];
        let map = group_by_date(events, day, day);
        let order: Vec<&str> = map[&day].iter().map(|e| e.id.as_str()).collect();
        assert_eq!(order, vec!["alpha", "zeta"]);
    }

    #[test]
    fn dedupe_ignores_the_v2_detail_fields() {
        let day = d(2026, 9, 7);
        let mut plain = ev("a", at(day, 9, 0), at(day, 10, 0), false);
        plain.title = "Sync".into();
        let mut rich = plain.clone();
        rich.id = "b".into();
        rich.calendar_color = Some((1, 2, 3));
        rich.notes = Some("agenda".into());
        rich.attendees = vec!["Priya".into(), "you".into()];
        let map = group_by_date(vec![plain, rich], day, day);
        assert_eq!(map[&day].len(), 1);
    }

    #[test]
    fn identical_events_from_several_calendars_are_deduped() {
        let day = d(2026, 9, 7);
        let holiday = |id: &str| Event {
            id: id.into(),
            title: "Labor Day".into(),
            location: None,
            start: at(day, 0, 0),
            end: at(day, 23, 59),
            all_day: true,
            ..Event::default()
        };
        let mut other = holiday("other");
        other.title = "Labor Day (observed)".into();
        let map = group_by_date(
            vec![
                holiday("work"),
                holiday("personal"),
                holiday("school"),
                other,
            ],
            day,
            day,
        );
        let titles: Vec<&str> = map[&day].iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, vec!["Labor Day", "Labor Day (observed)"]);
        // The first copy is the one kept.
        assert_eq!(map[&day][0].id, "work");
    }

    #[test]
    fn same_title_at_different_times_is_not_a_duplicate() {
        let day = d(2026, 9, 11);
        let map = group_by_date(
            vec![
                ev("standup", at(day, 9, 0), at(day, 9, 15), false),
                ev("standup", at(day, 17, 0), at(day, 17, 15), false),
            ],
            day,
            day,
        );
        assert_eq!(map[&day].len(), 2);
    }

    #[test]
    fn store_caches_and_serves_days() {
        let store = CalendarStore::new(Box::new(StubSource::new()));
        assert!(store.is_empty());
        let today = d(2026, 9, 11);
        store.refresh_as_of(today);
        assert_eq!(store.window(), window_around(today));
        assert!(!store.is_empty());
        // Sep 11 2026 is a Friday: standup, 1:1 (odd day), demo.
        let events = store.events_on(today);
        assert!(!events.is_empty());
        assert_eq!(store.count_on(today), events.len());
        assert!(store.has_events_on(today));
        assert_eq!(store.source_name(), "stub");
    }

    #[test]
    fn refresh_bumps_the_generation() {
        let store = CalendarStore::new(Box::new(StubSource::new()));
        assert_eq!(store.generation(), 0);
        store.refresh_as_of(d(2026, 9, 11));
        assert_eq!(store.generation(), 1);
        store.refresh_as_of(d(2026, 9, 11));
        assert_eq!(store.generation(), 2);
    }

    #[test]
    fn clones_share_one_cache() {
        let store = CalendarStore::new(Box::new(StubSource::new()));
        let handle = store.clone();
        store.refresh_as_of(d(2026, 9, 11));
        assert_eq!(handle.len(), store.len());
        assert!(handle.generation() == 1);
    }

    #[test]
    fn days_outside_the_window_are_empty_not_a_panic() {
        let store = CalendarStore::new(Box::new(StubSource::new()));
        store.refresh_as_of(d(2026, 9, 11));
        assert!(store.events_on(d(2030, 1, 1)).is_empty());
        assert_eq!(store.count_on(d(2000, 1, 1)), 0);
    }

    #[test]
    fn events_for_days_returns_one_entry_per_day_in_order() {
        let store = CalendarStore::new(Box::new(StubSource::new()));
        store.refresh_as_of(d(2026, 9, 11));
        let week = crate::model::week_containing(d(2026, 9, 11));
        let rows = store.events_for_days(&week);
        assert_eq!(rows.len(), 7);
        assert_eq!(rows[0].0, d(2026, 9, 6));
        assert_eq!(rows[6].0, d(2026, 9, 12));
    }

    #[test]
    fn window_is_sixty_back_ninety_ahead() {
        let (from, to) = window_around(d(2026, 9, 11));
        assert_eq!(from, d(2026, 7, 13));
        assert_eq!(to, d(2026, 12, 10));
    }

    #[test]
    fn refresh_picks_up_access_granted_after_construction() {
        // A source that starts denied and is granted in System Settings later.
        struct Flipping {
            access: AccessState,
        }
        impl CalendarSource for Flipping {
            fn events_between(&self, from: NaiveDate, _: NaiveDate) -> Vec<Event> {
                vec![ev("granted", at(from, 9, 0), at(from, 10, 0), false)]
            }
            fn access_state(&self) -> AccessState {
                self.access
            }
            fn refresh_access(&mut self) -> AccessState {
                self.access = AccessState::Granted;
                self.access
            }
        }
        let store = CalendarStore::new(Box::new(Flipping {
            access: AccessState::Denied,
        }));
        assert_eq!(store.access_state(), AccessState::Denied);
        store.refresh_as_of(d(2026, 9, 11));
        assert_eq!(store.access_state(), AccessState::Granted);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn the_cache_stays_readable_while_a_slow_fetch_is_in_flight() {
        use std::sync::mpsc;
        struct Blocking {
            gate: Mutex<mpsc::Receiver<()>>,
        }
        impl CalendarSource for Blocking {
            fn events_between(&self, _: NaiveDate, _: NaiveDate) -> Vec<Event> {
                let _ = self.gate.lock().unwrap().recv();
                Vec::new()
            }
        }
        let (tx, rx) = mpsc::channel();
        let store = CalendarStore::new(Box::new(Blocking {
            gate: Mutex::new(rx),
        }));
        let worker = store.clone();
        let handle = std::thread::spawn(move || worker.refresh_as_of(d(2026, 9, 11)));
        // Would deadlock until the fetch finishes if one mutex covered both.
        assert!(store.events_on(d(2026, 9, 11)).is_empty());
        assert_eq!(store.generation(), 0);
        tx.send(()).unwrap();
        handle.join().unwrap();
        assert_eq!(store.generation(), 1);
    }

    #[test]
    fn denied_source_yields_an_empty_cache_and_a_message() {
        struct Denied;
        impl CalendarSource for Denied {
            fn events_between(&self, _: NaiveDate, _: NaiveDate) -> Vec<Event> {
                Vec::new()
            }
            fn access_state(&self) -> AccessState {
                AccessState::Denied
            }
        }
        let store = CalendarStore::new(Box::new(Denied));
        store.refresh_as_of(d(2026, 9, 11));
        assert!(store.is_empty());
        assert_eq!(store.access_state(), AccessState::Denied);
        assert!(store.access_state().message().is_some());
        assert!(!store.access_state().can_read());
    }
}
