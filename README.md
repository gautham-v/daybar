# daybar

A macOS menu bar calendar, written in Rust with [GPUI](https://www.gpui.rs/).

The menu bar shows a small calendar glyph with today's date inside it — drawn at runtime as a
template image, so it tints itself for light and dark like the system glyphs do, and redraws at
midnight. Click it and a popover drops down
with a month grid (Sunday-start, ISO week numbers, a dot on days that have something on them)
and the selected day's events, read straight from Calendar.app via EventKit — so whatever is
synced there, Google Calendar included, shows up. Click an event and it opens in place, with the
location, who else is on it, the notes, and a Join button for the Zoom/Meet/Teams link. No Dock
icon, no window management, and it never opens another app unless you ask it to.

It follows the system light/dark appearance, and each event wears its own calendar's colour.

![daybar popover, with one event expanded in place](docs/screenshot.png)

<!-- Captured from `cargo run --example popover_preview` (stub data, no real meeting links).
     Crop the window and overwrite docs/screenshot.png. -->

## Build and run

Requires Rust stable and Xcode (GPUI needs the Metal toolchain:
`xcodebuild -downloadComponent MetalToolchain` if the build complains).

```sh
make run      # release build → target/Daybar.app → open it
make bundle   # just build the .app
make install  # copy to /Applications and relaunch from there
make test     # cargo test
make check    # cargo fmt --check && cargo clippy --all-targets -D warnings
```

`make run` kills any running copy first. To see the UI without the menu bar, against fake data:

```sh
cargo run --example popover_preview
cargo run --example dump_events -- --stub   # print the next 7 days
```

### Launch at login

The `···` menu has a **Launch at login** toggle, backed by `SMAppService` (macOS 13+). It shows
a checkmark whenever macOS reports the login item as enabled, so it agrees with
**System Settings › General › Login Items**, where Daybar appears once it is on.

Two things follow from `SMAppService` registering a *path*:

- It only works from the bundled app. Under `cargo run` or the preview example there is no
  bundle, so the toggle is disabled with a note.
- Moving the `.app` afterwards leaves the login item pointing at the old location. Toggle it
  off and on again after a move.

Which is why `make install` is the recommended way to keep it around:

```sh
make install   # → /Applications/Daybar.app, relaunched from there
```

### Code signing

The bundle is signed with the first identity `security find-identity -v -p codesigning`
reports, or with whatever `CODESIGN_IDENTITY` names:

```sh
CODESIGN_IDENTITY="Apple Development: ..." make bundle
```

A stable identity matters. An ad-hoc signature (`-`) gives every rebuild a different code
identity, so macOS treats each build as a brand-new app and re-prompts for calendar access on
every single launch. With no identity on the machine the script falls back to ad-hoc and says
so. Switching from ad-hoc to a real identity prompts once more for the existing item — choose
**Always Allow** that first time and it stops asking.

## Calendar permission

On first launch macOS asks for full calendar access. The prompt only appears for the bundled
`Daybar.app` (it carries `NSCalendarsFullAccessUsageDescription`), which is why `make run`
builds a bundle rather than running the bare binary. Until access is granted the popover simply
shows no events; grant or revoke it later in **System Settings › Privacy & Security › Calendars**.

Grants are attached to the bundle's code signature, so see "Code signing" below if a rebuild
keeps re-prompting.

Events are cached 60 days back and 90 days ahead, refetched every 5 minutes and every time the
popover opens.

## Keys

| Key | |
| --- | --- |
| `←` `→` | previous / next day |
| `↑` `↓` | previous / next week, or move between event rows once the list has focus (past either end of the list the arrows go back to moving a week) |
| `⇥` | put keyboard focus on the event list |
| `space` | expand / collapse the focused event |
| `t` | jump to today |
| `⏎` | expand the focused event, or open the selected day in Google Calendar |
| `esc` | close the `···` menu, then the expanded event, then list focus, then the popover |

`‹` `›` page the month grid without moving the selection. Clicking a day selects it and switches
back to Day mode; the footer's **Week** button swaps the list between one day and the Sun–Sat week
around the selection, and **Today** jumps back.

## Event details

Clicking a row (or Enter/Space on the focused one) expands it in place — one at a time, and the
popover grows and shrinks to fit. It shows the location, the attendees (`Priya, you`), and the
notes clamped to six lines; invite HTML and Google's `~:~:~` banner lines are stripped first.
**Join** appears when a Zoom / Meet / Teams / Webex link is found anywhere in the event's url,
location or notes. An event that merely carries a link (an agenda doc, a ticket) gets a plain
**Open link** instead — `EKEvent.URL` is a general-purpose field, not a meeting. **Open in Google
Calendar** opens the day in the browser.

The `···` button in the header holds **Refresh** (refetch now, without closing), **Launch at
login** and **Quit Daybar**.

## Layout

- `src/main.rs` — activation policy, status item, popover window, refresh schedule
- `src/menu_bar_icon.rs` — the menu bar glyph, drawn with Core Graphics
- `src/model.rs` — `Event`, the grid and date math, formatting helpers (pure, unit-tested)
- `src/calendar/` — `CalendarSource` trait, `eventkit.rs`, `store.rs` cache, `stub.rs` for dev
- `src/ui/` — `popover.rs` root view plus `month_grid`, `day_list`, `week_list`, `theme`
- `assets/icons/` — the mockup's SVGs, compiled in via `src/ui/icons.rs` (a gpui `AssetSource`)
- `scripts/bundle.sh` — builds `Daybar.app`

`cargo run --example measure_heights` checks that what the popover draws is exactly as tall as
`preferred_height()` — the number main.rs sizes the window to, and clips at.
