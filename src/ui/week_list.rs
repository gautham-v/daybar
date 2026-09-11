//! The Week mode list: the seven days (Sun–Sat) containing the selected day,
//! each row showing the weekday letters + day number and that day's events,
//! or a "–" when the day is empty.

use chrono::Datelike;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled,
};

use crate::model;
use crate::ui::format;
use crate::ui::popover::{Popover, LIST_PAD_BOTTOM, LIST_PAD_TOP};
use crate::ui::theme;

const ROW_PAD: f32 = 7.0;
const LABEL_BLOCK: f32 = 28.0;
const EVENT_LINE: f32 = 16.0;

/// Height of the week list for a week whose days have these event counts.
pub fn content_height(counts: &[usize; 7]) -> f32 {
    counts
        .iter()
        .map(|&n| {
            let body = if n == 0 {
                EVENT_LINE
            } else {
                n as f32 * EVENT_LINE + (n.saturating_sub(1)) as f32 * 3.0
            };
            ROW_PAD * 2.0 + body.max(LABEL_BLOCK) + 1.0
        })
        .sum()
}

pub fn render(state: &Popover, cx: &mut Context<Popover>) -> impl IntoElement {
    let today = state.today();
    let selected = state.selected();
    let week = model::week_containing(selected);

    div()
        .flex()
        .flex_col()
        .px(theme::PAD_X)
        .pt(px(LIST_PAD_TOP))
        .pb(px(LIST_PAD_BOTTOM))
        .min_h(theme::LIST_MIN_HEIGHT)
        .children(week.into_iter().map(|date| {
            let events = state.events_on(date);
            let is_today = date == today;
            let is_selected = date == selected;
            let label_color = if is_today {
                theme::ACCENT
            } else if is_selected {
                theme::TEXT
            } else {
                theme::MUTED
            };

            div()
                .id(gpui::ElementId::Name(format!("week-{date}").into()))
                .flex()
                .flex_row()
                .gap(px(12.))
                .py(px(ROW_PAD))
                .border_b_1()
                .border_color(theme::ROW_RULE)
                .cursor_pointer()
                .hover(|s| s.bg(theme::HOVER))
                .on_click(cx.listener(move |this, _, _, cx| this.pick(date, cx)))
                .child(
                    div()
                        .w(theme::WEEK_LABEL_WIDTH)
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .pt(px(1.))
                        .text_color(label_color)
                        .child(
                            div()
                                .text_size(theme::TEXT_MICRO)
                                .line_height(px(12.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format::dow_abbrev(date)),
                        )
                        .child(
                            div()
                                .text_size(theme::TEXT_BODY)
                                .line_height(px(16.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(date.day().to_string()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .flex_grow()
                        .line_height(px(16.))
                        .when(events.is_empty(), |el| {
                            el.child(
                                div()
                                    .pt(px(2.))
                                    .text_size(theme::TEXT_SMALL)
                                    .text_color(theme::PLACEHOLDER)
                                    .child("–"),
                            )
                        })
                        .children(events.into_iter().map(|event| {
                            let time = if event.all_day {
                                "all-day".to_string()
                            } else {
                                format::short_time(event.start.time())
                            };
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(8.))
                                .text_size(theme::TEXT_SMALL)
                                .child(
                                    div()
                                        .w(px(38.))
                                        .flex_shrink_0()
                                        .font_family(theme::MONO_FAMILY)
                                        .text_color(theme::MUTED)
                                        .child(time),
                                )
                                .child(
                                    div()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(event.title.clone()),
                                )
                        })),
                )
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_week_uses_the_label_floor() {
        let h = content_height(&[0; 7]);
        assert_eq!(h, 7.0 * (ROW_PAD * 2.0 + LABEL_BLOCK + 1.0));
    }

    #[test]
    fn busy_days_grow_the_week() {
        assert!(content_height(&[4, 4, 4, 4, 4, 4, 4]) > content_height(&[0; 7]));
    }
}
