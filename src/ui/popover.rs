//! Root popover view: header (‹ › chevrons, centered month title, "···" menu),
//! weekday row, month grid, rule, the day or week list, and the footer.
//!
//! Owns the selected date, the Day/Week mode, which event row is expanded and
//! which is keyboard-focused, the event provider and the clock, and the key
//! bindings (← → ↑ ↓, Enter, Space, `t`, Esc).

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime};
use gpui::prelude::FluentBuilder;
use gpui::{
    actions, div, px, App, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, KeyBinding, ParentElement, Pixels, Render,
    StatefulInteractiveElement, Styled, Window,
};

use crate::calendar::AccessState;
use crate::model::{self, Event};
use crate::ui::theme::Theme;
use crate::ui::{day_list, month_grid, theme, week_list};

/// Events the popover raises to whoever owns its window.
pub enum PopoverEvent {
    /// Esc (or any other dismissal the view decides on) — close the window.
    Close,
    /// The "···" menu's Refresh item — refetch the calendar now.
    Refresh,
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

/// Reports the data source's permission state, so an empty list can explain
/// itself instead of reading as a free day.
pub type AccessProbe = Box<dyn Fn() -> AccessState>;

/// Reports the inclusive `[from, to]` range the provider actually has data for.
/// Navigation is clamped to it — past the edge every day would look empty.
pub type WindowProbe = Box<dyn Fn() -> (NaiveDate, NaiveDate)>;

actions!(
    daybar,
    [PrevDay, NextDay, PrevWeek, NextWeek, GoToday, OpenDay, FocusList, ToggleRow, Dismiss]
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
        KeyBinding::new("tab", FocusList, Some(KEY_CONTEXT)),
        KeyBinding::new("space", ToggleRow, Some(KEY_CONTEXT)),
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
    /// Id of the one expanded event row, if any.
    expanded: Option<String>,
    /// Index of the keyboard-focused row within the day list, if the list has
    /// keyboard focus at all.
    focused_row: Option<usize>,
    /// Whether the "···" menu is showing.
    menu_open: bool,
    /// Colors for the window's current appearance, refreshed each render.
    theme: Theme,
    /// Redraws the popover when the system flips between light and dark.
    appearance: Option<gpui::Subscription>,
    provider: EventProvider,
    now: Clock,
    access: AccessProbe,
    bounds: Option<WindowProbe>,
}

impl Popover {
    pub fn new(provider: EventProvider, now: Clock, cx: &mut Context<Self>) -> Self {
        let today = now().date();
        Self {
            focus: cx.focus_handle(),
            selected: today,
            visible_month: today,
            mode: Mode::Day,
            expanded: None,
            focused_row: None,
            menu_open: false,
            theme: Theme::default(),
            appearance: None,
            provider,
            now,
            access: Box::new(|| AccessState::Granted),
            bounds: None,
        }
    }

    /// Tell the view how to read the source's permission state.
    pub fn with_access(mut self, access: AccessProbe) -> Self {
        self.access = access;
        self
    }

    /// Confine navigation to the range the provider can answer for.
    pub fn with_bounds(mut self, bounds: WindowProbe) -> Self {
        self.bounds = bounds.into();
        self
    }

    /// The source's current permission state.
    pub fn access(&self) -> AccessState {
        (self.access)()
    }

    /// Clamp a date into the provider's window, if one was supplied.
    fn clamp(&self, date: NaiveDate) -> NaiveDate {
        clamp_to(date, self.bounds.as_ref().map(|b| b()))
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

    /// Colors for the window's appearance.
    pub fn theme(&self) -> Theme {
        self.theme
    }

    /// Id of the expanded row, if any.
    pub fn expanded(&self) -> Option<&str> {
        self.expanded.as_deref()
    }

    /// Index of the keyboard-focused row, if the list holds focus.
    pub fn focused_row(&self) -> Option<usize> {
        self.focused_row
    }

    pub(crate) fn menu_open(&self) -> bool {
        self.menu_open
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
        let date = self.clamp(date);
        self.selected = date;
        self.visible_month = date;
        // A different day is a different list; nothing stays open or focused.
        self.expanded = None;
        self.focused_row = None;
        cx.notify();
    }

    /// Clicking a cell selects it *and* drops back to Day mode, per the spec.
    pub(crate) fn pick(&mut self, date: NaiveDate, cx: &mut Context<Self>) {
        self.mode = Mode::Day;
        self.menu_open = false;
        self.select(date, cx);
    }

    // ── Row state ────────────────────────────────────────────────────────────

    /// Click (or Enter/Space) on row `index`: focus it and toggle its details.
    pub(crate) fn toggle_row(&mut self, index: usize, id: &str, cx: &mut Context<Self>) {
        self.focused_row = Some(index);
        self.expanded = if self.expanded.as_deref() == Some(id) {
            None
        } else {
            Some(id.to_string())
        };
        self.menu_open = false;
        cx.notify();
    }

    /// Number of rows the day list currently has.
    fn row_count(&self) -> usize {
        match self.mode {
            Mode::Day => self.events_on(self.selected).len(),
            Mode::Week => 0,
        }
    }

    /// Toggle the focused row, entering the list at the first row when nothing
    /// is focused yet. Returns false when there is no list to enter.
    fn toggle_focused_row(&mut self, cx: &mut Context<Self>) -> bool {
        let count = self.row_count();
        if count == 0 {
            return false;
        }
        let index = self.focused_row.unwrap_or(0).min(count - 1);
        let Some(id) = self
            .events_on(self.selected)
            .get(index)
            .map(|e| e.id.clone())
        else {
            return false;
        };
        self.toggle_row(index, &id, cx);
        true
    }

    /// Tab: put keyboard focus on the list without disclosing anything, then
    /// step through the rows.
    fn focus_list(&mut self, cx: &mut Context<Self>) {
        let count = self.row_count();
        if count == 0 {
            return;
        }
        self.focused_row = Some(match self.focused_row {
            Some(i) => (i + 1) % count,
            None => 0,
        });
        cx.notify();
    }

    /// ↑/↓ inside the list. Returns false when focus is not in the list, so the
    /// caller can fall back to moving a week.
    fn move_row(&mut self, delta: isize, cx: &mut Context<Self>) -> bool {
        let count = self.row_count();
        let Some(current) = self.focused_row else {
            return false;
        };
        if count == 0 {
            self.focused_row = None;
            return false;
        }
        let next = (current as isize + delta).clamp(0, count as isize - 1) as usize;
        self.focused_row = Some(next);
        // Keep the disclosure with the focus: an open row follows the cursor.
        if self.expanded.is_some() {
            if let Some(event) = self.events_on(self.selected).get(next) {
                self.expanded = Some(event.id.clone());
            }
        }
        cx.notify();
        true
    }

    /// Esc: collapse an expanded row first, then close the popover.
    fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.menu_open {
            self.menu_open = false;
            cx.notify();
        } else if self.expanded.is_some() {
            self.expanded = None;
            cx.notify();
        } else {
            cx.emit(PopoverEvent::Close);
        }
    }

    fn shift(&mut self, days: i64, cx: &mut Context<Self>) {
        let next = self.selected + Duration::days(days);
        self.select(next, cx);
    }

    fn step_month(&mut self, months: i64, cx: &mut Context<Self>) {
        // Clamping to the window can land mid-month; the grid only needs *some*
        // day in the month it should draw.
        self.visible_month = self.clamp(month_start(self.visible_month, months));
        self.menu_open = false;
        cx.notify();
    }

    fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.mode = mode;
        self.expanded = None;
        self.focused_row = None;
        self.menu_open = false;
        cx.notify();
    }

    fn toggle_week_mode(&mut self, cx: &mut Context<Self>) {
        let next = match self.mode {
            Mode::Day => Mode::Week,
            Mode::Week => Mode::Day,
        };
        self.set_mode(next, cx);
    }

    fn toggle_menu(&mut self, cx: &mut Context<Self>) {
        self.menu_open = !self.menu_open;
        cx.notify();
    }

    /// Back to today in Day mode — what a fresh popover open should show.
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.go_today(cx);
    }

    fn go_today(&mut self, cx: &mut Context<Self>) {
        let today = self.today();
        self.mode = Mode::Day;
        self.menu_open = false;
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

    /// Height the list wants, expanded row included.
    fn list_height(&self) -> f32 {
        let inner = match self.mode {
            Mode::Day => day_list::content_height(
                &self.events_on(self.selected),
                self.access(),
                self.expanded.as_deref(),
            ),
            Mode::Week => week_list::content_height(
                &model::week_containing(self.selected).map(|d| self.events_on(d).len()),
            ),
        };
        inner + LIST_PAD_TOP + LIST_PAD_BOTTOM
    }

    /// Height the window should be given for the current content.
    pub fn preferred_height(&self) -> Pixels {
        px(self.chrome_height() + self.list_height())
    }

    // ── Pieces ───────────────────────────────────────────────────────────────

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .pt(px(12.))
            .pb(px(6.))
            .px(theme::PAD_X)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.))
                    .child(icon_button(
                        "prev",
                        "‹",
                        theme,
                        cx.listener(|this, _, _, cx| this.step_month(-1, cx)),
                    ))
                    .child(icon_button(
                        "next",
                        "›",
                        theme,
                        cx.listener(|this, _, _, cx| this.step_month(1, cx)),
                    )),
            )
            .child(
                div()
                    .text_size(theme::TEXT_TITLE)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text)
                    .child(model::month_title(self.visible_month)),
            )
            .child(icon_button(
                "more",
                "···",
                theme,
                cx.listener(|this, _, _, cx| this.toggle_menu(cx)),
            ))
    }

    /// The "···" dropdown, drawn over the grid inside the popover.
    fn menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let item = |id: &'static str,
                    label: &'static str,
                    enabled: bool,
                    cx: &mut Context<Self>,
                    action: fn(&mut Self, &mut Context<Self>)| {
            div()
                .id(id)
                .px(px(10.))
                .py(px(5.))
                .rounded(px(5.))
                .text_size(theme::TEXT_SMALL)
                .text_color(if enabled { theme.text } else { theme.tertiary })
                .when(enabled, |el| {
                    el.cursor_pointer()
                        .hover(|s| s.bg(theme.hover))
                        .on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
                })
                .child(label)
        };

        div()
            .absolute()
            // Without this the month grid underneath the panel gets the same
            // click and quietly changes the selected day.
            .occlude()
            .top(px(HEADER_HEIGHT - 2.0))
            .right(theme::PAD_X)
            .w(px(168.))
            .p(px(4.))
            .rounded(px(8.))
            .bg(theme.menu_bg)
            .border_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .child(item("menu-refresh", "Refresh", true, cx, |this, cx| {
                this.menu_open = false;
                cx.emit(PopoverEvent::Refresh);
                cx.notify();
            }))
            .child(item("menu-login", "Launch at login", false, cx, |_, _| {}))
            .child(div().h(px(1.)).my(px(4.)).bg(theme.separator))
            .child(item("menu-quit", "Quit Daybar", true, cx, |this, cx| {
                this.menu_open = false;
                cx.quit();
            }))
    }

    fn weekday_row(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .px(theme::GRID_PAD_X)
            .pb(px(2.))
            .h(px(WEEKDAY_ROW_HEIGHT))
            .child(div().w(theme::WEEK_COLUMN_WIDTH).flex_shrink_0())
            .children(model::WEEKDAY_LETTERS.iter().map(|letter| {
                div()
                    .flex()
                    .flex_1()
                    .justify_center()
                    .text_size(theme::TEXT_MICRO)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(self.theme.tertiary)
                    .child(*letter)
            }))
    }

    fn rule(&self) -> impl IntoElement {
        div().h(px(1.)).mx(theme::PAD_X).bg(self.theme.separator)
    }

    fn footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let text_button = |id: &'static str,
                           label: &'static str,
                           cx: &mut Context<Self>,
                           action: fn(&mut Self, &mut Context<Self>)| {
            div()
                .id(id)
                .cursor_pointer()
                .text_color(theme.tertiary)
                .hover(|s| s.text_color(theme.text))
                .on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
                .child(label)
        };

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px(theme::PAD_X)
            .pt(px(8.))
            .pb(px(9.))
            .text_size(theme::TEXT_TINY)
            .text_color(theme.tertiary)
            .child(div().child(footer_stamp(
                self.selected,
                self.now(),
                self.selected == self.today(),
            )))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(12.))
                    .child(text_button("today", "Today", cx, |this, cx| {
                        this.go_today(cx)
                    }))
                    .child(text_button("week", "Week", cx, |this, cx| {
                        this.toggle_week_mode(cx)
                    })),
            )
    }
}

/// `"Fri, Sep 11 · 12:40"` for today, `"Fri, Sep 11"` for any other day — the
/// clock only means something when the date on the left is today's.
pub(crate) fn footer_stamp(date: NaiveDate, now: NaiveDateTime, is_today: bool) -> String {
    let stamp = date.format("%a, %b %-d").to_string();
    if is_today {
        format!("{stamp} · {}", model::short_time(now.time()))
    } else {
        stamp
    }
}

/// Clamp `date` into `bounds` (inclusive), or leave it alone when unbounded.
fn clamp_to(date: NaiveDate, bounds: Option<(NaiveDate, NaiveDate)>) -> NaiveDate {
    match bounds {
        Some((from, to)) => date.clamp(from.min(to), to.max(from)),
        None => date,
    }
}

/// The first of the month `months` away from `from`.
fn month_start(from: NaiveDate, months: i64) -> NaiveDate {
    let (mut y, mut m) = (from.year(), from.month() as i64);
    m += months;
    while m < 1 {
        m += 12;
        y -= 1;
    }
    while m > 12 {
        m -= 12;
        y += 1;
    }
    NaiveDate::from_ymd_opt(y, m as u32, 1).expect("first of a normalized month is always valid")
}

/// A 22px square glyph button: the header chevrons and the "···" menu.
fn icon_button(
    id: &'static str,
    glyph: &'static str,
    theme: Theme,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(theme::ICON_BUTTON)
        .flex_shrink_0()
        .rounded(px(5.))
        .text_size(theme::TEXT_BODY)
        .text_color(theme.secondary)
        .cursor_pointer()
        .hover(|s| s.bg(theme.hover).text_color(theme.text))
        .on_click(on_click)
        .child(glyph)
}

// ── Layout constants (see `docs/mockup-v2-inline-expand.dc.html`) ────────────

const HEADER_HEIGHT: f32 = 12.0 + 22.0 + 6.0;
const WEEKDAY_ROW_HEIGHT: f32 = 16.0;
pub(crate) const LIST_PAD_TOP: f32 = 6.0;
pub(crate) const LIST_PAD_BOTTOM: f32 = 4.0;
const FOOTER_HEIGHT: f32 = 8.0 + 14.0 + 9.0;

fn grid_height() -> f32 {
    let cell: f32 = theme::CELL_HEIGHT.into();
    6.0 * cell + 8.0
}

impl EventEmitter<PopoverEvent> for Popover {}

impl Focusable for Popover {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Popover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Follow the system appearance without the views having to ask. The
        // window only repaints when something notifies it, so a light/dark flip
        // while the popover is up has to wake it explicitly.
        if self.appearance.is_none() {
            let this = cx.entity();
            self.appearance = Some(window.observe_window_appearance(move |_window, cx| {
                this.update(cx, |_, cx| cx.notify());
            }));
        }
        let theme = Theme::for_appearance(window.appearance());
        if theme != self.theme {
            self.theme = theme;
        }

        let header = self.header(cx);
        let grid = month_grid::render(self, cx);
        let list: gpui::AnyElement = match self.mode {
            Mode::Day => day_list::render(self, cx).into_any_element(),
            Mode::Week => week_list::render(self, cx).into_any_element(),
        };
        let footer = self.footer(cx);
        // A transparent backdrop under the menu so a click anywhere else
        // dismisses it instead of falling through to the grid or the list.
        let backdrop = self.menu_open.then(|| {
            div()
                .id("menu-backdrop")
                .absolute()
                .inset_0()
                .occlude()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.menu_open = false;
                    cx.notify();
                }))
        });
        let menu = self.menu_open.then(|| self.menu(cx));

        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &PrevDay, _, cx| this.shift(-1, cx)))
            .on_action(cx.listener(|this, _: &NextDay, _, cx| this.shift(1, cx)))
            .on_action(cx.listener(|this, _: &PrevWeek, _, cx| {
                if !this.move_row(-1, cx) {
                    this.shift(-7, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &NextWeek, _, cx| {
                if !this.move_row(1, cx) {
                    this.shift(7, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &FocusList, _, cx| this.focus_list(cx)))
            .on_action(cx.listener(|this, _: &ToggleRow, _, cx| {
                this.toggle_focused_row(cx);
            }))
            .on_action(cx.listener(|this, _: &GoToday, _, cx| this.go_today(cx)))
            .on_action(cx.listener(|this, _: &OpenDay, _, cx| {
                // Enter toggles the focused row when the list has focus, and
                // otherwise opens the selected day in the browser.
                if this.focused_row.is_none() || !this.toggle_focused_row(cx) {
                    this.open_in_google_calendar(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Dismiss, _, cx| this.dismiss(cx)))
            .relative()
            .flex()
            .flex_col()
            .w(theme::POPOVER_WIDTH)
            .min_h(self.preferred_height())
            .bg(theme.bg)
            .rounded(theme::POPOVER_RADIUS)
            .border_1()
            .border_color(theme.border)
            .overflow_hidden()
            .font_family(theme::UI_FAMILY)
            .text_size(theme::TEXT_BODY)
            .text_color(theme.text)
            .child(header)
            .child(self.weekday_row())
            .child(grid)
            .child(self.rule())
            .child(list)
            .child(footer)
            .children(backdrop)
            .children(menu)
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

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    const WINDOW: (NaiveDate, NaiveDate) = (
        match NaiveDate::from_ymd_opt(2026, 7, 13) {
            Some(d) => d,
            None => panic!(),
        },
        match NaiveDate::from_ymd_opt(2026, 12, 10) {
            Some(d) => d,
            None => panic!(),
        },
    );

    #[test]
    fn navigation_saturates_at_the_data_window() {
        assert_eq!(clamp_to(d(2027, 5, 1), Some(WINDOW)), WINDOW.1);
        assert_eq!(clamp_to(d(2020, 1, 1), Some(WINDOW)), WINDOW.0);
        assert_eq!(clamp_to(d(2026, 9, 11), Some(WINDOW)), d(2026, 9, 11));
    }

    #[test]
    fn unbounded_navigation_is_left_alone() {
        assert_eq!(clamp_to(d(2099, 1, 1), None), d(2099, 1, 1));
    }

    #[test]
    fn month_paging_normalizes_across_year_boundaries() {
        assert_eq!(month_start(d(2026, 12, 31), 1), d(2027, 1, 1));
        assert_eq!(month_start(d(2026, 1, 31), -1), d(2025, 12, 1));
        assert_eq!(month_start(d(2026, 9, 11), 0), d(2026, 9, 1));
    }

    #[test]
    fn month_paging_past_the_window_pins_to_its_edge() {
        // Four clicks of › from September would be January 2027.
        let mut month = d(2026, 9, 1);
        for _ in 0..4 {
            month = clamp_to(month_start(month, 1), Some(WINDOW));
        }
        assert_eq!(month, WINDOW.1);
    }

    #[test]
    fn chrome_constants_match_the_mockup() {
        assert_eq!(HEADER_HEIGHT, 40.0);
        // 6 rows of 32, plus the 8px bottom pad.
        assert_eq!(grid_height(), 200.0);
        assert_eq!(FOOTER_HEIGHT, 31.0);
    }

    #[test]
    fn the_footer_stamp_only_carries_a_clock_for_today() {
        let now = d(2026, 9, 11).and_hms_opt(12, 40, 0).unwrap();
        assert_eq!(
            footer_stamp(d(2026, 9, 11), now, true),
            "Fri, Sep 11 · 12:40"
        );
        assert_eq!(footer_stamp(d(2026, 9, 14), now, false), "Mon, Sep 14");
    }
}
