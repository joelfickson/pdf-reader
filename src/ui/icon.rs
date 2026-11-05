use gpui::{AssetSource, SharedString};
use std::borrow::Cow;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Icon {
    Attach,
    Bookmark,
    Check,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    Close,
    Copy,
    Cursor,
    Draw,
    Export,
    Extract,
    File,
    Fit,
    Goto,
    Grid,
    Highlight,
    Image,
    Inspector,
    Keyboard,
    Layout,
    List,
    Lookup,
    Magnifier,
    Minus,
    Moon,
    Note,
    Outline,
    Pages,
    Pen,
    Plus,
    Present,
    Print,
    Rect,
    Redo,
    RotateLeft,
    RotateRight,
    Save,
    Search,
    Settings,
    Sidebar,
    Sign,
    Split,
    Stamp,
    Strike,
    Text,
    Trash,
    Underline,
    Undo,
    Upload,
}

impl Icon {
    pub fn path(self) -> &'static str {
        match self {
            Self::Attach => "icons/attach.svg",
            Self::Bookmark => "icons/bookmark.svg",
            Self::Check => "icons/check.svg",
            Self::ChevronDown => "icons/chevron-down.svg",
            Self::ChevronLeft => "icons/chevron-left.svg",
            Self::ChevronRight => "icons/chevron-right.svg",
            Self::ChevronUp => "icons/chevron-up.svg",
            Self::Close => "icons/close.svg",
            Self::Copy => "icons/copy.svg",
            Self::Cursor => "icons/cursor.svg",
            Self::Draw => "icons/draw.svg",
            Self::Export => "icons/export.svg",
            Self::Extract => "icons/extract.svg",
            Self::File => "icons/file.svg",
            Self::Fit => "icons/fit.svg",
            Self::Goto => "icons/goto.svg",
            Self::Grid => "icons/grid.svg",
            Self::Highlight => "icons/highlight.svg",
            Self::Image => "icons/image.svg",
            Self::Inspector => "icons/inspector.svg",
            Self::Keyboard => "icons/keyboard.svg",
            Self::Layout => "icons/layout.svg",
            Self::List => "icons/list.svg",
            Self::Lookup => "icons/lookup.svg",
            Self::Magnifier => "icons/magnifier.svg",
            Self::Minus => "icons/minus.svg",
            Self::Moon => "icons/moon.svg",
            Self::Note => "icons/note.svg",
            Self::Outline => "icons/outline.svg",
            Self::Pages => "icons/pages.svg",
            Self::Pen => "icons/pen.svg",
            Self::Plus => "icons/plus.svg",
            Self::Present => "icons/present.svg",
            Self::Print => "icons/print.svg",
            Self::Rect => "icons/rect.svg",
            Self::Redo => "icons/redo.svg",
            Self::RotateLeft => "icons/rotate-left.svg",
            Self::RotateRight => "icons/rotate-right.svg",
            Self::Save => "icons/save.svg",
            Self::Search => "icons/search.svg",
            Self::Settings => "icons/settings.svg",
            Self::Sidebar => "icons/sidebar.svg",
            Self::Sign => "icons/sign.svg",
            Self::Split => "icons/split.svg",
            Self::Stamp => "icons/stamp.svg",
            Self::Strike => "icons/strike.svg",
            Self::Text => "icons/text.svg",
            Self::Trash => "icons/trash.svg",
            Self::Underline => "icons/underline.svg",
            Self::Undo => "icons/undo.svg",
            Self::Upload => "icons/upload.svg",
        }
    }
}

const ICONS: &[(&str, &[u8])] = &[
    (
        "icons/attach.svg",
        include_bytes!("../../assets/icons/attach.svg"),
    ),
    (
        "icons/bookmark.svg",
        include_bytes!("../../assets/icons/bookmark.svg"),
    ),
    (
        "icons/check.svg",
        include_bytes!("../../assets/icons/check.svg"),
    ),
    (
        "icons/chevron-down.svg",
        include_bytes!("../../assets/icons/chevron-down.svg"),
    ),
    (
        "icons/chevron-left.svg",
        include_bytes!("../../assets/icons/chevron-left.svg"),
    ),
    (
        "icons/chevron-right.svg",
        include_bytes!("../../assets/icons/chevron-right.svg"),
    ),
    (
        "icons/chevron-up.svg",
        include_bytes!("../../assets/icons/chevron-up.svg"),
    ),
    (
        "icons/close.svg",
        include_bytes!("../../assets/icons/close.svg"),
    ),
    (
        "icons/copy.svg",
        include_bytes!("../../assets/icons/copy.svg"),
    ),
    (
        "icons/cursor.svg",
        include_bytes!("../../assets/icons/cursor.svg"),
    ),
    (
        "icons/draw.svg",
        include_bytes!("../../assets/icons/draw.svg"),
    ),
    (
        "icons/export.svg",
        include_bytes!("../../assets/icons/export.svg"),
    ),
    (
        "icons/extract.svg",
        include_bytes!("../../assets/icons/extract.svg"),
    ),
    (
        "icons/file.svg",
        include_bytes!("../../assets/icons/file.svg"),
    ),
    (
        "icons/fit.svg",
        include_bytes!("../../assets/icons/fit.svg"),
    ),
    (
        "icons/goto.svg",
        include_bytes!("../../assets/icons/goto.svg"),
    ),
    (
        "icons/grid.svg",
        include_bytes!("../../assets/icons/grid.svg"),
    ),
    (
        "icons/highlight.svg",
        include_bytes!("../../assets/icons/highlight.svg"),
    ),
    (
        "icons/image.svg",
        include_bytes!("../../assets/icons/image.svg"),
    ),
    (
        "icons/inspector.svg",
        include_bytes!("../../assets/icons/inspector.svg"),
    ),
    (
        "icons/keyboard.svg",
        include_bytes!("../../assets/icons/keyboard.svg"),
    ),
    (
        "icons/layout.svg",
        include_bytes!("../../assets/icons/layout.svg"),
    ),
    (
        "icons/list.svg",
        include_bytes!("../../assets/icons/list.svg"),
    ),
    (
        "icons/lookup.svg",
        include_bytes!("../../assets/icons/lookup.svg"),
    ),
    (
        "icons/magnifier.svg",
        include_bytes!("../../assets/icons/magnifier.svg"),
    ),
    (
        "icons/minus.svg",
        include_bytes!("../../assets/icons/minus.svg"),
    ),
    (
        "icons/moon.svg",
        include_bytes!("../../assets/icons/moon.svg"),
    ),
    (
        "icons/note.svg",
        include_bytes!("../../assets/icons/note.svg"),
    ),
    (
        "icons/outline.svg",
        include_bytes!("../../assets/icons/outline.svg"),
    ),
    (
        "icons/pages.svg",
        include_bytes!("../../assets/icons/pages.svg"),
    ),
    (
        "icons/pen.svg",
        include_bytes!("../../assets/icons/pen.svg"),
    ),
    (
        "icons/plus.svg",
        include_bytes!("../../assets/icons/plus.svg"),
    ),
    (
        "icons/present.svg",
        include_bytes!("../../assets/icons/present.svg"),
    ),
    (
        "icons/print.svg",
        include_bytes!("../../assets/icons/print.svg"),
    ),
    (
        "icons/rect.svg",
        include_bytes!("../../assets/icons/rect.svg"),
    ),
    (
        "icons/redo.svg",
        include_bytes!("../../assets/icons/redo.svg"),
    ),
    (
        "icons/rotate-left.svg",
        include_bytes!("../../assets/icons/rotate-left.svg"),
    ),
    (
        "icons/rotate-right.svg",
        include_bytes!("../../assets/icons/rotate-right.svg"),
    ),
    (
        "icons/save.svg",
        include_bytes!("../../assets/icons/save.svg"),
    ),
    (
        "icons/search.svg",
        include_bytes!("../../assets/icons/search.svg"),
    ),
    (
        "icons/settings.svg",
        include_bytes!("../../assets/icons/settings.svg"),
    ),
    (
        "icons/sidebar.svg",
        include_bytes!("../../assets/icons/sidebar.svg"),
    ),
    (
        "icons/sign.svg",
        include_bytes!("../../assets/icons/sign.svg"),
    ),
    (
        "icons/split.svg",
        include_bytes!("../../assets/icons/split.svg"),
    ),
    (
        "icons/stamp.svg",
        include_bytes!("../../assets/icons/stamp.svg"),
    ),
    (
        "icons/strike.svg",
        include_bytes!("../../assets/icons/strike.svg"),
    ),
    (
        "icons/text.svg",
        include_bytes!("../../assets/icons/text.svg"),
    ),
    (
        "icons/trash.svg",
        include_bytes!("../../assets/icons/trash.svg"),
    ),
    (
        "icons/underline.svg",
        include_bytes!("../../assets/icons/underline.svg"),
    ),
    (
        "icons/undo.svg",
        include_bytes!("../../assets/icons/undo.svg"),
    ),
    (
        "icons/upload.svg",
        include_bytes!("../../assets/icons/upload.svg"),
    ),
];

pub(crate) struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_path_resolves_to_embedded_svg() {
        for (name, _) in ICONS {
            let bytes = Assets.load(name).unwrap().unwrap();
            assert!(std::str::from_utf8(&bytes).unwrap().starts_with("<svg"));
        }
        assert!(Assets.load(Icon::Close.path()).unwrap().is_some());
        assert!(Assets.load("icons/missing.svg").unwrap().is_none());
    }
}
