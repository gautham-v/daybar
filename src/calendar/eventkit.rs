//! EventKit-backed [`CalendarSource`] — reads Calendar.app, where Google
//! Calendar is already synced.
//!
//! TODO(eventkit): implement. Sketch of the work:
//!   1. Hold a single `Retained<EKEventStore>` for the app's lifetime.
//!   2. On first launch call `requestFullAccessToEventsWithCompletion:` and
//!      cache the granted/denied state. Requires an .app bundle carrying
//!      `NSCalendarsFullAccessUsageDescription` (see `scripts/bundle.sh`).
//!   3. `events_between` builds an `NSPredicate` with
//!      `predicateForEventsWithStartDate:endDate:calendars:` (nil = all
//!      calendars) and runs `eventsMatchingPredicate:`.
//!   4. Map each `EKEvent` to [`Event`]: `eventIdentifier` -> id, `title`,
//!      `location` (empty string -> None), `startDate`/`endDate` converted from
//!      `NSDate` to local `NaiveDateTime`, `isAllDay` -> all_day.
//!   5. Cache results; the caller refreshes every 5 minutes and on popover
//!      open. Also subscribe to `EKEventStoreChangedNotification`.
//!
//! Note: EventKit calls must happen on a thread with a run loop; the store is
//! created on the main thread and is not `Send`/`Sync`.

use chrono::NaiveDate;

use crate::calendar::CalendarSource;
use crate::model::Event;

/// Whether the user has granted calendar access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Not asked yet.
    NotDetermined,
    Granted,
    Denied,
}

/// Reads events from macOS EventKit.
///
/// Not `Send`/`Sync`: the underlying `EKEventStore` is main-thread bound.
pub struct EventKitSource {
    access: Access,
    // TODO(eventkit): store: Retained<EKEventStore>,
}

impl EventKitSource {
    /// Create the store. Does not prompt; call [`Self::request_access`].
    pub fn new() -> Self {
        // TODO(eventkit): create the EKEventStore here.
        Self {
            access: Access::NotDetermined,
        }
    }

    /// Prompt for full calendar access, invoking `on_result` when the user answers.
    pub fn request_access(&mut self, _on_result: impl FnOnce(Access) + 'static) {
        // TODO(eventkit): requestFullAccessToEventsWithCompletion:
        unimplemented!("EventKit access request not implemented yet")
    }

    pub fn access(&self) -> Access {
        self.access
    }
}

impl Default for EventKitSource {
    fn default() -> Self {
        Self::new()
    }
}

impl CalendarSource for EventKitSource {
    fn events_between(&self, _from: NaiveDate, _to: NaiveDate) -> Vec<Event> {
        // TODO(eventkit): predicateForEventsWithStartDate:endDate:calendars:
        // then eventsMatchingPredicate: and map EKEvent -> Event.
        Vec::new()
    }

    fn name(&self) -> &'static str {
        "eventkit"
    }
}
