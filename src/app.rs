use crate::{pdf::Worker, reader::*};
use gpui::AppContext;
use gpui::{
    App, Application, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds,
    WindowOptions, px, size,
};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

pub(crate) fn run(paths: Vec<PathBuf>) -> anyhow::Result<()> {
    let (worker, events) = Worker::start()?;
    let startup_error = Rc::new(RefCell::new(None));
    let error_slot = startup_error.clone();
    Application::new().run(move |cx: &mut App| {
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.set_menus(vec![
            Menu {
                name: "PDF Reader".into(),
                items: vec![MenuItem::action("Quit PDF Reader", Quit)],
            },
            Menu {
                name: "File".into(),
                items: vec![
                    MenuItem::action("Open PDF…", Open),
                    MenuItem::separator(),
                    MenuItem::action("Close Tab", Close),
                ],
            },
            Menu {
                name: "Edit".into(),
                items: vec![
                    MenuItem::action("Copy", CopyText),
                    MenuItem::action("Select All on Page", SelectAll),
                ],
            },
            Menu {
                name: "View".into(),
                items: vec![
                    MenuItem::action("Previous Page", Previous),
                    MenuItem::action("Next Page", Next),
                    MenuItem::separator(),
                    MenuItem::action("Zoom In", ZoomIn),
                    MenuItem::action("Zoom Out", ZoomOut),
                    MenuItem::action("Fit to Width", Fit),
                    MenuItem::separator(),
                    MenuItem::action("Toggle Sidebar", ToggleSidebar),
                ],
            },
        ]);
        let modifier = if cfg!(target_os = "macos") {
            "cmd"
        } else {
            "ctrl"
        };
        cx.bind_keys([
            KeyBinding::new(&format!("{modifier}-c"), CopyText, Some("Reader")),
            KeyBinding::new(&format!("{modifier}-a"), SelectAll, Some("Reader")),
            KeyBinding::new("escape", ClearSelection, Some("Reader")),
            KeyBinding::new("ctrl-tab", NextTab, Some("Reader")),
            KeyBinding::new("ctrl-shift-tab", PreviousTab, Some("Reader")),
            KeyBinding::new(&format!("{modifier}-o"), Open, Some("Reader")),
            KeyBinding::new("left", Previous, Some("Reader")),
            KeyBinding::new("right", Next, Some("Reader")),
            KeyBinding::new("pageup", Previous, Some("Reader")),
            KeyBinding::new("pagedown", Next, Some("Reader")),
            KeyBinding::new(&format!("{modifier}-="), ZoomIn, Some("Reader")),
            KeyBinding::new(&format!("{modifier}--"), ZoomOut, Some("Reader")),
            KeyBinding::new(&format!("{modifier}-0"), Fit, Some("Reader")),
            KeyBinding::new(&format!("{modifier}-b"), ToggleSidebar, Some("Reader")),
            KeyBinding::new(&format!("{modifier}-w"), Close, Some("Reader")),
            KeyBinding::new(&format!("{modifier}-q"), Quit, None),
        ]);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit()
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(1100.0), px(850.0)), cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(800.0), px(500.0))),
                titlebar: Some(TitlebarOptions {
                    title: Some("PDF Reader".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Reader::new(paths, worker, events, window, cx)),
        );
        if let Err(error) = opened {
            *error_slot.borrow_mut() = Some(error);
            cx.quit();
            return;
        }
        cx.activate(true);
    });
    if let Some(error) = startup_error.take() {
        return Err(error);
    }
    Ok(())
}
