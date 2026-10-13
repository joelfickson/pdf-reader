use gpui::actions;

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
        Quit
    ]
);
