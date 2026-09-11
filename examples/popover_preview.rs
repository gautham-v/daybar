//! Dev preview: renders the popover in a normal window against `StubSource`.
//!
//! `cargo run --example popover_preview`

use chrono::{Local, NaiveDate, NaiveDateTime};
use daybar::calendar::{stub::StubSource, CalendarSource};
use daybar::ui::popover::{self, Popover, PopoverEvent};
use gpui::{
    div, point, px, size, App, AppContext, Application, Bounds, Focusable, IntoElement,
    ParentElement, Render, Styled, TitlebarOptions, Window, WindowBounds, WindowOptions,
};

/// A frame around the popover so its rounded corners and shadowless edge are
/// visible against something that is not the popover's own background.
struct Preview {
    popover: gpui::Entity<Popover>,
}

impl Render for Preview {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .justify_center()
            .items_start()
            .bg(gpui::rgb(0xd7d3cc))
            .p(px(24.))
            .child(self.popover.clone())
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        popover::bind_keys(cx);

        let bounds = Bounds {
            origin: point(px(120.), px(120.)),
            size: size(px(368.), px(760.)),
        };

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("daybar preview".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                let popover = cx.new(|cx| {
                    let source = StubSource::new();
                    let provider = Box::new(move |d: NaiveDate| source.events_between(d, d));
                    // Pin "now" to mid-afternoon so past/next-up styling shows.
                    let now: Box<dyn Fn() -> NaiveDateTime> = Box::new(|| {
                        Local::now()
                            .date_naive()
                            .and_hms_opt(12, 40, 0)
                            .expect("valid time")
                    });
                    Popover::new(provider, now, cx)
                });

                cx.subscribe(&popover, |_, event, _| match event {
                    PopoverEvent::Close => println!("preview: popover asked to close"),
                })
                .detach();

                window.focus(&popover.focus_handle(cx));
                cx.new(|_| Preview { popover })
            },
        )
        .expect("open preview window");

        cx.activate(true);
    });
}
