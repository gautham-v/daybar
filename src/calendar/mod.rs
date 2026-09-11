//! Calendar data sources.
//!
//! Everything the UI needs sits behind [`CalendarSource`] so EventKit can be
//! swapped for a Google API client (or the dev stub) without touching views.

use chrono::NaiveDate;

use crate::model::Event;

pub mod eventkit;
pub mod stub;

/// A source of calendar events.
///
/// Implementations are expected to be cheap to call — the caller refreshes on
/// a timer and on popover open, so any network or IPC work should be cached
/// behind the implementation rather than done per call.
pub trait CalendarSource {
    /// All events overlapping the inclusive date range `from..=to`,
    /// in no particular order (the UI sorts per day).
    fn events_between(&self, from: NaiveDate, to: NaiveDate) -> Vec<Event>;

    /// Human-readable name, for diagnostics.
    fn name(&self) -> &'static str {
        "calendar"
    }
}
