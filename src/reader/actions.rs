use gpui::{Action, actions};
use std::path::PathBuf;

actions!(
    reader,
    [
        Open,
        NextTab,
        PreviousTab,
        CopyText,
        SelectAll,
        ClearSelection,
        Previous,
        Next,
        ZoomIn,
        ZoomOut,
        Fit,
        ToggleSidebar,
        Close,
        Quit,
        TogglePalette,
        ToggleFind,
        ToggleAnnotate,
        ToggleOrganize,
        ToggleInspector,
        ShowExport,
        ShowPrint,
        ShowSettings,
        ShowShortcuts,
        GoToPage,
        Dismiss,
        Save,
        Bookmark,
        ToggleSplit,
        TogglePresent,
        FirstPage,
        LastPage,
        ClearRecent
    ]
);

#[derive(Clone, PartialEq, Debug, Action)]
#[action(namespace = reader, no_json)]
pub(crate) struct OpenRecent {
    pub path: PathBuf,
}
