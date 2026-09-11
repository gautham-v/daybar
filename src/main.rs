//! daybar — a macOS menu bar calendar.
//!
//! This file owns the plumbing only: the activation policy, the status item,
//! the popover window, and the refresh schedule. The views live in
//! `daybar::ui`, the data in `daybar::calendar`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration as StdDuration;

use chrono::{Datelike, Local};
use futures::StreamExt;
use gpui::{
    point, px, size, App, AppContext, Application, Bounds, Entity, Focusable, Pixels, Size,
    Subscription, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind,
    WindowOptions,
};
use objc2::MainThreadMarker;

use daybar::calendar::{eventkit::EventKitSource, CalendarStore};
use daybar::status_item::{ScreenRect, StatusItem, StatusItemEvent};
use daybar::ui::popover::{self, Popover, PopoverEvent};
use daybar::ui::theme;

/// How often the calendar is refetched while the app runs.
const REFRESH_EVERY: StdDuration = StdDuration::from_secs(5 * 60);
/// How often we check whether the date rolled over (so the menu bar number and
/// "today" stay right without waiting for the 5-minute refresh).
const DATE_TICK: StdDuration = StdDuration::from_secs(30);
/// Keep the popover this far from the screen edges.
const SCREEN_MARGIN: f32 = 8.0;

/// The popover window, when one is open. Shared by the click loop, the
/// dismissal subscriptions and the resize observer.
type WindowSlot = Rc<RefCell<Option<WindowHandle<Popover>>>>;

fn main() {
    Application::new().run(|cx: &mut App| {
        let mtm = MainThreadMarker::new().expect("gpui runs its callbacks on the main thread");
        daybar::status_item::set_accessory_activation_policy(mtm);
        popover::bind_keys(cx);

        let store = CalendarStore::new(Box::new(EventKitSource::new()));

        let today = Local::now().date_naive();
        let (item, mut clicks) = StatusItem::new(mtm, today.day());
        // Held for the life of the process; dropping it removes the menu bar item.
        let item = Rc::new(item);

        // One popover entity for the whole run: it keeps the provider closure
        // and is re-rendered into whatever window is open.
        let popover = {
            let store = store.clone();
            cx.new(|cx| Popover::with_system_clock(Box::new(move |date| store.events_on(date)), cx))
        };

        let window: WindowSlot = Rc::new(RefCell::new(None));
        // Replaced on every open so the previous window's observer is dropped.
        let activation: Rc<RefCell<Option<Subscription>>> = Rc::new(RefCell::new(None));

        // Esc (and anything else the view treats as a dismissal) closes.
        cx.subscribe(&popover, {
            let window = window.clone();
            move |_, event, cx| match event {
                PopoverEvent::Close => {
                    close_popover(&window, cx);
                }
            }
        })
        .detach();

        // The list grows and shrinks with the day and the Day/Week toggle, so
        // keep the panel sized to the content.
        cx.observe(&popover, {
            let window = window.clone();
            move |popover, cx| {
                let Some(handle) = *window.borrow() else {
                    return;
                };
                let height = popover.read(cx).preferred_height();
                let _ = handle.update(cx, |_, window, _| {
                    window.resize(size(theme::POPOVER_WIDTH, height))
                });
            }
        })
        .detach();

        // First load: this is the call that raises the calendar-access prompt.
        refresh_in_background(cx, store.clone(), popover.clone(), true);

        // Periodic refresh.
        cx.spawn({
            let store = store.clone();
            let popover = popover.clone();
            async move |cx| loop {
                cx.background_executor().timer(REFRESH_EVERY).await;
                let store = store.clone();
                cx.background_executor()
                    .spawn(async move { store.refresh() })
                    .await;
                if cx
                    .update(|cx| popover.update(cx, |_, cx| cx.notify()))
                    .is_err()
                {
                    break; // app is shutting down
                }
            }
        })
        .detach();

        // Date rollover: keep the menu bar number honest past midnight.
        cx.spawn({
            let item = item.clone();
            let store = store.clone();
            let popover = popover.clone();
            let mut shown = today;
            async move |cx| loop {
                cx.background_executor().timer(DATE_TICK).await;
                let now = Local::now().date_naive();
                if now == shown {
                    continue;
                }
                shown = now;
                if cx
                    .update(|cx| {
                        if let Some(mtm) = MainThreadMarker::new() {
                            item.set_day_number(mtm, now.day());
                        }
                        popover.update(cx, |_, cx| cx.notify());
                    })
                    .is_err()
                {
                    break;
                }
                // The cached window is anchored on "today"; slide it.
                let store = store.clone();
                cx.background_executor()
                    .spawn(async move { store.refresh() })
                    .await;
            }
        })
        .detach();

        cx.spawn({
            let item = item.clone();
            let store = store.clone();
            let popover = popover.clone();
            async move |cx| {
                while let Some(event) = clicks.next().await {
                    if event == StatusItemEvent::ClickedOutside {
                        if cx.update(|cx| close_popover(&window, cx)).is_err() {
                            break;
                        }
                        continue;
                    }

                    let anchor = item.screen_rect(mtm);
                    let result = cx.update(|cx| {
                        // A click while the popover is up is a toggle.
                        if close_popover(&window, cx) {
                            return;
                        }
                        popover.update(cx, |this, cx| this.reset(cx));
                        let height = popover.read(cx).preferred_height();
                        match open_popover(cx, anchor, height, popover.clone(), &activation) {
                            Ok(handle) => *window.borrow_mut() = Some(handle),
                            Err(err) => eprintln!("daybar: could not open popover: {err}"),
                        }
                        // Opening always re-reads the calendar.
                        refresh_in_background(cx, store.clone(), popover.clone(), true);
                    });
                    if result.is_err() {
                        break; // app is shutting down
                    }
                }
            }
        })
        .detach();
    });
}

/// Fetch the calendar off the main thread, then re-render the popover.
///
/// `prompt` asks for calendar access first; that call blocks while the system
/// prompt is up, which is exactly why it runs on the background executor.
fn refresh_in_background(
    cx: &mut App,
    store: CalendarStore,
    popover: Entity<Popover>,
    prompt: bool,
) {
    cx.spawn(async move |cx| {
        let worker = store.clone();
        cx.background_executor()
            .spawn(async move {
                if prompt {
                    worker.request_access();
                }
                worker.refresh();
            })
            .await;
        let _ = cx.update(|cx| popover.update(cx, |_, cx| cx.notify()));
    })
    .detach();
}

/// Close the popover if one is open. Returns whether it closed something.
fn close_popover(window: &WindowSlot, cx: &mut App) -> bool {
    let handle = window.borrow_mut().take();
    match handle {
        // A stale handle (the window is already gone) updates with an error;
        // that counts as "nothing was open".
        Some(handle) => handle.update(cx, |_, w, _| w.remove_window()).is_ok(),
        None => false,
    }
}

fn open_popover(
    cx: &mut App,
    anchor: Option<ScreenRect>,
    height: Pixels,
    popover: Entity<Popover>,
    activation: &Rc<RefCell<Option<Subscription>>>,
) -> anyhow::Result<WindowHandle<Popover>> {
    let bounds = popover_bounds(cx, anchor, height);

    let activation = activation.clone();
    let handle = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            focus: true,
            show: true,
            // PopUp maps to a borderless NSPanel with the non-activating style
            // mask on macOS, which is exactly the menu-bar popover behavior.
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            window_background: WindowBackgroundAppearance::Transparent,
            window_min_size: None,
            display_id: None,
            app_id: None,
            window_decorations: None,
            tabbing_identifier: None,
        },
        |window, cx| {
            let subscription = popover.update(cx, |_, cx| {
                // gpui fires this observer once at registration, before the
                // panel is key; only treat deactivation as a dismissal after
                // the window has genuinely been active.
                let was_active = Cell::new(false);
                cx.observe_window_activation(window, move |_, window, _| {
                    if window.is_window_active() {
                        was_active.set(true);
                    } else if was_active.get() {
                        window.remove_window();
                    }
                })
            });
            // Dropping the previous window's observer.
            *activation.borrow_mut() = Some(subscription);

            window.focus(&popover.focus_handle(cx));
            popover.clone()
        },
    )?;

    // The panel is non-activating, so ask AppKit to make it key explicitly.
    handle.update(cx, |_, window, _| window.activate_window())?;
    Ok(handle)
}

/// Place the popover directly under the status item, clamped to the screen.
fn popover_bounds(cx: &App, anchor: Option<ScreenRect>, height: Pixels) -> Bounds<Pixels> {
    let display_bounds = cx.primary_display().map(|d| d.bounds()).unwrap_or(Bounds {
        origin: point(px(0.), px(0.)),
        size: size(px(1440.), px(900.)),
    });
    let screen_width: f32 = display_bounds.size.width.into();

    let popover: Size<Pixels> = size(theme::POPOVER_WIDTH, height);
    let width: f32 = theme::POPOVER_WIDTH.into();
    let gap: f32 = theme::POPOVER_TOP_GAP.into();

    let (x, y) = match anchor {
        Some(a) => {
            let centered = a.x + a.width / 2.0 - width / 2.0;
            let max_x = (screen_width - width - SCREEN_MARGIN).max(SCREEN_MARGIN);
            (centered.clamp(SCREEN_MARGIN, max_x), a.y + a.height + gap)
        }
        // No status item frame (shouldn't happen): hug the top-right corner.
        None => (screen_width - width - SCREEN_MARGIN, 28.0 + gap),
    };

    Bounds {
        origin: point(px(x), px(y)),
        size: popover,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, width: f32) -> ScreenRect {
        ScreenRect {
            x,
            y: 0.0,
            width,
            height: 24.0,
        }
    }

    #[test]
    fn anchor_math_centers_under_the_item() {
        let a = rect(1000.0, 60.0);
        let width: f32 = theme::POPOVER_WIDTH.into();
        let centered = a.x + a.width / 2.0 - width / 2.0;
        assert_eq!(centered, 1000.0 + 30.0 - 160.0);
    }

    #[test]
    fn anchor_math_clamps_to_the_right_edge() {
        let screen_width = 1440.0_f32;
        let width: f32 = theme::POPOVER_WIDTH.into();
        let a = rect(1400.0, 40.0);
        let centered = a.x + a.width / 2.0 - width / 2.0;
        let max_x = (screen_width - width - SCREEN_MARGIN).max(SCREEN_MARGIN);
        assert_eq!(centered.clamp(SCREEN_MARGIN, max_x), 1440.0 - 320.0 - 8.0);
    }
}
