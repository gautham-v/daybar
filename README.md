# daybar

Your calendar in the macOS menu bar. Rust + [GPUI](https://www.gpui.rs/), sibling of
[claudebar](https://github.com/gautham-v/claudebar).

<img src="docs/screenshot.png" width="480" alt="daybar: the menu bar item and its popover, with one event expanded">

The menu bar shows today's date in a calendar glyph. Click it for a month grid with a dot on
every day that has something on it, and the selected day's events, read from Calendar.app, so
whatever is synced there shows up, Google Calendar included. Click an event to open it in place:
location, who else is on it, the notes, and a **Join** button when there is a Zoom, Meet, Teams
or Webex link. No account, no config, no Dock icon.

## Install

```sh
brew install --cask gautham-v/tap/daybar
```

The build is signed and notarized. On first launch macOS asks for calendar access. Then turn on
**Launch at login** from the `···` menu. The same `Daybar-<version>.zip` is on the
[releases page](https://github.com/gautham-v/daybar/releases) if you would rather skip Homebrew.

From source, with Rust and Xcode installed:

```sh
make install   # builds Daybar.app, copies it to /Applications, launches it
```

`make run` builds and launches from `target/` instead; `cargo run --example popover_preview`
shows the popover over stub data, and `DAYBAR_STUB=1 target/Daybar.app/Contents/MacOS/daybar`
runs the real bundle over it.

## Keys

| Key | |
|---|---|
| `←` `→` | previous / next day |
| `↑` `↓` | previous / next week, or move between events once the list has focus |
| `⇥` | focus the event list |
| `space` `⏎` | expand the focused event; `⏎` on a day opens it in Google Calendar |
| `t` | today |
| `esc` | close the menu, the expanded event, list focus, then the popover |

`‹` `›` page the month without moving the selection. **Week** in the footer swaps the list to the
Sunday to Saturday week around the selection, and **Today** jumps back.

## Where the events come from

EventKit, on your machine, cached 60 days back and 90 days ahead and refetched every 5 minutes
and whenever the popover opens. Nothing leaves the machine and there is no telemetry. Calendar
access is granted per code signature, so `make install` signs with the Developer ID identity on
the machine when there is one; an ad hoc build re-prompts on every rebuild. Grant or revoke
access in **System Settings › Privacy & Security › Calendars**.

## License

MIT.
