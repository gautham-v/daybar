//! The single-day event list, per `docs/mockup-v2-inline-expand.dc.html`.
//!
//! Each event is a row: a 3px calendar-colored bar, the title, a `HH:MM – HH:MM`
//! line, and a disclosure chevron. Clicking a row (or pressing Enter/Space on a
//! keyboard-focused one) expands it in place with location, attendees, notes and
//! the Join / Open in Google Calendar buttons. Only one row is open at a time.
//! Past events dim; the next upcoming event today gets a "· in 20 min" tail.

use chrono::NaiveDateTime;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Rgba,
    StatefulInteractiveElement, Styled,
};

use crate::calendar::AccessState;
use crate::model::{self, Event};

use crate::ui::popover::{Popover, LIST_PAD_BOTTOM, LIST_PAD_TOP};
use crate::ui::theme::{self, Theme};

// ── Row metrics, kept next to the layout they describe ───────────────────────

const ROW_PAD_Y: f32 = 7.0;
const TITLE_LINE: f32 = 17.0;
const TIME_LINE: f32 = 14.0;
const TITLE_TIME_GAP: f32 = 1.0;
/// A collapsed row, padding included.
pub(crate) const ROW_HEIGHT: f32 = ROW_PAD_Y * 2.0 + TITLE_LINE + TITLE_TIME_GAP + TIME_LINE;

const DETAIL_PAD_TOP: f32 = 2.0;
const DETAIL_PAD_BOTTOM: f32 = 10.0;
const DETAIL_GAP: f32 = 8.0;
const DETAIL_LINE: f32 = 17.0;
const NOTES_LINE: f32 = 17.0;
const BUTTON_ROW: f32 = 24.0 + 2.0;
/// Notes are clamped to this many lines.
const NOTES_MAX_LINES: usize = 6;
/// Roughly how many characters of `TEXT_SMALL` fit across the detail column.
const NOTES_CHARS_PER_LINE: usize = 40;

const EMPTY_LINE: f32 = 16.0;
const EMPTY_BLOCK: f32 = 24.0 + EMPTY_LINE + 24.0;
const EMPTY_CHARS_PER_LINE: usize = 44;

/// The line shown when a day has no events — either "nothing scheduled" or the
/// reason we cannot see the calendar at all.
pub fn empty_notice(access: AccessState) -> &'static str {
    access.message().unwrap_or("nothing scheduled")
}

/// How many lines the notes block will take once clamped.
fn notes_lines(notes: &str) -> usize {
    notes
        .lines()
        .map(|line| line.len().div_ceil(NOTES_CHARS_PER_LINE).max(1))
        .sum::<usize>()
        .clamp(1, NOTES_MAX_LINES)
}

/// The notes text actually drawn: clamped to [`NOTES_MAX_LINES`] lines.
fn clamped_notes(notes: &str) -> String {
    notes
        .lines()
        .take(NOTES_MAX_LINES)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extra height an expanded row adds under its collapsed self.
pub(crate) fn detail_height(event: &Event) -> f32 {
    let mut blocks: Vec<f32> = Vec::new();
    if event.location.is_some() {
        blocks.push(DETAIL_LINE);
    }
    if event.attendee_line().is_some() {
        blocks.push(DETAIL_LINE);
    }
    if let Some(notes) = event.notes.as_deref() {
        blocks.push(notes_lines(notes) as f32 * NOTES_LINE);
    }
    blocks.push(BUTTON_ROW);

    let gaps = (blocks.len().saturating_sub(1)) as f32 * DETAIL_GAP;
    DETAIL_PAD_TOP + DETAIL_PAD_BOTTOM + gaps + blocks.iter().sum::<f32>()
}

/// How tall the day list's content is, so the window can size to it.
///
/// `expanded` is the id of the open row, if any — its details are part of the
/// height, which is what lets the window grow and shrink with the disclosure.
pub fn content_height(events: &[Event], access: AccessState, expanded: Option<&str>) -> f32 {
    if events.is_empty() {
        let lines = empty_notice(access).len().div_ceil(EMPTY_CHARS_PER_LINE);
        return EMPTY_BLOCK + (lines.saturating_sub(1) as f32) * EMPTY_LINE;
    }
    events
        .iter()
        .map(|e| {
            ROW_HEIGHT
                + if expanded == Some(e.id.as_str()) {
                    detail_height(e)
                } else {
                    0.0
                }
        })
        .sum()
}

/// Index of the next event that has not finished yet, when `date` is today.
fn next_up(events: &[Event], now: NaiveDateTime, is_today: bool) -> Option<usize> {
    if !is_today {
        return None;
    }
    events
        .iter()
        .position(|e| !e.all_day && !model::is_past(e.end, now))
}

/// `"in 20 min"` / `"in 4 h"` / `"now"` for an event starting at `start`.
pub(crate) fn starts_in(start: NaiveDateTime, now: NaiveDateTime) -> Option<String> {
    let minutes = (start - now).num_minutes();
    if minutes < 0 {
        Some("now".to_string())
    } else if minutes < 60 {
        Some(format!("in {minutes} min"))
    } else if minutes < 60 * 12 {
        Some(format!("in {} h", minutes / 60))
    } else {
        None
    }
}

/// The second line of a row: `"all-day"`, or `"11:30 – 12:00"` with the
/// next-up tail appended.
pub(crate) fn time_line(event: &Event, now: NaiveDateTime, is_next: bool) -> String {
    if event.all_day {
        return "all-day".to_string();
    }
    let span = format!(
        "{} – {}",
        model::short_time(event.start.time()),
        model::short_time(event.end.time())
    );
    match is_next.then(|| starts_in(event.start, now)).flatten() {
        Some(tail) => format!("{span} · {tail}"),
        None => span,
    }
}

/// The event's calendar color, or the neutral fallback.
fn bar_color(event: &Event, theme: &Theme) -> Rgba {
    match event.calendar_color {
        Some((r, g, b)) => Rgba {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        },
        None => theme.event_bar,
    }
}

pub fn render(state: &Popover, cx: &mut Context<Popover>) -> impl IntoElement {
    let theme = state.theme();
    let date = state.selected();
    let now = state.now();
    let is_today = date == now.date();
    let events = state.events_on(date);
    let notice = empty_notice(state.access());
    let expanded = state.expanded().map(str::to_string);
    let focused = state.focused_row();
    let next = next_up(&events, now, is_today);

    div()
        .flex()
        .flex_col()
        .px(theme::LIST_PAD_X)
        .pt(px(LIST_PAD_TOP))
        .pb(px(LIST_PAD_BOTTOM))
        .when(events.is_empty(), |el| {
            el.child(
                div()
                    .py(px(24.))
                    .w_full()
                    .text_center()
                    .text_size(theme::TEXT_SMALL)
                    .text_color(theme.tertiary)
                    .child(notice),
            )
        })
        .children(events.into_iter().enumerate().map(move |(i, event)| {
            let past = is_today && !event.all_day && model::is_past(event.end, now);
            let is_open = expanded.as_deref() == Some(event.id.as_str());
            let is_focused = focused == Some(i);
            let id = event.id.clone();

            div()
                .id(gpui::ElementId::Name(format!("event-{}", event.id).into()))
                .flex()
                .flex_col()
                .rounded(theme::ROW_RADIUS)
                .cursor_pointer()
                .when(is_open, |el| el.bg(theme.expanded_bg))
                .when(!is_open && is_focused, |el| el.bg(theme.hover))
                .when(!is_open && !is_focused, |el| {
                    el.hover(|s| s.bg(theme.hover))
                })
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_row(i, &id, cx)))
                .child(row_header(
                    &event,
                    &theme,
                    now,
                    next == Some(i),
                    past,
                    is_open,
                ))
                .when(is_open, |el| el.child(details(&event, &theme, cx)))
        }))
}

/// The always-visible half of an event row.
fn row_header(
    event: &Event,
    theme: &Theme,
    now: NaiveDateTime,
    is_next: bool,
    past: bool,
    is_open: bool,
) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.))
        .px(px(8.))
        .py(px(ROW_PAD_Y))
        .when(past, |el| el.opacity(theme::PAST_OPACITY))
        .child(
            div()
                .w(theme::BAR_WIDTH)
                .h(theme::BAR_HEIGHT)
                .flex_shrink_0()
                .rounded(px(2.))
                .bg(bar_color(event, theme)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_grow()
                .min_w_0()
                .gap(px(TITLE_TIME_GAP))
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_size(theme::TEXT_BODY)
                        .text_color(theme.text)
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(event.title.clone()),
                )
                .child(
                    div()
                        .text_size(theme::TEXT_TINY)
                        .text_color(if is_next {
                            theme.accent
                        } else {
                            theme.secondary
                        })
                        .child(time_line(event, now, is_next)),
                ),
        )
        .child(
            div()
                .flex_shrink_0()
                .w(px(10.))
                .text_size(theme::TEXT_TINY)
                .text_color(theme.chevron)
                // gpui has no rotation transform for a text element, so the
                // "rotated" chevron is spelled with the glyph pointing down.
                .child(if is_open { "⌄" } else { "›" }),
        )
}

/// The disclosed half: location, attendees, notes, buttons.
fn details(event: &Event, theme: &Theme, cx: &mut Context<Popover>) -> impl IntoElement {
    let detail_row = |icon: &'static str, text: String| {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .text_size(theme::TEXT_SMALL)
            .text_color(theme.text)
            .child(
                div()
                    .w(px(13.))
                    .flex_shrink_0()
                    .text_color(theme.secondary)
                    .child(icon),
            )
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(text),
            )
    };

    let join = event.join_url();
    let day_url = model::google_calendar_day_url(event.start.date());

    div()
        .flex()
        .flex_col()
        .gap(px(DETAIL_GAP))
        .pt(px(DETAIL_PAD_TOP))
        .pb(px(DETAIL_PAD_BOTTOM))
        .pl(px(21.))
        .pr(px(8.))
        .children(event.location.clone().map(|l| detail_row("◎", l)))
        .children(event.attendee_line().map(|a| detail_row("⚇", a)))
        .children(event.notes.as_deref().map(|notes| {
            div()
                .text_size(theme::TEXT_SMALL)
                .line_height(px(NOTES_LINE))
                .text_color(theme.secondary)
                .child(clamped_notes(notes))
        }))
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(6.))
                .pt(px(2.))
                .children(join.map(|url| {
                    div()
                        .id("join")
                        .px(px(10.))
                        .py(px(4.))
                        .rounded(px(6.))
                        .bg(theme.accent)
                        .text_size(theme::TEXT_SMALL)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.on_accent)
                        .cursor_pointer()
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.stop_propagation();
                            cx.open_url(&url);
                        }))
                        .child("Join")
                }))
                .child(
                    div()
                        .id("open-gcal")
                        .px(px(10.))
                        .py(px(4.))
                        .rounded(px(6.))
                        .bg(theme.button_bg)
                        .text_size(theme::TEXT_SMALL)
                        .text_color(theme.text)
                        .cursor_pointer()
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.stop_propagation();
                            cx.open_url(&day_url);
                        }))
                        .child("Open in Google Calendar"),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn d() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, 11).unwrap()
    }

    fn ev(h: u32, all_day: bool) -> Event {
        Event {
            id: format!("e{h}"),
            title: "x".into(),
            start: d().and_hms_opt(h, 0, 0).unwrap(),
            end: d().and_hms_opt(h + 1, 0, 0).unwrap(),
            all_day,
            ..Event::default()
        }
    }

    fn now() -> NaiveDateTime {
        d().and_hms_opt(12, 40, 0).unwrap()
    }

    #[test]
    fn empty_day_still_has_height() {
        assert!(content_height(&[], AccessState::Granted, None) > 0.0);
    }

    #[test]
    fn more_events_are_taller() {
        assert!(
            content_height(&[ev(9, false), ev(11, false)], AccessState::Granted, None)
                > content_height(&[ev(9, false)], AccessState::Granted, None)
        );
    }

    #[test]
    fn an_expanded_row_grows_the_list() {
        let events = [ev(9, false), ev(11, false)];
        let collapsed = content_height(&events, AccessState::Granted, None);
        let open = content_height(&events, AccessState::Granted, Some("e9"));
        assert!(open > collapsed);
        // Only the named row expands.
        assert_eq!(open - collapsed, detail_height(&events[0]));
    }

    #[test]
    fn an_unknown_expanded_id_changes_nothing() {
        let events = [ev(9, false)];
        assert_eq!(
            content_height(&events, AccessState::Granted, Some("nope")),
            content_height(&events, AccessState::Granted, None)
        );
    }

    #[test]
    fn details_grow_with_the_content() {
        let bare = ev(9, false);
        let mut rich = ev(9, false);
        rich.location = Some("Zoom".into());
        rich.attendees = vec!["Priya".into(), "you".into()];
        rich.notes = Some("a\nb\nc".into());
        assert!(detail_height(&rich) > detail_height(&bare));
    }

    #[test]
    fn a_denied_day_explains_itself_and_gets_more_room() {
        assert_eq!(empty_notice(AccessState::Granted), "nothing scheduled");
        assert!(empty_notice(AccessState::Denied).contains("System Settings"));
        assert!(
            content_height(&[], AccessState::Denied, None)
                > content_height(&[], AccessState::Granted, None)
        );
    }

    #[test]
    fn notes_are_clamped_to_six_lines() {
        let many = (1..=12)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(notes_lines(&many), NOTES_MAX_LINES);
        assert_eq!(clamped_notes(&many).lines().count(), NOTES_MAX_LINES);
        assert_eq!(notes_lines("one line"), 1);
    }

    #[test]
    fn next_up_skips_finished_events() {
        let events = vec![ev(9, false), ev(11, false), ev(15, false), ev(17, false)];
        assert_eq!(next_up(&events, now(), true), Some(2));
    }

    #[test]
    fn next_up_is_none_on_other_days() {
        assert_eq!(next_up(&[ev(15, false)], now(), false), None);
    }

    #[test]
    fn next_up_ignores_all_day_events() {
        let early = d().and_hms_opt(8, 0, 0).unwrap();
        assert_eq!(next_up(&[ev(0, true), ev(9, false)], early, true), Some(1));
    }

    #[test]
    fn the_time_line_spans_start_to_end() {
        let e = ev(11, false);
        assert_eq!(time_line(&e, now(), false), "11:00 – 12:00");
        assert_eq!(time_line(&ev(0, true), now(), false), "all-day");
    }

    #[test]
    fn the_next_event_gets_a_countdown_tail() {
        let e = ev(13, false);
        assert_eq!(time_line(&e, now(), true), "1:00 – 2:00 · in 20 min");
    }

    #[test]
    fn countdowns_read_in_minutes_then_hours() {
        assert_eq!(
            starts_in(d().and_hms_opt(13, 0, 0).unwrap(), now()).as_deref(),
            Some("in 20 min")
        );
        assert_eq!(
            starts_in(d().and_hms_opt(16, 40, 0).unwrap(), now()).as_deref(),
            Some("in 4 h")
        );
        assert_eq!(
            starts_in(d().and_hms_opt(12, 30, 0).unwrap(), now()).as_deref(),
            Some("now")
        );
        // Beyond half a day the countdown stops meaning anything.
        let tomorrow = d().succ_opt().unwrap().and_hms_opt(2, 0, 0).unwrap();
        assert_eq!(starts_in(tomorrow, now()), None);
    }
}
