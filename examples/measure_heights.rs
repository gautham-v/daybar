//! Checks that what the popover *draws* is as tall as what
//! [`Popover::preferred_height`] asks the window for — main.rs sizes the panel
//! to that number and clips the overflow, so a shortfall eats the footer.
//!
//! `cargo run --example measure_heights` — prints one line per state and exits
//! non-zero if any of them disagree by more than half a point.

use std::cell::Cell;
use std::rc::Rc;

use chrono::{Local, NaiveDate, NaiveDateTime};
use daybar::calendar::{stub::StubSource, CalendarSource};
use daybar::ui::icons::Assets;
use daybar::ui::popover::{self, Mode, Popover};
use gpui::{
    div, point, px, size, App, AppContext, Application, Bounds, IntoElement, ParentElement, Render,
    Styled, WindowBounds, WindowOptions,
};

struct Probe {
    popover: gpui::Entity<Popover>,
    label: Rc<Cell<&'static str>>,
    bad: Rc<Cell<bool>>,
}

impl Render for Probe {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let popover = self.popover.clone();
        let label = self.label.clone();
        let bad = self.bad.clone();
        div()
            .size_full()
            .child(self.popover.clone())
            .on_children_prepainted(move |bounds, _window, cx| {
                let drawn = bounds.first().map(|b| b.size.height).unwrap_or(px(0.));
                let want = popover.read(cx).preferred_height();
                let delta = f32::from(drawn) - f32::from(want);
                if delta.abs() > 0.5 {
                    bad.set(true);
                }
                println!(
                    "{:<22} drawn {:>7.2}  preferred {:>7.2}  delta {:>6.2}",
                    label.get(),
                    f32::from(drawn),
                    f32::from(want),
                    delta
                );
            })
    }
}

fn main() {
    Application::new().with_assets(Assets).run(|cx: &mut App| {
        popover::bind_keys(cx);
        let bounds = Bounds {
            origin: point(px(80.), px(80.)),
            size: size(px(300.), px(900.)),
        };
        let label = Rc::new(Cell::new("collapsed"));
        let bad = Rc::new(Cell::new(false));

        let handle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_window, cx| {
                    let popover = cx.new(|cx| {
                        let source = StubSource::new();
                        let provider = Box::new(move |d: NaiveDate| source.events_between(d, d));
                        let now: Box<dyn Fn() -> NaiveDateTime> = Box::new(|| {
                            Local::now()
                                .date_naive()
                                .and_hms_opt(12, 40, 0)
                                .expect("valid time")
                        });
                        Popover::new(provider, now, cx)
                    });
                    cx.new(|_| Probe {
                        popover,
                        label: label.clone(),
                        bad: bad.clone(),
                    })
                },
            )
            .expect("open probe window");

        cx.spawn(async move |cx| {
            // One frame per state, each measured by the prepaint listener.
            for (name, step) in [
                ("expanded row 0", 0usize),
                ("expanded row 1", 1),
                ("week mode", 2),
                ("tomorrow", 3),
            ] {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(400))
                    .await;
                label.set(name);
                let _ = handle.update(cx, |probe, _window, cx| {
                    probe.popover.update(cx, |p, cx| match step {
                        0 => {
                            let events = p.events_on(p.selected());
                            if let Some(e) = events.first() {
                                let id = e.id.clone();
                                p.toggle_row(0, &id, cx);
                            }
                        }
                        1 => {
                            let events = p.events_on(p.selected());
                            if let Some(e) = events.get(1) {
                                let id = e.id.clone();
                                p.toggle_row(1, &id, cx);
                            }
                        }
                        2 => p.set_mode(Mode::Week, cx),
                        _ => {
                            p.set_mode(Mode::Day, cx);
                            let next = p.selected().succ_opt().expect("a next day");
                            p.select(next, cx);
                        }
                    });
                    cx.notify();
                });
            }
            cx.background_executor()
                .timer(std::time::Duration::from_millis(500))
                .await;
            let failed = bad.get();
            let _ = cx.update(|cx| cx.quit());
            if failed {
                eprintln!("MISMATCH: drawn height differs from preferred_height()");
                std::process::exit(1);
            }
            println!("all states match preferred_height()");
        })
        .detach();

        cx.activate(false);
    });
}
