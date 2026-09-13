//! The menu bar glyph: a small calendar frame with today's day number inside.
//!
//! Drawn at runtime with Core Graphics rather than shipped as an asset, because
//! the number changes at midnight and a template image has to be regenerated to
//! change. The image is marked `isTemplate`, so AppKit throws away the colours
//! and keeps only the alpha — it then tints the glyph for the current menu bar
//! appearance (dark, light, and the "reduce transparency" variants) for free.
//! Everything below therefore draws in opaque black on a clear background.

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_app_kit::{
    NSAttributedStringNSStringDrawing, NSColor, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSGraphicsContext, NSImage, NSKernAttributeName,
};
use objc2_core_graphics::{CGContext, CGLineCap, CGLineJoin};
use objc2_foundation::{
    NSAttributedString, NSDictionary, NSNumber, NSPoint, NSRect, NSSize, NSString,
};

/// Glyph box, in points. Matched to the optical size of the system glyphs
/// (Wi-Fi, globe, battery) rather than to the menu bar's full 22pt height.
pub const WIDTH: f64 = 20.0;
pub const HEIGHT: f64 = 15.0;

/// The calendar frame: a plain rounded-rect outline. No header band — at this
/// size it read as a smudge and crowded the number.
const STROKE: f64 = 1.1;
const RADIUS: f64 = 3.5;
/// The day number.
const FONT_SIZE: f64 = 10.0;
const KERN: f64 = -0.4;

/// A template [`NSImage`] of the calendar frame with `day` drawn inside it.
pub fn calendar_image(day: u32) -> Retained<NSImage> {
    let label = day_label(day);
    let handler = RcBlock::new(move |_dirty: NSRect| -> Bool {
        draw(&label);
        Bool::YES
    });
    let image =
        NSImage::imageWithSize_flipped_drawingHandler(NSSize::new(WIDTH, HEIGHT), false, &handler);
    image.setTemplate(true);
    image
}

/// What goes inside the frame. Days are 1..=31, so this is always one or two
/// digits; anything out of range is clamped rather than drawn as an overflow.
pub fn day_label(day: u32) -> String {
    day.clamp(1, 31).to_string()
}

/// Vertical centre the number is optically aligned to, in the image's own
/// bottom-left-origin coordinates: the middle of the frame.
pub fn number_center_y() -> f64 {
    HEIGHT / 2.0
}

/// Draw into whatever context AppKit has made current for the drawing handler.
fn draw(label: &str) {
    let Some(ctx) = NSGraphicsContext::currentContext() else {
        return;
    };
    let cg = ctx.CGContext();
    let cg = Some(&*cg);

    CGContext::set_should_antialias(cg, true);
    CGContext::set_line_width(cg, STROKE);
    CGContext::set_line_cap(cg, CGLineCap::Round);
    CGContext::set_line_join(cg, CGLineJoin::Round);
    CGContext::set_gray_stroke_color(cg, 0.0, 1.0);
    CGContext::set_gray_fill_color(cg, 0.0, 1.0);

    // The frame, inset by half the stroke so the outline lands inside the box.
    let h = STROKE / 2.0;
    let (left, right) = (h, WIDTH - h);
    let (bottom, top) = (h, HEIGHT - h);
    rounded_rect(cg, left, bottom, right, top, RADIUS);
    CGContext::stroke_path(cg);

    draw_number(label);
}

/// A rounded rectangle from its edges, as a closed path.
fn rounded_rect(cg: Option<&CGContext>, left: f64, bottom: f64, right: f64, top: f64, r: f64) {
    CGContext::begin_path(cg);
    CGContext::move_to_point(cg, left + r, bottom);
    CGContext::add_arc_to_point(cg, right, bottom, right, top, r);
    CGContext::add_arc_to_point(cg, right, top, left, top, r);
    CGContext::add_arc_to_point(cg, left, top, left, bottom, r);
    CGContext::add_arc_to_point(cg, left, bottom, right, bottom, r);
    CGContext::close_path(cg);
}

/// The day number, centred in the frame.
fn draw_number(label: &str) {
    let font =
        unsafe { NSFont::systemFontOfSize_weight(FONT_SIZE, objc2_app_kit::NSFontWeightSemibold) };
    let black = NSColor::blackColor();
    let kern = NSNumber::new_f64(KERN);
    let attrs = unsafe {
        NSDictionary::from_slices(
            &[
                NSFontAttributeName,
                NSForegroundColorAttributeName,
                NSKernAttributeName,
            ],
            &[&*font as &objc2::runtime::AnyObject, &*black, &*kern],
        )
    };
    // Safety: the three attributes are the documented types for those keys.
    let string =
        unsafe { NSAttributedString::new_with_attributes(&NSString::from_str(label), &attrs) };
    let size = string.size();

    // The trailing kern is applied after the last glyph too, so take it back
    // out before centring or the number sits a hair left of centre.
    let width = size.width - KERN;
    let x = (WIDTH - width) / 2.0;
    // Optical centring, not box centring: digits have no descender, so the
    // line box sits low. Put the baseline half a cap-height below the middle
    // and let `drawAtPoint`'s descender offset do the rest.
    let baseline = number_center_y() - font.capHeight() / 2.0;
    string.drawAtPoint(NSPoint::new(x, baseline + font.descender()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_labels_are_one_or_two_digits() {
        assert_eq!(day_label(1), "1");
        assert_eq!(day_label(31), "31");
    }

    #[test]
    fn out_of_range_days_are_clamped_rather_than_overflowing_the_frame() {
        assert_eq!(day_label(0), "1");
        assert_eq!(day_label(99), "31");
    }

    #[test]
    fn the_number_is_centred_on_the_frame() {
        assert_eq!(number_center_y(), HEIGHT / 2.0);
    }
}
