//! The 6x7 Sunday-start month grid with the ISO week-number gutter.
//!
//! Renders [`crate::model::month_grid`] output: out-of-month days dimmed,
//! weekends a shade lighter, today a filled system-blue circle, the selected
//! non-today day a neutral filled circle, and a 3px dot under any day that has
//! events.

use chrono::Datelike;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled,
};

use crate::model;
use crate::ui::popover::Popover;
use crate::ui::theme;

pub fn render(state: &Popover, cx: &mut Context<Popover>) -> impl IntoElement {
    let theme = state.theme();
    let today = state.today();
    let selected = state.selected();
    let grid = model::month_grid(state.visible_month());

    div()
        .flex()
        .flex_col()
        .px(theme::GRID_PAD_X)
        .pb(px(8.))
        .children(grid.into_iter().map(|row| {
            div()
                .flex()
                .flex_row()
                .child(
                    div()
                        .w(theme::WEEK_COLUMN_WIDTH)
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .h(theme::CELL_HEIGHT)
                        .text_size(theme::TEXT_MICRO)
                        .text_color(theme.tertiary)
                        .child(row.week_number.to_string()),
                )
                .children(row.days.into_iter().map(|cell| {
                    let date = cell.date;
                    let is_selected = date == selected;
                    let is_today = date == today;
                    let has_events = state.has_events(date);

                    // Today always wins the circle; a selected other day gets
                    // the neutral fill.
                    let (fill, fg) = if is_today {
                        (Some(theme.accent), theme.on_accent)
                    } else if is_selected {
                        (Some(theme.selected_fill), theme.text)
                    } else if !cell.in_month {
                        (None, theme.out_of_month)
                    } else if model::is_weekend(date) {
                        (None, theme.weekend)
                    } else {
                        (None, theme.text)
                    };

                    let dot = if !has_events {
                        None
                    } else if is_today {
                        Some(theme.on_accent)
                    } else if cell.in_month && !model::is_weekend(date) {
                        Some(theme.dot)
                    } else {
                        Some(theme.dot_dim)
                    };

                    div()
                        .id(gpui::ElementId::Name(format!("day-{date}").into()))
                        .flex()
                        .flex_1()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .h(theme::CELL_HEIGHT)
                        .rounded(px(7.))
                        .cursor_pointer()
                        .when(fill.is_none(), |el| el.hover(|s| s.bg(theme.hover)))
                        .on_click(cx.listener(move |this, _, _, cx| this.pick(date, cx)))
                        .when_some(fill, |el, c| {
                            // A filled day: the number alone, centered in the
                            // circle, so the dot never crowds it.
                            el.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .size(theme::CELL_CIRCLE)
                                    .rounded(theme::CELL_CIRCLE)
                                    .bg(c)
                                    .text_size(theme::TEXT_SMALL)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(fg)
                                    .child(date.day().to_string()),
                            )
                        })
                        .when(fill.is_none(), |el| {
                            el.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(2.))
                                    .text_size(theme::TEXT_SMALL)
                                    .text_color(fg)
                                    .child(div().child(date.day().to_string()))
                                    .child(
                                        div()
                                            .size(theme::DOT_SIZE)
                                            .rounded(px(2.))
                                            .when_some(dot, |el, c| el.bg(c)),
                                    ),
                            )
                        })
                }))
        }))
}
