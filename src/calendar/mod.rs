//! Calendar data sources.
//!
//! Everything the UI needs sits behind [`CalendarSource`] so EventKit can be
//! swapped for a Google API client (or the dev stub) without touching views.
//! Nothing in this module knows about gpui.

use chrono::NaiveDate;

use crate::model::Event;

pub mod eventkit;
pub mod store;
pub mod stub;

#[allow(unused_imports)]
pub use store::CalendarStore;

/// Whether the app may read the user's calendars.
///
/// Surfaced by [`CalendarSource::access_state`] so the popover can render an
/// explanation instead of an empty list when access was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccessState {
    /// Never asked (or the request has not come back yet).
    #[default]
    NotDetermined,
    /// Full read access granted.
    Granted,
    /// The user said no. Sources return an empty `Vec`.
    Denied,
    /// Blocked by policy (MDM, parental controls). Also returns empty.
    Restricted,
    /// Write-only access: enough to add events, not to read them.
    WriteOnly,
}

impl AccessState {
    /// Can we actually read events?
    pub fn can_read(self) -> bool {
        matches!(self, AccessState::Granted)
    }

    /// A short line the popover can show in place of the event list.
    pub fn message(self) -> Option<&'static str> {
        match self {
            AccessState::Granted => None,
            AccessState::NotDetermined => Some("waiting for calendar access"),
            AccessState::Denied | AccessState::WriteOnly => {
                Some("calendar access denied — enable it in System Settings › Privacy & Security › Calendars")
            }
            AccessState::Restricted => Some("calendar access is restricted on this Mac"),
        }
    }
}

/// A source of calendar events.
///
/// Implementations are expected to be cheap to call — the caller refreshes on
/// a timer and on popover open, so any network or IPC work should be cached
/// behind the implementation rather than done per call.
pub trait CalendarSource {
    /// All events overlapping the inclusive date range `from..=to`,
    /// in no particular order (the caller groups and sorts).
    ///
    /// Returns an empty `Vec` rather than failing when access is missing.
    fn events_between(&self, from: NaiveDate, to: NaiveDate) -> Vec<Event>;

    /// Human-readable name, for diagnostics.
    fn name(&self) -> &'static str {
        "calendar"
    }

    /// Current permission state. Sources that need no permission are granted.
    fn access_state(&self) -> AccessState {
        AccessState::Granted
    }

    /// Prompt for access if it has not been decided yet, blocking until the
    /// user answers. Sources that need no permission do nothing.
    ///
    /// Must be called off the main thread: the prompt is answered on another
    /// queue and this blocks meanwhile.
    fn ensure_access(&mut self) -> AccessState {
        self.access_state()
    }
}
