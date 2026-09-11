//! daybar as a library, so `examples/popover_preview.rs` (and future
//! integration tests) can build the views without the menu-bar binary.
//!
//! NOTE(integrator): `src/main.rs` still declares its own `mod` tree, so these
//! modules compile twice. Once main.rs switches to `use daybar::…` this is the
//! single definition.

#![allow(dead_code)]

pub mod calendar;
pub mod launch_at_login;
pub mod menu_bar_icon;
pub mod model;
pub mod status_item;
pub mod ui;
