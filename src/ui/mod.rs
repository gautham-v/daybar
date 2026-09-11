//! gpui views for the popover.
//!
//! `popover.rs` is the root view and owns the state (selected day, Day/Week
//! mode, the loaded events); the other modules are leaf views it composes.
//! All colors and sizes come from [`theme`] — no literals in the views.

pub mod day_list;
pub mod month_grid;
pub mod popover;
pub mod theme;
pub mod week_list;
