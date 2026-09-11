//! All the Cocoa glue lives here: activation policy, the `NSStatusItem`, and
//! the bridge from an AppKit click back into gpui.
//!
//! The bridge is deliberately dumb: the status item's target/action is a tiny
//! `objc2` class that owns an unbounded channel sender and pushes a
//! [`StatusItemEvent`] on every click. `main.rs` drains that channel from a
//! gpui task, so nothing AppKit-shaped leaks into the views.

use block2::RcBlock;
use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{define_class, msg_send, sel, AnyThread, DefinedClass, MainThreadMarker};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSCellImagePosition, NSControl, NSEvent,
    NSEventMask, NSImage, NSScreen, NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

/// What the status item tells the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusItemEvent {
    /// The user clicked the menu bar item.
    Clicked,
    /// The user clicked somewhere outside this app; the popover should dismiss.
    ///
    /// A non-activating panel never makes daybar the active app, so AppKit's
    /// resign-key notification is not a reliable "clicked outside" signal — a
    /// global event monitor is. Global monitors never see our own app's
    /// clicks, so clicking the status item itself does not produce this.
    ClickedOutside,
}

/// A rectangle in gpui's screen coordinate space (top-left origin, y down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

struct TargetIvars {
    tx: UnboundedSender<StatusItemEvent>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "DaybarStatusItemTarget"]
    #[ivars = TargetIvars]
    struct StatusItemTarget;

    unsafe impl NSObjectProtocol for StatusItemTarget {}

    impl StatusItemTarget {
        #[unsafe(method(daybarStatusItemClicked:))]
        fn clicked(&self, _sender: *mut AnyObject) {
            // Unbounded send only fails once the receiver is gone, which means
            // the app is shutting down — dropping the click is correct then.
            let _ = self.ivars().tx.unbounded_send(StatusItemEvent::Clicked);
        }
    }
);

impl StatusItemTarget {
    fn new(tx: UnboundedSender<StatusItemEvent>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(TargetIvars { tx });
        unsafe { msg_send![super(this), init] }
    }
}

/// Owns the menu bar item for the lifetime of the app.
///
/// Dropping it removes the item from the menu bar, so keep it alive.
pub struct StatusItem {
    item: Retained<NSStatusItem>,
    // Held so the target outlives the status item's unretained `target` pointer.
    _target: Retained<StatusItemTarget>,
    // The global mouse-down monitor; removed on drop.
    outside_monitor: Option<Retained<objc2::runtime::AnyObject>>,
}

impl StatusItem {
    /// Install the menu bar item. Returns it plus the click channel.
    ///
    /// `day_number` is today's day-of-month, rendered next to the glyph.
    pub fn new(
        mtm: MainThreadMarker,
        day_number: u32,
    ) -> (Self, UnboundedReceiver<StatusItemEvent>) {
        let (tx, rx) = mpsc::unbounded();
        let tx_outside = tx.clone();
        let target = StatusItemTarget::new(tx);

        let bar = NSStatusBar::systemStatusBar();
        let item = bar.statusItemWithLength(NSVariableStatusItemLength);

        if let Some(button) = item.button(mtm) {
            unsafe {
                button.setTitle(&NSString::from_str(&day_number.to_string()));

                // A template SF Symbol tints itself for light/dark menu bars.
                let name = NSString::from_str("calendar");
                let desc = NSString::from_str("Calendar");
                if let Some(image) =
                    NSImage::imageWithSystemSymbolName_accessibilityDescription(&name, Some(&desc))
                {
                    image.setTemplate(true);
                    image.setSize(NSSize::new(14.0, 14.0));
                    button.setImage(Some(&image));
                    button.setImagePosition(NSCellImagePosition::ImageLeft);
                }

                let control: &NSControl = &button;
                control.setTarget(Some(&*target));
                control.setAction(Some(sel!(daybarStatusItemClicked:)));
            }
        }

        let outside_monitor = install_outside_click_monitor(tx_outside);

        (
            Self {
                item,
                _target: target,
                outside_monitor,
            },
            rx,
        )
    }

    /// Update the day number shown in the menu bar (call when the date rolls over).
    pub fn set_day_number(&self, mtm: MainThreadMarker, day_number: u32) {
        if let Some(button) = self.item.button(mtm) {
            button.setTitle(&NSString::from_str(&day_number.to_string()));
        }
    }

    /// The status item button's frame, in gpui screen coordinates.
    ///
    /// AppKit hands back a bottom-left-origin rect on the status bar's own
    /// window; gpui wants top-left-origin relative to the primary display, so
    /// we flip through the primary screen's height.
    pub fn screen_rect(&self, mtm: MainThreadMarker) -> Option<ScreenRect> {
        let button = self.item.button(mtm)?;
        let window = button.window()?;
        let frame: NSRect = window.frame();
        let flip_height = primary_screen_height(mtm)?;

        Some(ScreenRect {
            x: frame.origin.x as f32,
            // Top edge in flipped coords.
            y: (flip_height - (frame.origin.y + frame.size.height)) as f32,
            width: frame.size.width as f32,
            height: frame.size.height as f32,
        })
    }
}

/// Height of the primary screen (the one whose origin is 0,0), used as the
/// reference for flipping AppKit's y axis.
fn primary_screen_height(mtm: MainThreadMarker) -> Option<f64> {
    let screens = NSScreen::screens(mtm);
    let mut fallback: Option<f64> = None;
    for screen in screens.iter() {
        let frame = screen.frame();
        if fallback.is_none() {
            fallback = Some(frame.size.height);
        }
        if frame.origin == NSPoint::new(0.0, 0.0) {
            return Some(frame.size.height);
        }
    }
    fallback
}

/// Run as a menu bar accessory: no Dock icon, no menu bar menus, never the
/// active app.
pub fn set_accessory_activation_policy(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
}

impl Drop for StatusItem {
    fn drop(&mut self) {
        if let Some(monitor) = self.outside_monitor.take() {
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
    }
}

/// Watch for mouse-downs in other applications so the popover can dismiss.
fn install_outside_click_monitor(
    tx: UnboundedSender<StatusItemEvent>,
) -> Option<Retained<objc2::runtime::AnyObject>> {
    let mask =
        NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown | NSEventMask::OtherMouseDown;
    let handler = RcBlock::new(move |_event: core::ptr::NonNull<NSEvent>| {
        let _ = tx.unbounded_send(StatusItemEvent::ClickedOutside);
    });
    let monitor = unsafe { NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &handler) };
    if monitor.is_none() {
        eprintln!("daybar: global mouse monitor unavailable; outside clicks will not dismiss");
    }
    monitor
}
