//! The mockup's SVG icons, compiled into the binary.
//!
//! gpui's `svg()` element asks the app's [`gpui::AssetSource`] for a path and
//! rasterizes the result as an alpha mask, tinted with `.text_color(..)`. The
//! set is fixed and tiny, so the source is a `match` over `include_bytes!`
//! rather than a crate that walks a directory.

use std::borrow::Cow;

use gpui::SharedString;

/// Paths, so callers never spell a string literal.
pub const CHEVRON_LEFT: &str = "icons/chevron-left.svg";
pub const CHEVRON_RIGHT: &str = "icons/chevron-right.svg";
pub const ELLIPSIS: &str = "icons/ellipsis.svg";
pub const PIN: &str = "icons/pin.svg";
pub const PEOPLE: &str = "icons/people.svg";

const FILES: [(&str, &[u8]); 5] = [
    (
        CHEVRON_LEFT,
        include_bytes!("../../assets/icons/chevron-left.svg"),
    ),
    (
        CHEVRON_RIGHT,
        include_bytes!("../../assets/icons/chevron-right.svg"),
    ),
    (ELLIPSIS, include_bytes!("../../assets/icons/ellipsis.svg")),
    (PIN, include_bytes!("../../assets/icons/pin.svg")),
    (PEOPLE, include_bytes!("../../assets/icons/people.svg")),
];

/// The app's asset source. Register with
/// `Application::new().with_assets(Assets)`.
pub struct Assets;

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(FILES
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(FILES
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::AssetSource;

    #[test]
    fn every_icon_loads_and_is_an_svg() {
        for (name, _) in FILES {
            let bytes = Assets.load(name).unwrap().expect(name);
            assert!(std::str::from_utf8(&bytes).unwrap().starts_with("<svg"));
        }
    }

    #[test]
    fn an_unknown_path_is_none_not_an_error() {
        assert!(Assets.load("icons/nope.svg").unwrap().is_none());
        assert_eq!(Assets.list("icons/").unwrap().len(), FILES.len());
    }
}
