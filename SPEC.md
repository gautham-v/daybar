# daybar — spec

A macOS menu bar calendar. Click the date in the menu bar, a popover drops down with a month grid
and the selected day's events. Built with GPUI (Rust). Never opens another app unless asked.

## Reference
- `docs/reference-itsycal.png` — the user's current menu bar calendar (Itsycal-like). Match its
  structure: prev/next arrows, "September 2026" title, week-number column on the left, Sunday-start
  grid, today as a filled circle, out-of-month days dimmed.
- `docs/mockup-popover.dc.html` — the approved mockup (HTML/CSS). Match its visual tokens:
  background `#fbfaf8`, text `#1a1a1a`, muted `#8a8781`, faint `#a3a099`, rules `#ebe8e2`, selected
  day filled `#1a1a1a` with `#fbfaf8` text, today/accent `#c2410c`, footer band `#f6f4f0`.
  Font: system (SF Pro), times in monospace (SF Mono). Popover 320px wide, 12px radius.

## Behavior
- Menu bar item shows just the day number ("11") in the menu bar font. Click toggles the popover.
  Clicking outside or pressing Esc closes it. No Dock icon (accessory activation policy).
- Popover: header (‹ › arrows, month title, Day/Week toggle), weekday row, 6-row month grid
  (Sunday start, ISO week numbers on the left), a thin rule, then the list, then a footer with
  key hints and a "Today" button.
- Click a day → the list swaps to that day, popover stays open. Today's past events dimmed to 40%;
  the next upcoming event's time is drawn in the accent color; "N left" summary for today,
  "N events" otherwise; "nothing scheduled" when empty.
- Day/Week toggle: Week lists the seven days (Sun–Sat) containing the selected day, each row
  showing DOW + day number and its events (or "–").
- Keys: ← → move a day, ↑ ↓ move a week, Enter opens the selected day in Google Calendar in the
  browser (`https://calendar.google.com/calendar/r/day/YYYY/M/D`), `t` jumps to today, Esc closes.
- Data source: macOS EventKit (Calendar.app), which is where Google Calendar is already synced.
  Request full calendar access on first launch (needs an .app bundle with
  `NSCalendarsFullAccessUsageDescription`). Refresh every 5 minutes and whenever the popover opens.
  Window of interest: 60 days back to 90 days ahead. All-day events listed first without a time.
- Data model is behind a `CalendarSource` trait so a Google API source can be added later.

## Crates
gpui 0.2, objc2 0.6 family (objc2-foundation, objc2-app-kit, objc2-event-kit), chrono.

## Repo layout
- `src/main.rs` — app entry, activation policy, status item, popover window management
- `src/model.rs` — `Event`, `Day`, date helpers (pure, unit-tested)
- `src/calendar/` — `CalendarSource` trait, `eventkit.rs` impl, `stub.rs` for tests/dev
- `src/ui/` — `popover.rs` root view, `month_grid.rs`, `day_list.rs`, `week_list.rs`, `theme.rs`
- `scripts/bundle.sh` — builds `target/release/daybar` into `target/Daybar.app` with Info.plist
- `Makefile` — `make run` (bundle + open), `make test`, `make check`
