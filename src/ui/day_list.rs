//! The single-day event list: all-day events first without a time, then timed
//! events. Today's past events dim to 40%, the next upcoming event's time is
//! drawn in the accent color, and an empty day reads "nothing scheduled".

use gpui::prelude::FluentBuilder;
use gpui::{div, px, FontWeight, IntoElement, ParentElement, Styled};

use crate::model::Event;
use crate::ui::format;
use crate::ui::popover::{Popover, LIST_PAD_BOTTOM, LIST_PAD_TOP};
use crate::ui::theme;

/// Per-row heights, kept next to the layout they describe.
const ROW_PAD: f32 = 8.0;
const TITLE_LINE: f32 = 17.0;
const LOCATION_LINE: f32 = 17.0;
const HEADER_ROW: f32 = 4.0 + 17.0 + 8.0;
const EMPTY_BLOCK: f32 = 24.0 + 16.0 + 24.0;

/// How tall the day list's content is, so the window can size to it.
pub fn content_height(events: &[Event]) -> f32 {
    let body = if events.is_empty() {
        EMPTY_BLOCK
    } else {
        events
            .iter()
            .map(|e| {
                ROW_PAD * 2.0
                    + TITLE_LINE
                    + if e.location.is_some() {
                        LOCATION_LINE + 2.0
                    } else {
                        0.0
                    }
                    + 1.0
            })
            .sum()
    };
    HEADER_ROW + body
}

/// Index of the next event that has not finished yet, when `date` is today.
fn next_up(events: &[Event], now: chrono::NaiveDateTime, is_today: bool) -> Option<usize> {
    if !is_today {
        return None;
    }
    events
        .iter()
        .position(|e| !e.all_day && !format::is_past(e.end, now))
}

pub fn render(state: &Popover) -> impl IntoElement {
    let date = state.selected();
    let now = state.now();
    let today = now.date();
    let is_today = date == today;
    let events = state.events_on(date);

    let next = next_up(&events, now, is_today);
    let remaining = events
        .iter()
        .filter(|e| e.all_day || !format::is_past(e.end, now))
        .count();

    let summary = format::day_summary(events.len(), remaining, is_today);

    div()
        .flex()
        .flex_col()
        .px(theme::PAD_X)
        .pt(px(LIST_PAD_TOP))
        .pb(px(LIST_PAD_BOTTOM))
        .min_h(theme::LIST_MIN_HEIGHT)
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_baseline()
                .pt(px(4.))
                .pb(px(8.))
                .child(
                    div()
                        .text_size(theme::TEXT_BODY)
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format::day_title(date, today)),
                )
                .child(
                    div()
                        .text_size(theme::TEXT_SMALL)
                        .text_color(theme::MUTED)
                        .child(summary),
                ),
        )
        .when(events.is_empty(), |el| {
            el.child(
                div()
                    .py(px(24.))
                    .w_full()
                    .text_center()
                    .italic()
                    .text_size(theme::TEXT_SMALL)
                    .text_color(theme::FAINT)
                    .child("nothing scheduled"),
            )
        })
        .children(events.into_iter().enumerate().map(move |(i, event)| {
            let past = is_today && !event.all_day && format::is_past(event.end, now);
            let is_next = next == Some(i);
            let time = if event.all_day {
                "all-day".to_string()
            } else {
                format::short_time(event.start.time())
            };

            div()
                .flex()
                .flex_row()
                .gap(px(12.))
                .py(px(ROW_PAD))
                .border_b_1()
                .border_color(theme::ROW_RULE)
                .when(past, |el| el.opacity(theme::PAST_OPACITY))
                .child(
                    div()
                        .w(theme::TIME_COLUMN_WIDTH)
                        .flex_shrink_0()
                        .pt(px(1.))
                        .font_family(theme::MONO_FAMILY)
                        .text_size(theme::TEXT_SMALL)
                        .text_color(if is_next { theme::ACCENT } else { theme::MUTED })
                        .font_weight(if is_next {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .child(time),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .child(event.title.clone()),
                        )
                        .children(event.location.clone().map(|l| {
                            div()
                                .text_size(theme::TEXT_SMALL)
                                .text_color(theme::MUTED)
                                .child(l)
                        })),
                )
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn ev(h: u32, all_day: bool) -> Event {
        let d = NaiveDate::from_ymd_opt(2026, 9, 11).unwrap();
        Event {
            id: format!("e{h}"),
            title: "x".into(),
            location: None,
            start: d.and_hms_opt(h, 0, 0).unwrap(),
            end: d.and_hms_opt(h + 1, 0, 0).unwrap(),
            all_day,
        }
    }

    #[test]
    fn empty_day_still_has_height() {
        assert!(content_height(&[]) > 0.0);
    }

    #[test]
    fn more_events_are_taller() {
        assert!(content_height(&[ev(9, false), ev(11, false)]) > content_height(&[ev(9, false)]));
    }

    #[test]
    fn next_up_skips_finished_events() {
        let now = NaiveDate::from_ymd_opt(2026, 9, 11)
            .unwrap()
            .and_hms_opt(12, 40, 0)
            .unwrap();
        let events = vec![ev(9, false), ev(11, false), ev(15, false), ev(17, false)];
        assert_eq!(next_up(&events, now, true), Some(2));
    }

    #[test]
    fn next_up_is_none_on_other_days() {
        let now = NaiveDate::from_ymd_opt(2026, 9, 11)
            .unwrap()
            .and_hms_opt(12, 40, 0)
            .unwrap();
        assert_eq!(next_up(&[ev(15, false)], now, false), None);
    }

    #[test]
    fn next_up_ignores_all_day_events() {
        let now = NaiveDate::from_ymd_opt(2026, 9, 11)
            .unwrap()
            .and_hms_opt(8, 0, 0)
            .unwrap();
        let events = vec![ev(0, true), ev(9, false)];
        assert_eq!(next_up(&events, now, true), Some(1));
    }
}
