//! daybar — a macOS menu bar calendar.
//!
//! Scaffold: the status item is live and toggles a borderless non-activating
//! popover window; the popover currently renders a placeholder. The real views
//! land in `src/ui/`.

// Scaffold: theme tokens, model helpers and the calendar trait exist for the
// views that other agents are filling in, so they are not all called yet.
#![allow(dead_code)]

mod calendar;
mod model;
mod status_item;
mod ui;

use chrono::{Datelike, Local};
use futures::StreamExt;
use gpui::{
    actions, div, point, px, size, App, AppContext, Application, Bounds, Context, FocusHandle,
    Focusable, InteractiveElement, IntoElement, KeyBinding, ParentElement, Pixels, Render, Size,
    Styled, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind,
    WindowOptions,
};
use objc2::MainThreadMarker;

use status_item::{ScreenRect, StatusItem, StatusItemEvent};
use ui::theme;

actions!(daybar, [Close]);

/// Key context the popover's bindings are scoped to.
const KEY_CONTEXT: &str = "Daybar";

/// Placeholder root view. Replaced by `ui::popover::Popover`.
struct Placeholder {
    focus: FocusHandle,
    /// gpui fires the activation observer once at registration time, and the
    /// panel is not key yet at that point. Only start closing on deactivation
    /// after the window has genuinely been active once.
    has_been_active: bool,
}

impl Focusable for Placeholder {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Placeholder {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            // Key bindings are dispatched against this context, and the focus
            // handle is what makes the popover a key-event target at all.
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .on_action(|_: &Close, window: &mut Window, _: &mut App| window.remove_window())
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::BG)
            .rounded(theme::POPOVER_RADIUS)
            .text_color(theme::TEXT)
            .text_size(theme::TEXT_TITLE)
            .child("daybar")
    }
}

/// Placeholder popover height until the real view drives it.
const POPOVER_HEIGHT: Pixels = px(420.);
/// Keep the popover this far from the screen edges.
const SCREEN_MARGIN: f32 = 8.0;

fn main() {
    Application::new().run(|cx: &mut App| {
        let mtm = MainThreadMarker::new().expect("gpui runs its callbacks on the main thread");
        status_item::set_accessory_activation_policy(mtm);

        cx.bind_keys([KeyBinding::new("escape", Close, Some(KEY_CONTEXT))]);

        let today = Local::now().date_naive();
        let (item, mut clicks) = StatusItem::new(mtm, today.day());

        // Held for the life of the process; dropping it removes the menu bar item.
        let item = std::rc::Rc::new(item);

        let mut window: Option<WindowHandle<Placeholder>> = None;

        cx.spawn(async move |cx| {
            while let Some(event) = clicks.next().await {
                if event == StatusItemEvent::ClickedOutside {
                    if let Some(handle) = window.take() {
                        let _ = cx.update(|cx| handle.update(cx, |_, w, _| w.remove_window()));
                    }
                    continue;
                }
                let anchor = item.screen_rect(mtm);
                let result = cx.update(|cx| {
                    // A stale handle (Esc or click-outside already closed the
                    // window) updates with an error — that counts as closed,
                    // so this click should open a fresh popover.
                    let closed_it = match window.take() {
                        Some(handle) => handle.update(cx, |_, w, _| w.remove_window()).is_ok(),
                        None => false,
                    };
                    if closed_it {
                        return;
                    }
                    match open_popover(cx, anchor) {
                        Ok(handle) => window = Some(handle),
                        Err(err) => eprintln!("daybar: could not open popover: {err}"),
                    }
                });
                if result.is_err() {
                    break; // app is shutting down
                }
            }
        })
        .detach();
    });
}

fn open_popover(
    cx: &mut App,
    anchor: Option<ScreenRect>,
) -> anyhow::Result<WindowHandle<Placeholder>> {
    let bounds = popover_bounds(cx, anchor);

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
            let view = cx.new(|cx| {
                // Losing key focus closes, which covers "clicked outside".
                cx.observe_window_activation(window, |this: &mut Placeholder, window, _| {
                    if window.is_window_active() {
                        this.has_been_active = true;
                    } else if this.has_been_active {
                        window.remove_window();
                    }
                })
                .detach();

                Placeholder {
                    focus: cx.focus_handle(),
                    has_been_active: false,
                }
            });

            window.focus(&view.read(cx).focus);
            view
        },
    )?;

    // The panel is non-activating, so ask AppKit to make it key explicitly.
    handle.update(cx, |_, window, _| window.activate_window())?;
    Ok(handle)
}

/// Place the popover directly under the status item, clamped to the screen.
fn popover_bounds(cx: &App, anchor: Option<ScreenRect>) -> Bounds<Pixels> {
    let display_bounds = cx.primary_display().map(|d| d.bounds()).unwrap_or(Bounds {
        origin: point(px(0.), px(0.)),
        size: size(px(1440.), px(900.)),
    });
    let screen_width: f32 = display_bounds.size.width.into();

    let popover: Size<Pixels> = size(theme::POPOVER_WIDTH, POPOVER_HEIGHT);
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
