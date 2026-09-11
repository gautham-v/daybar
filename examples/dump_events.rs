//! Print the next 7 days of calendar events, so EventKit can be verified
//! without launching the UI.
//!
//!     cargo run --example dump_events            # EventKit
//!     cargo run --example dump_events -- --stub  # deterministic fake data
//!
//! TCC caveat: a plain `cargo run` binary is not an .app bundle, so it carries
//! no `NSCalendarsFullAccessUsageDescription` and macOS will usually refuse the
//! prompt outright — expect `access: Denied` and zero events unless the
//! terminal app running it already has Calendar access (System Settings ›
//! Privacy & Security › Calendars › your terminal). The bundled `Daybar.app`
//! is the real test. `--stub` always works.

use chrono::{Duration, Local};

use daybar::calendar::eventkit::EventKitSource;
use daybar::calendar::stub::StubSource;
use daybar::calendar::{CalendarSource, CalendarStore};

fn main() {
    let use_stub = std::env::args().any(|a| a == "--stub");

    let source: Box<dyn CalendarSource + Send> = if use_stub {
        Box::new(StubSource::new())
    } else {
        let mut ek = EventKitSource::new();
        let access = ek.request_access();
        eprintln!("access: {access:?}");
        if let Some(msg) = access.message() {
            eprintln!("note: {msg}");
        }
        Box::new(ek)
    };

    let store = CalendarStore::new(source);
    store.refresh();

    let today = Local::now().date_naive();
    println!(
        "source: {} | window {:?} | {} cached events",
        store.source_name(),
        store.window(),
        store.len()
    );

    for offset in 0..7 {
        let day = today + Duration::days(offset);
        println!("\n{}", day.format("%a %b %-d"));
        let events = store.events_on(day);
        if events.is_empty() {
            println!("  nothing scheduled");
            continue;
        }
        for e in events {
            let when = if e.all_day {
                "all-day".to_string()
            } else {
                e.start.format("%-I:%M %p").to_string()
            };
            let where_ = e.location.as_deref().unwrap_or("");
            println!(
                "  {when:>8}  {}{}",
                e.title,
                if where_.is_empty() {
                    String::new()
                } else {
                    format!("  — {where_}")
                }
            );
        }
    }
}
