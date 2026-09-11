//! EventKit-backed [`CalendarSource`] — reads Calendar.app, where Google
//! Calendar is already synced.
//!
//! Access: on macOS 14+ the modern call is
//! `requestFullAccessToEventsWithCompletion:`; older systems only have the
//! deprecated `requestAccessToEntityType:completion:`. We pick between them by
//! asking the store whether it responds to the newer selector, and block on the
//! completion handler with a channel so callers get a synchronous answer.
//! Either way the app must be an .app bundle carrying
//! `NSCalendarsFullAccessUsageDescription` (see `scripts/bundle.sh`), or the
//! prompt never appears and access comes back denied.

use std::sync::mpsc;
use std::time::Duration as StdDuration;

use block2::RcBlock;
use chrono::{Duration, Local, NaiveDate, NaiveDateTime, TimeZone};
use objc2::rc::{autoreleasepool, Retained};
use objc2::runtime::{Bool, NSObjectProtocol};
use objc2::sel;
use objc2_core_graphics::{CGColor, CGColorSpace, CGColorSpaceModel};
use objc2_event_kit::{
    EKAuthorizationStatus, EKCalendar, EKEntityType, EKEvent, EKEventStore, EKParticipant,
};
use objc2_foundation::{NSDate, NSError};

use crate::calendar::{AccessState, CalendarSource};
use crate::model::{sort_attendees, Event};

/// How long we wait for the user to answer the permission prompt before giving
/// up and reporting `NotDetermined`. The prompt itself is modal to the user,
/// not to us, so this is generous.
const ACCESS_TIMEOUT: StdDuration = StdDuration::from_secs(60);

/// Reads events from macOS EventKit.
pub struct EventKitSource {
    store: Retained<EKEventStore>,
    access: AccessState,
}

// SAFETY: `EKEventStore` is not documented as main-thread-only — Apple's own
// guidance is that one instance be used from one thread at a time, which is
// what `CalendarStore`'s mutex guarantees. Nothing here touches AppKit.
unsafe impl Send for EventKitSource {}

impl EventKitSource {
    /// Create the store and read the current authorization status.
    ///
    /// Does not prompt; call [`Self::request_access`] for that.
    pub fn new() -> Self {
        let store = unsafe { EKEventStore::new() };
        let access = current_access_state();
        Self { store, access }
    }

    /// Prompt for full calendar access and block until the user answers.
    ///
    /// Returns immediately if the status is already decided. Must not be called
    /// from the main thread while the run loop is needed — EventKit delivers
    /// the completion on its own queue, so blocking the caller is safe, but a
    /// blocked main thread means a frozen UI for the duration of the prompt.
    /// `CalendarStore::spawn_refresh` runs this on a worker thread.
    pub fn request_access(&mut self) -> AccessState {
        if self.access != AccessState::NotDetermined {
            return self.access;
        }

        let (tx, rx) = mpsc::channel::<bool>();
        let handler = RcBlock::new(move |granted: Bool, _err: *mut NSError| {
            // The receiver may already have timed out; ignore the send error.
            let _ = tx.send(granted.as_bool());
        });

        autoreleasepool(|_| {
            let modern = self
                .store
                .respondsToSelector(sel!(requestFullAccessToEventsWithCompletion:));
            unsafe {
                if modern {
                    self.store
                        .requestFullAccessToEventsWithCompletion(RcBlock::as_ptr(&handler));
                } else {
                    #[allow(deprecated)]
                    self.store.requestAccessToEntityType_completion(
                        EKEntityType::Event,
                        RcBlock::as_ptr(&handler),
                    );
                }
            }
        });

        self.access = match rx.recv_timeout(ACCESS_TIMEOUT) {
            // Trust the status over the boolean: on a write-only grant the
            // handler still reports success for the old selector.
            Ok(_) => current_access_state(),
            Err(_) => AccessState::NotDetermined,
        };
        self.access
    }

    pub fn access(&self) -> AccessState {
        self.access
    }

    /// Re-read the system authorization status (it can change while we run).
    pub fn refresh_access(&mut self) -> AccessState {
        self.access = current_access_state();
        self.access
    }
}

impl Default for EventKitSource {
    fn default() -> Self {
        Self::new()
    }
}

impl CalendarSource for EventKitSource {
    fn events_between(&self, from: NaiveDate, to: NaiveDate) -> Vec<Event> {
        if !self.access.can_read() || to < from {
            return Vec::new();
        }

        // EventKit's predicate is a half-open [start, end) instant range, so
        // ask for midnight on `from` through midnight after `to`.
        let Some(start) = local_midnight(from) else {
            return Vec::new();
        };
        let Some(end) = local_midnight(to + Duration::days(1)) else {
            return Vec::new();
        };

        // EventKit hands back autoreleased objects (the predicate, the event
        // array, and every string/date read off an event). This runs on a
        // long-lived background-executor thread with no pool of its own, so
        // drain one per fetch or the whole event set leaks every 5 minutes.
        autoreleasepool(|_| unsafe {
            let predicate = self
                .store
                .predicateForEventsWithStartDate_endDate_calendars(&start, &end, None);
            let matched = self.store.eventsMatchingPredicate(&predicate);
            matched.iter().filter_map(|ek| map_event(&ek)).collect()
        })
    }

    fn name(&self) -> &'static str {
        "eventkit"
    }

    fn access_state(&self) -> AccessState {
        self.access
    }

    fn ensure_access(&mut self) -> AccessState {
        EventKitSource::request_access(self)
    }

    fn refresh_access(&mut self) -> AccessState {
        EventKitSource::refresh_access(self)
    }
}

/// The current system authorization status for calendar events.
fn current_access_state() -> AccessState {
    let status = unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) };
    match status {
        EKAuthorizationStatus::FullAccess => AccessState::Granted,
        EKAuthorizationStatus::Denied => AccessState::Denied,
        EKAuthorizationStatus::Restricted => AccessState::Restricted,
        EKAuthorizationStatus::WriteOnly => AccessState::WriteOnly,
        _ => AccessState::NotDetermined,
    }
}

/// Local midnight at the start of `date`, as an `NSDate`.
///
/// Falls back an hour at a time across a spring-forward gap, where local
/// midnight does not exist (Brazil used to do this).
fn local_midnight(date: NaiveDate) -> Option<Retained<NSDate>> {
    let mut naive = date.and_hms_opt(0, 0, 0)?;
    for _ in 0..4 {
        if let Some(dt) = Local.from_local_datetime(&naive).earliest() {
            let secs = dt.timestamp() as f64 + f64::from(dt.timestamp_subsec_nanos()) / 1e9;
            return Some(NSDate::dateWithTimeIntervalSince1970(secs));
        }
        naive += Duration::hours(1);
    }
    None
}

/// `NSDate` -> local wall-clock time.
fn nsdate_to_local(date: &NSDate) -> NaiveDateTime {
    let secs = date.timeIntervalSince1970();
    let whole = secs.floor();
    let nanos = ((secs - whole) * 1e9).round() as u32;
    Local
        .timestamp_opt(whole as i64, nanos.min(999_999_999))
        .earliest()
        .map(|dt| dt.naive_local())
        // An out-of-range NSDate (distantPast/distantFuture) clamps rather than
        // dropping the event on the floor.
        .unwrap_or_else(|| Local::now().naive_local())
}

/// Map one `EKEvent` onto the pure model.
///
/// # Safety
/// Caller must hold a live reference to the event's store.
unsafe fn map_event(ek: &EKEvent) -> Option<Event> {
    let id = unsafe { ek.eventIdentifier() }
        .map(|s| s.to_string())
        .unwrap_or_default();
    let title = unsafe { ek.title() }.to_string();
    let location = unsafe { ek.location() }
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty());
    let all_day = unsafe { ek.isAllDay() };
    let start = nsdate_to_local(&unsafe { ek.startDate() }.clone());
    let end = nsdate_to_local(&unsafe { ek.endDate() }.clone());
    let (start, end) = normalize_span(start, end, all_day);

    // Events with no identifier still render; synthesize one so keys are stable
    // within a fetch.
    let id = if id.is_empty() {
        format!("ek-{}-{}", start.and_utc().timestamp(), title)
    } else {
        id
    };

    let notes = unsafe { ek.notes() }
        .map(|s| s.to_string())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let url = unsafe { ek.URL() }
        .and_then(|u| u.absoluteString())
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty());
    let calendar_color = unsafe { ek.calendar() }
        .as_deref()
        .and_then(|cal| unsafe { calendar_rgb(cal) });
    let attendees = unsafe { read_attendees(ek) };

    Some(Event {
        id,
        title: if title.is_empty() {
            "(no title)".to_string()
        } else {
            title
        },
        location,
        start,
        end,
        all_day,
        calendar_color,
        notes,
        url,
        attendees,
    })
}

/// Display names of an event's attendees, the current user as `"you"`, sorted
/// with `"you"` last. Empty when the event has no attendees (EventKit reports
/// `nil` for a solo event).
///
/// # Safety
/// Caller must hold a live reference to the event's store.
unsafe fn read_attendees(ek: &EKEvent) -> Vec<String> {
    let Some(list) = (unsafe { ek.attendees() }) else {
        return Vec::new();
    };
    let mut names: Vec<String> = list
        .iter()
        .filter_map(|p: Retained<EKParticipant>| unsafe { participant_name(&p) })
        .collect();
    names.dedup();
    sort_attendees(&mut names);
    names
}

/// One participant's display name, or `None` when it has neither a name nor a
/// usable address.
///
/// # Safety
/// Caller must hold a live reference to the participant's event.
unsafe fn participant_name(p: &EKParticipant) -> Option<String> {
    if unsafe { p.isCurrentUser() } {
        return Some("you".to_string());
    }
    let name = unsafe { p.name() }
        .map(|s| s.to_string())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if name.is_some() {
        return name;
    }
    // No display name: fall back to the local part of the mailto: URL.
    //
    // objc2-event-kit declares `URL` nonnull, so its generated binding panics
    // on a nil one — and EventKit does hand back participants without a URL
    // (Exchange/ICS invitees, room resources). Ask for it as optional instead;
    // this runs on the refresh task's thread, where a panic would kill the
    // whole refresh.
    let url: Option<objc2::rc::Retained<objc2_foundation::NSURL>> =
        unsafe { objc2::msg_send![p, URL] };
    let url = url?;
    let text = url.absoluteString()?.to_string();
    let address = text.strip_prefix("mailto:").unwrap_or(&text);
    let local = address.split('@').next().unwrap_or(address).trim();
    (!local.is_empty()).then(|| local.to_string())
}

/// A calendar's colour as sRGB bytes.
///
/// `CGColor` can come back nil, and its colour space can be monochrome (grey
/// calendars) or something we do not understand (CMYK, pattern). Only RGB-ish
/// and monochrome spaces are converted; anything else is `None` so the UI can
/// fall back to its own accent.
///
/// # Safety
/// Caller must hold a live reference to the calendar's store.
unsafe fn calendar_rgb(cal: &EKCalendar) -> Option<(u8, u8, u8)> {
    let color = unsafe { cal.CGColor() }?;
    let components = CGColor::components(Some(&color));
    if components.is_null() {
        return None;
    }
    let count = CGColor::number_of_components(Some(&color));
    if count == 0 {
        return None;
    }
    let space = CGColor::color_space(Some(&color))?;
    let model = CGColorSpace::model(Some(&space));
    // SAFETY: `components` points at `count` floats for as long as `color` is
    // alive, and `color` is retained by this scope.
    let comps = unsafe { std::slice::from_raw_parts(components, count) };
    let (r, g, b) = match model {
        CGColorSpaceModel::RGB if count >= 3 => (comps[0], comps[1], comps[2]),
        CGColorSpaceModel::Monochrome => (comps[0], comps[0], comps[0]),
        _ => return None,
    };
    Some((to_byte(r), to_byte(g), to_byte(b)))
}

/// A 0..1 colour component as a byte, clamping anything out of range (extended
/// -range colour spaces produce negatives and values above 1).
fn to_byte(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Normalize an event's span into "the days it visibly occupies".
///
/// EventKit is inconsistent about all-day end dates: depending on the account
/// type the end is either `23:59:59` on the last day or midnight at the start
/// of the *following* day. The exclusive-midnight form would make a one-day
/// holiday show up on two days, so pull it back. Also guard against `end`
/// before `start`, which some CalDAV servers produce.
pub(crate) fn normalize_span(
    start: NaiveDateTime,
    end: NaiveDateTime,
    // Kept in the signature because the caller always has it and the
    // distinction may matter again if EventKit's end conventions change.
    _all_day: bool,
) -> (NaiveDateTime, NaiveDateTime) {
    if end < start {
        return (start, start);
    }
    // An end at exactly midnight is exclusive whether or not the event is
    // all-day: a 22:00–00:00 meeting belongs to the day it started on, not to
    // the next morning's grid cell.
    if end.time() == chrono::NaiveTime::MIN && end.date() > start.date() {
        return (start, end - Duration::seconds(1));
    }
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, min, 0)
            .unwrap()
    }

    #[test]
    fn exclusive_all_day_end_is_pulled_back_into_the_last_day() {
        let (s, e) = normalize_span(dt(2026, 9, 11, 0, 0), dt(2026, 9, 12, 0, 0), true);
        assert_eq!(s.date(), NaiveDate::from_ymd_opt(2026, 9, 11).unwrap());
        assert_eq!(e.date(), NaiveDate::from_ymd_opt(2026, 9, 11).unwrap());
    }

    #[test]
    fn inclusive_all_day_end_is_left_alone() {
        let end = dt(2026, 9, 11, 23, 59);
        let (_, e) = normalize_span(dt(2026, 9, 11, 0, 0), end, true);
        assert_eq!(e, end);
    }

    #[test]
    fn multi_day_all_day_keeps_its_interior_days() {
        let (s, e) = normalize_span(dt(2026, 9, 11, 0, 0), dt(2026, 9, 14, 0, 0), true);
        assert_eq!(s.date(), NaiveDate::from_ymd_opt(2026, 9, 11).unwrap());
        assert_eq!(e.date(), NaiveDate::from_ymd_opt(2026, 9, 13).unwrap());
    }

    #[test]
    fn timed_event_ending_at_midnight_stays_on_its_start_day() {
        let (s, e) = normalize_span(dt(2026, 9, 11, 22, 0), dt(2026, 9, 12, 0, 0), false);
        assert_eq!(s.date(), NaiveDate::from_ymd_opt(2026, 9, 11).unwrap());
        assert_eq!(e.date(), NaiveDate::from_ymd_opt(2026, 9, 11).unwrap());
        assert_eq!(e, dt(2026, 9, 11, 23, 59) + Duration::seconds(59));
    }

    #[test]
    fn timed_events_are_untouched() {
        let (s, e) = normalize_span(dt(2026, 9, 11, 9, 0), dt(2026, 9, 11, 10, 0), false);
        assert_eq!((s, e), (dt(2026, 9, 11, 9, 0), dt(2026, 9, 11, 10, 0)));
    }

    #[test]
    fn backwards_span_collapses_to_a_point() {
        let (s, e) = normalize_span(dt(2026, 9, 11, 9, 0), dt(2026, 9, 10, 9, 0), false);
        assert_eq!(s, e);
    }

    #[test]
    fn local_midnight_exists_for_every_day_of_a_year() {
        let mut day = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        while chrono::Datelike::year(&day) == 2026 {
            assert!(local_midnight(day).is_some(), "no midnight for {day}");
            day += Duration::days(1);
        }
    }

    #[test]
    fn color_components_clamp_into_bytes() {
        assert_eq!(to_byte(0.0), 0);
        assert_eq!(to_byte(1.0), 255);
        assert_eq!(to_byte(0.5), 128);
        assert_eq!(to_byte(-0.3), 0);
        assert_eq!(to_byte(1.4), 255);
        assert_eq!(to_byte(f64::NAN), 0);
    }

    #[test]
    fn authorization_status_reads_without_prompting() {
        // Never panics and never prompts; the value depends on the machine.
        let _ = current_access_state();
    }
}
