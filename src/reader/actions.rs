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
        CheckForUpdates,
        ShowShortcuts,
        GoToPage,
        Dismiss,
        Save,
        Bookmark,
        ToggleSplit,
        TogglePresent,
        FirstPage,
        LastPage,
        ClearRecent,
        GoBack,
        GoForward,
        ToggleAgent,
        Undo,
        Redo,
        FitPage,
        ActualSize
    ]
);

#[derive(Clone, PartialEq, Debug, Action)]
#[action(namespace = reader, no_json)]
pub(crate) struct OpenRecent {
    pub path: PathBuf,
}
