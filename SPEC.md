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

## v2 — native look and inline event details (approved 2026-09-11)
Reference: `docs/mockup-v2-inline-expand.dc.html` (option A, approved).

### Visual
- Replace the warm paper palette with a native macOS material look: popover background
  `#f0f0f2` light (`#28282a` dark), fully opaque, text `#1d1d1f`, secondary `#6e6e73`,
  tertiary `#aeaeb2`, separators `rgba(0,0,0,0.08)`, today = filled system blue `#0a7aff` circle
  26px with white bold number, selected (non-today) day = `rgba(0,0,0,0.08)` filled circle.
  Follow the system appearance (light/dark) automatically.
- Popover 300px wide, 11px radius, hairline border `rgba(0,0,0,0.12)`.
- Header: ‹ › as small 22px chevron buttons on the left, month title centered, a "···" button on the
  right that opens a small menu (Quit, Refresh, Launch at login placeholder). No Day/Week segmented
  control in the header.
- Grid: week-number column 22px, cells 32px tall, 12px text, 3px event dot under days with events,
  out-of-month days `#c7c7cc`, weekends `#aeaeb2`.
- Event rows: 3px rounded color bar (the event's calendar color from EventKit's `CGColor`), title
  500 weight, second line `HH:MM – HH:MM` in 11px secondary, "· in 20 min" appended for the next
  upcoming event today. Past events at 45% opacity. Chevron at the right, rotates 90° when expanded.
- Footer: left "Fri, Sep 11 · 12:40" (selected date; live clock only when today), right "Today" and
  "Week" text buttons (Week toggles the week list mode as before).
- Menu bar: day number only (already done).

### Inline expand (option A)
- Clicking an event row (or pressing Enter/Space on a keyboard-focused row) expands it in place
  with: location (with pin icon), attendees (with people icon; names from EKParticipant, "you"
  for the current user, collapsed to "Chetan, Priya, you"; skip when none), notes (plain text,
  clamped to ~6 lines), then buttons: "Join" (only if the event has a URL, or a Zoom/Meet/Teams
  link found in location/notes/url; opens it) and "Open in Google Calendar" (existing day URL).
- Only one row expanded at a time; clicking again collapses. Expanded row background
  `rgba(0,0,0,0.045)`. Popover window grows/shrinks to fit.
- Keyboard: ↑/↓ move a focused row within the list when a row is focused (Tab or clicking enters the
  list); Left/Right still move days; Esc collapses an expanded row first, then closes the popover.
- Model: extend `Event` with `calendar_color: Option<Rgba-ish (r,g,b)>`, `notes: Option<String>`,
  `url: Option<String>`, `attendees: Vec<String>`, `is_current_user_attendee` handled in mapping.
