//! Visual tokens from `docs/mockup-popover.dc.html`, as gpui types.
//!
//! Everything the views need should come from here — no literal colors or
//! magic numbers in `popover.rs`, `month_grid.rs`, `day_list.rs`, `week_list.rs`.

use gpui::{px, Pixels, Rgba};

/// `const`-friendly hex -> [`Rgba`] (gpui's own `rgb()` is not `const`).
const fn hex(value: u32) -> Rgba {
    Rgba {
        r: ((value >> 16) & 0xff) as f32 / 255.0,
        g: ((value >> 8) & 0xff) as f32 / 255.0,
        b: (value & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

/// Same, with an explicit alpha in 0.0..=1.0.
const fn hex_a(value: u32, alpha: f32) -> Rgba {
    let c = hex(value);
    Rgba { a: alpha, ..c }
}

// ── Colors ───────────────────────────────────────────────────────────────────

/// Popover background.
pub const BG: Rgba = hex(0xfbfaf8);
/// Primary text.
pub const TEXT: Rgba = hex(0x1a1a1a);
/// Secondary text (summaries, locations).
pub const MUTED: Rgba = hex(0x8a8781);
/// Tertiary text (weekday letters, week numbers, footer hints).
pub const FAINT: Rgba = hex(0xa3a099);
/// Hairline rules and borders.
pub const RULE: Rgba = hex(0xebe8e2);
/// The lighter rule between list rows.
pub const ROW_RULE: Rgba = hex(0xf0ede8);
/// Selected day: filled background.
pub const SELECTED_BG: Rgba = hex(0x1a1a1a);
/// Selected day: text on the fill.
pub const SELECTED_FG: Rgba = hex(0xfbfaf8);
/// Today / next-up accent.
pub const ACCENT: Rgba = hex(0xc2410c);
/// Footer band background.
pub const FOOTER_BG: Rgba = hex(0xf6f4f0);
/// Day/Week segmented-control track.
pub const SEGMENT_TRACK: Rgba = hex(0xefece7);
/// The selected segment's fill.
pub const SEGMENT_ACTIVE: Rgba = hex(0xfbfaf8);
/// Hover wash on a day cell.
pub const HOVER: Rgba = hex(0xefece7);
/// The "–" placeholder in the week list.
pub const PLACEHOLDER: Rgba = hex(0xb5b2ab);
/// Out-of-month day numbers.
pub const DIM: Rgba = hex_a(0x1a1a1a, 0.28);
/// Event dot under a day that has events.
pub const DOT: Rgba = hex(0x8a8781);
/// Dot under the selected day (drawn on the dark fill).
pub const DOT_ON_SELECTED: Rgba = hex_a(0xfbfaf8, 0.7);

/// Opacity applied to today's events that have already ended.
pub const PAST_OPACITY: f32 = 0.4;

// ── Sizes ────────────────────────────────────────────────────────────────────

/// Popover width. Fixed; height grows with content.
pub const POPOVER_WIDTH: Pixels = px(320.);
/// Corner radius of the popover.
pub const POPOVER_RADIUS: Pixels = px(12.);
/// Gap between the menu bar and the top of the popover.
pub const POPOVER_TOP_GAP: Pixels = px(6.);
/// Minimum height of the list area, so the popover does not jump when a day is empty.
pub const LIST_MIN_HEIGHT: Pixels = px(200.);

/// Horizontal padding for the header, list, and footer.
pub const PAD_X: Pixels = px(16.);
/// Horizontal padding for the grid (cells carry their own inset).
pub const GRID_PAD_X: Pixels = px(12.);

/// Height of one day cell in the month grid.
pub const CELL_HEIGHT: Pixels = px(34.);
/// Corner radius of a day cell.
pub const CELL_RADIUS: Pixels = px(7.);
/// Gap between day cells, in both axes.
pub const CELL_GAP: Pixels = px(2.);
/// Width of the ISO week-number gutter on the left of the grid.
pub const WEEK_COLUMN_WIDTH: Pixels = px(20.);
/// Size of the event dot under a day number.
pub const DOT_SIZE: Pixels = px(4.);

/// Width of the time column in the day list.
pub const TIME_COLUMN_WIDTH: Pixels = px(40.);
/// Width of the DOW/day column in the week list.
pub const WEEK_LABEL_WIDTH: Pixels = px(40.);

// ── Type scale ───────────────────────────────────────────────────────────────

/// Month title.
pub const TEXT_TITLE: Pixels = px(15.);
/// Body: event titles, day numbers.
pub const TEXT_BODY: Pixels = px(13.);
/// Secondary: locations, summaries, segment labels, times.
pub const TEXT_SMALL: Pixels = px(12.);
/// Footer hints.
pub const TEXT_TINY: Pixels = px(11.);
/// Weekday letters and week numbers.
pub const TEXT_MICRO: Pixels = px(10.);

/// Monospace family for times and key hints.
pub const MONO_FAMILY: &str = "SF Mono";
/// UI family.
pub const UI_FAMILY: &str = ".SystemUIFont";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_the_mockup_background() {
        let bg = BG;
        assert_eq!((bg.r * 255.0).round() as u32, 0xfb);
        assert_eq!((bg.g * 255.0).round() as u32, 0xfa);
        assert_eq!((bg.b * 255.0).round() as u32, 0xf8);
        assert_eq!(bg.a, 1.0);
    }

    #[test]
    fn accent_is_the_mockup_orange() {
        assert_eq!((ACCENT.r * 255.0).round() as u32, 0xc2);
        assert_eq!((ACCENT.g * 255.0).round() as u32, 0x41);
        assert_eq!((ACCENT.b * 255.0).round() as u32, 0x0c);
    }

    #[test]
    fn popover_is_320_wide() {
        assert_eq!(POPOVER_WIDTH, px(320.));
    }
}
