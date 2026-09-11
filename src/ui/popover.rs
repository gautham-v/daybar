//! Root popover view: header (month title, ‹ ›, Day/Week toggle), weekday row,
//! month grid, rule, the day or week list, and the footer band.
//!
//! Owns the selected date, the Day/Week mode, the event provider and the clock,
//! and handles the key bindings (← → ↑ ↓, Enter, `t`, Esc).

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime};
use gpui::prelude::FluentBuilder;
use gpui::{
    actions, div, px, App, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, KeyBinding, ParentElement, Pixels, Render,
    StatefulInteractiveElement, Styled, Window,
};

use crate::model::{self, Event};
use crate::ui::{day_list, month_grid, theme, week_list};

/// Events the popover raises to whoever owns its window.
pub enum PopoverEvent {
    /// Esc (or any other dismissal the view decides on) — close the window.
    Close,
}

/// Which list is shown under the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Day,
    Week,
}

/// Anything that can answer "what is on this day?".
///
/// A `CalendarSource` is adapted into one of these by the window owner, so the
/// view never has to know about EventKit or the refresh timer.
pub type EventProvider = Box<dyn Fn(NaiveDate) -> Vec<Event>>;

/// Clock, injectable so previews and tests can pin "now".
pub type Clock = Box<dyn Fn() -> NaiveDateTime>;

actions!(
    daybar,
    [PrevDay, NextDay, PrevWeek, NextWeek, GoToday, OpenDay, Dismiss]
);

/// Key context the popover's bindings are scoped to.
pub const KEY_CONTEXT: &str = "Daybar";

/// Install the popover's key bindings. Call once at app start.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("left", PrevDay, Some(KEY_CONTEXT)),
        KeyBinding::new("right", NextDay, Some(KEY_CONTEXT)),
        KeyBinding::new("up", PrevWeek, Some(KEY_CONTEXT)),
        KeyBinding::new("down", NextWeek, Some(KEY_CONTEXT)),
        KeyBinding::new("t", GoToday, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", OpenDay, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", Dismiss, Some(KEY_CONTEXT)),
    ]);
}

pub struct Popover {
    focus: FocusHandle,
    /// The day whose events are listed.
    selected: NaiveDate,
    /// Any day in the month the grid is showing; may differ from `selected`
    /// while the user is paging around with the arrows.
    visible_month: NaiveDate,
    mode: Mode,
    provider: EventProvider,
    now: Clock,
}

impl Popover {
    pub fn new(provider: EventProvider, now: Clock, cx: &mut Context<Self>) -> Self {
        let today = now().date();
        Self {
            focus: cx.focus_handle(),
            selected: today,
            visible_month: today,
            mode: Mode::Day,
            provider,
            now,
        }
    }

    /// Convenience for the common case: system clock.
    pub fn with_system_clock(provider: EventProvider, cx: &mut Context<Self>) -> Self {
        Self::new(provider, Box::new(|| Local::now().naive_local()), cx)
    }

    pub fn now(&self) -> NaiveDateTime {
        (self.now)()
    }

    pub fn today(&self) -> NaiveDate {
        self.now().date()
    }

    pub fn selected(&self) -> NaiveDate {
        self.selected
    }

    pub fn visible_month(&self) -> NaiveDate {
        self.visible_month
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Events for `date`, sorted the way the lists want them.
    pub fn events_on(&self, date: NaiveDate) -> Vec<Event> {
        let mut events = (self.provider)(date);
        model::sort_for_day(&mut events);
        events
    }

    /// Does this day get a dot in the grid?
    pub(crate) fn has_events(&self, date: NaiveDate) -> bool {
        !(self.provider)(date).is_empty()
    }

    pub fn select(&mut self, date: NaiveDate, cx: &mut Context<Self>) {
        self.selected = date;
        self.visible_month = date;
        cx.notify();
    }

    /// Clicking a cell selects it *and* drops back to Day mode, per the spec.
    pub(crate) fn pick(&mut self, date: NaiveDate, cx: &mut Context<Self>) {
        self.mode = Mode::Day;
        self.select(date, cx);
    }

    fn shift(&mut self, days: i64, cx: &mut Context<Self>) {
        let next = self.selected + Duration::days(days);
        self.select(next, cx);
    }

    fn step_month(&mut self, months: i64, cx: &mut Context<Self>) {
        // Clamp the day so stepping from the 31st never falls off a short month.
        let (mut y, mut m) = (self.visible_month.year(), self.visible_month.month() as i64);
        m += months;
        while m < 1 {
            m += 12;
            y -= 1;
        }
        while m > 12 {
            m -= 12;
            y += 1;
        }
        self.visible_month = NaiveDate::from_ymd_opt(y, m as u32, 1)
            .expect("first of a normalized month is always valid");
        cx.notify();
    }

    fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.mode = mode;
        cx.notify();
    }

    fn go_today(&mut self, cx: &mut Context<Self>) {
        let today = self.today();
        self.mode = Mode::Day;
        self.select(today, cx);
    }

    fn open_in_google_calendar(&self, cx: &mut App) {
        cx.open_url(&model::google_calendar_day_url(self.selected));
    }

    // ── Sizing ───────────────────────────────────────────────────────────────

    /// Height of everything above the list, plus the footer.
    fn chrome_height(&self) -> f32 {
        HEADER_HEIGHT + WEEKDAY_ROW_HEIGHT + grid_height() + 1.0 + FOOTER_HEIGHT
    }

    /// Height the list wants, before the `LIST_MIN_HEIGHT` floor.
    fn list_height(&self) -> f32 {
        let inner = match self.mode {
            Mode::Day => day_list::content_height(&self.events_on(self.selected)),
            Mode::Week => week_list::content_height(
                &model::week_containing(self.selected).map(|d| self.events_on(d).len()),
            ),
        };
        let min: f32 = theme::LIST_MIN_HEIGHT.into();
        (inner + LIST_PAD_TOP + LIST_PAD_BOTTOM).max(min)
    }

    /// Height the window should be given for the current content.
    pub fn preferred_height(&self) -> Pixels {
        px(self.chrome_height() + self.list_height())
    }

    // ── Pieces ───────────────────────────────────────────────────────────────

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .pt(px(12.))
            .pb(px(8.))
            .px(theme::PAD_X)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.))
                    .child(arrow_button(
                        "prev",
                        "‹",
                        cx.listener(|this, _, _, cx| this.step_month(-1, cx)),
                    ))
                    .child(arrow_button(
                        "next",
                        "›",
                        cx.listener(|this, _, _, cx| this.step_month(1, cx)),
                    ))
                    .child(
                        div()
                            .ml(px(6.))
                            .text_size(theme::TEXT_TITLE)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::TEXT)
                            .child(model::month_title(self.visible_month)),
                    ),
            )
            .child(self.mode_toggle(cx))
    }

    fn mode_toggle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let segment = |label: &'static str, mode: Mode, active: bool, cx: &mut Context<Self>| {
            div()
                .id(label)
                .px(px(10.))
                .py(px(3.))
                .rounded(px(5.))
                .text_size(theme::TEXT_SMALL)
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .when_some(active.then_some(()), |el, _| el.bg(theme::SEGMENT_ACTIVE))
                .text_color(if active { theme::TEXT } else { theme::MUTED })
                .on_click(cx.listener(move |this, _, _, cx| this.set_mode(mode, cx)))
                .child(label)
        };

        let mode = self.mode;
        div()
            .flex()
            .flex_row()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(6.))
            .bg(theme::SEGMENT_TRACK)
            .child(segment("Day", Mode::Day, mode == Mode::Day, cx))
            .child(segment("Week", Mode::Week, mode == Mode::Week, cx))
    }

    fn weekday_row(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .px(theme::GRID_PAD_X)
            .pb(px(4.))
            .h(px(WEEKDAY_ROW_HEIGHT))
            .child(div().w(theme::WEEK_COLUMN_WIDTH).flex_shrink_0())
            .children(model::WEEKDAY_LETTERS.iter().map(|letter| {
                div()
                    .flex()
                    .flex_1()
                    .justify_center()
                    .text_size(theme::TEXT_MICRO)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::FAINT)
                    .child(*letter)
            }))
    }

    fn rule(&self) -> impl IntoElement {
        div().h(px(1.)).mx(theme::PAD_X).bg(theme::RULE)
    }

    fn footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let hint = |keys: &'static str, label: &'static str| {
            div()
                .flex()
                .flex_row()
                .gap(px(4.))
                .child(
                    div()
                        .font_family(theme::MONO_FAMILY)
                        .text_color(theme::MUTED)
                        .child(keys),
                )
                .child(label)
        };

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px(theme::PAD_X)
            .pt(px(8.))
            .pb(px(10.))
            .bg(theme::FOOTER_BG)
            .border_t_1()
            .border_color(theme::RULE)
            .text_size(theme::TEXT_TINY)
            .text_color(theme::FAINT)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(10.))
                    .child(hint("←→", "day"))
                    .child(hint("⏎", "open in Google Calendar")),
            )
            .child(
                div()
                    .id("today")
                    .cursor_pointer()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme::MUTED)
                    .hover(|s| s.text_color(theme::TEXT))
                    .on_click(cx.listener(|this, _, _, cx| this.go_today(cx)))
                    .child("Today"),
            )
    }
}

fn arrow_button(
    id: &'static str,
    glyph: &'static str,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(22.))
        .rounded(px(5.))
        .text_size(px(15.))
        .text_color(theme::MUTED)
        .cursor_pointer()
        .hover(|s| s.bg(theme::HOVER).text_color(theme::TEXT))
        .on_click(on_click)
        .child(glyph)
}

// ── Layout constants (see `docs/mockup-popover.dc.html`) ─────────────────────

const HEADER_HEIGHT: f32 = 12.0 + 24.0 + 8.0;
const WEEKDAY_ROW_HEIGHT: f32 = 18.0;
pub(crate) const LIST_PAD_TOP: f32 = 10.0;
pub(crate) const LIST_PAD_BOTTOM: f32 = 6.0;
const FOOTER_HEIGHT: f32 = 8.0 + 14.0 + 10.0;

fn grid_height() -> f32 {
    let cell: f32 = theme::CELL_HEIGHT.into();
    let gap: f32 = theme::CELL_GAP.into();
    6.0 * cell + 5.0 * gap + 10.0
}

impl EventEmitter<PopoverEvent> for Popover {}

impl Focusable for Popover {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Popover {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header = self.header(cx);
        let grid = month_grid::render(self, cx);
        let list: gpui::AnyElement = match self.mode {
            Mode::Day => day_list::render(self).into_any_element(),
            Mode::Week => week_list::render(self, cx).into_any_element(),
        };
        let footer = self.footer(cx);

        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &PrevDay, _, cx| this.shift(-1, cx)))
            .on_action(cx.listener(|this, _: &NextDay, _, cx| this.shift(1, cx)))
            .on_action(cx.listener(|this, _: &PrevWeek, _, cx| this.shift(-7, cx)))
            .on_action(cx.listener(|this, _: &NextWeek, _, cx| this.shift(7, cx)))
            .on_action(cx.listener(|this, _: &GoToday, _, cx| this.go_today(cx)))
            .on_action(cx.listener(|this, _: &OpenDay, _, cx| this.open_in_google_calendar(cx)))
            .on_action(cx.listener(|_, _: &Dismiss, _, cx| cx.emit(PopoverEvent::Close)))
            .flex()
            .flex_col()
            .w(theme::POPOVER_WIDTH)
            .min_h(self.preferred_height())
            .bg(theme::BG)
            .rounded(theme::POPOVER_RADIUS)
            .overflow_hidden()
            .font_family(theme::UI_FAMILY)
            .text_size(theme::TEXT_BODY)
            .text_color(theme::TEXT)
            .child(header)
            .child(self.weekday_row())
            .child(grid)
            .child(self.rule())
            .child(list)
            .child(footer)
    }
}

/// Build a popover entity backed by a [`crate::calendar::CalendarSource`].
///
/// The source is queried per day; implementations are expected to cache.
pub fn from_source<S>(source: S, cx: &mut Context<Popover>) -> Popover
where
    S: crate::calendar::CalendarSource + 'static,
{
    Popover::with_system_clock(Box::new(move |d| source.events_between(d, d)), cx)
}

/// Marker so callers can spell the entity type without importing gpui generics.
pub type PopoverEntity = Entity<Popover>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_constants_match_the_mockup() {
        assert_eq!(HEADER_HEIGHT, 44.0);
        // 6 rows of 34 with 2px gaps, plus the 10px bottom pad.
        assert_eq!(grid_height(), 224.0);
        assert_eq!(FOOTER_HEIGHT, 32.0);
    }
}
