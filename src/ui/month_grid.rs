//! The 6x7 Sunday-start month grid with the ISO week-number gutter.
//!
//! Renders [`crate::model::month_grid`] output: out-of-month days dimmed,
//! today accented, the selected day filled, and an event dot under any day
//! that has events.

use chrono::Datelike;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled,
};

use crate::model;
use crate::ui::popover::Popover;
use crate::ui::theme;

/// Today-and-selected is drawn as a filled **accent** square rather than the
/// usual black one, so "today" never disappears behind the selection.
pub fn render(state: &Popover, cx: &mut Context<Popover>) -> impl IntoElement {
    let today = state.today();
    let selected = state.selected();
    let grid = model::month_grid(state.visible_month());

    div()
        .flex()
        .flex_col()
        .gap(theme::CELL_GAP)
        .px(theme::GRID_PAD_X)
        .pb(px(10.))
        .children(grid.into_iter().map(|row| {
            div()
                .flex()
                .flex_row()
                .gap(theme::CELL_GAP)
                .child(
                    div()
                        .w(theme::WEEK_COLUMN_WIDTH)
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .h(theme::CELL_HEIGHT)
                        .text_size(theme::TEXT_MICRO)
                        .text_color(theme::FAINT)
                        .child(row.week_number.to_string()),
                )
                .children(row.days.into_iter().map(|cell| {
                    let date = cell.date;
                    let is_selected = date == selected;
                    let is_today = date == today;
                    let has_events = state.has_events(date);

                    let (bg, fg) = match (is_selected, is_today, cell.in_month) {
                        (true, true, _) => (Some(theme::ACCENT), theme::SELECTED_FG),
                        (true, false, _) => (Some(theme::SELECTED_BG), theme::SELECTED_FG),
                        (false, true, _) => (None, theme::ACCENT),
                        (false, false, true) => (None, theme::TEXT),
                        (false, false, false) => (None, theme::DIM),
                    };
                    let dot = if !has_events {
                        None
                    } else if is_selected {
                        Some(theme::DOT_ON_SELECTED)
                    } else if cell.in_month {
                        Some(theme::DOT)
                    } else {
                        Some(theme::DOT_DIM)
                    };

                    div()
                        .id(gpui::ElementId::Name(format!("day-{date}").into()))
                        .flex()
                        .flex_1()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(3.))
                        .h(theme::CELL_HEIGHT)
                        .rounded(theme::CELL_RADIUS)
                        .cursor_pointer()
                        .text_size(theme::TEXT_BODY)
                        .text_color(fg)
                        .font_weight(if is_selected || is_today {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .when_some(bg, |el, c| el.bg(c))
                        .when(bg.is_none(), |el| el.hover(|s| s.bg(theme::HOVER)))
                        .on_click(cx.listener(move |this, _, _, cx| this.pick(date, cx)))
                        .child(div().child(date.day().to_string()))
                        .child(
                            div()
                                .size(theme::DOT_SIZE)
                                .rounded(px(2.))
                                .when_some(dot, |el, c| el.bg(c)),
                        )
                }))
        }))
}
