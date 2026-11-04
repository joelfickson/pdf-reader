use crate::{pdf::Worker, reader::*, ui::icon::Assets};
use gpui::AppContext;
use gpui::{
    App, Application, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds,
    WindowOptions, point, px, size,
};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

const READER: &str = "Reader";
const READER_NOT_TYPING: &str = "Reader && !TextInput";

fn menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "PDF Reader".into(),
            items: vec![
                MenuItem::action("Settings…", ShowSettings),
                MenuItem::separator(),
                MenuItem::action("Quit PDF Reader", Quit),
            ],
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("Open PDF…", Open),
                MenuItem::action("Save", Save),
                MenuItem::separator(),
                MenuItem::action("Export…", ShowExport),
                MenuItem::action("Print…", ShowPrint),
                MenuItem::separator(),
                MenuItem::action("Close Tab", Close),
            ],
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::action("Copy", CopyText),
                MenuItem::action("Select All on Page", SelectAll),
                MenuItem::separator(),
                MenuItem::action("Find", ToggleFind),
                MenuItem::action("Annotate", ToggleAnnotate),
                MenuItem::action("Organize Pages", ToggleOrganize),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Command Palette", TogglePalette),
                MenuItem::separator(),
                MenuItem::action("Previous Page", Previous),
                MenuItem::action("Next Page", Next),
                MenuItem::action("Go to Page…", GoToPage),
                MenuItem::separator(),
                MenuItem::action("Zoom In", ZoomIn),
                MenuItem::action("Zoom Out", ZoomOut),
                MenuItem::action("Fit to Width", Fit),
                MenuItem::separator(),
                MenuItem::action("Toggle Sidebar", ToggleSidebar),
                MenuItem::action("Toggle Inspector", ToggleInspector),
                MenuItem::action("Keyboard Shortcuts", ShowShortcuts),
            ],
        },
    ]
}

pub(crate) fn bindings() -> Vec<KeyBinding> {
    let m = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    let any = Some(READER);
    let reading = Some(READER_NOT_TYPING);
    vec![
        KeyBinding::new(&format!("{m}-c"), CopyText, any),
        KeyBinding::new(&format!("{m}-a"), SelectAll, reading),
        KeyBinding::new("escape", Dismiss, any),
        KeyBinding::new("ctrl-tab", NextTab, any),
        KeyBinding::new("ctrl-shift-tab", PreviousTab, any),
        KeyBinding::new(&format!("{m}-o"), Open, any),
        KeyBinding::new("left", Previous, reading),
        KeyBinding::new("right", Next, reading),
        KeyBinding::new("pageup", Previous, reading),
        KeyBinding::new("pagedown", Next, reading),
        KeyBinding::new("home", FirstPage, reading),
        KeyBinding::new("end", LastPage, reading),
        KeyBinding::new(&format!("{m}-="), ZoomIn, any),
        KeyBinding::new(&format!("{m}--"), ZoomOut, any),
        KeyBinding::new(&format!("{m}-0"), Fit, any),
        KeyBinding::new(&format!("{m}-b"), ToggleSidebar, any),
        KeyBinding::new(&format!("{m}-w"), Close, any),
        KeyBinding::new(&format!("{m}-k"), TogglePalette, any),
        KeyBinding::new(&format!("{m}-f"), ToggleFind, any),
        KeyBinding::new(&format!("{m}-g"), GoToPage, any),
        KeyBinding::new(&format!("{m}-i"), ToggleInspector, any),
        KeyBinding::new(&format!("{m}-e"), ShowExport, any),
        KeyBinding::new(&format!("{m}-p"), ShowPrint, any),
        KeyBinding::new(&format!("{m}-s"), Save, any),
        KeyBinding::new(&format!("{m}-d"), Bookmark, any),
        KeyBinding::new(&format!("{m}-,"), ShowSettings, any),
        KeyBinding::new(&format!("{m}-\\"), ToggleSplit, any),
        KeyBinding::new(&format!("{m}-shift-a"), ToggleAnnotate, any),
        KeyBinding::new(&format!("{m}-shift-o"), ToggleOrganize, any),
        KeyBinding::new(&format!("{m}-shift-p"), TogglePresent, any),
        KeyBinding::new(&format!("{m}-q"), Quit, None),
    ]
}

pub(crate) fn run(paths: Vec<PathBuf>) -> anyhow::Result<()> {
    let (worker, events) = Worker::start()?;
    let startup_error = Rc::new(RefCell::new(None));
    let error_slot = startup_error.clone();
    Application::new()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.set_menus(menus());
            cx.bind_keys(bindings());
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit()
                }
            })
            .detach();
            let bounds = Bounds::centered(None, size(px(1180.0), px(860.0)), cx);
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(820.0), px(520.0))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("PDF Reader".into()),
                        appears_transparent: cfg!(target_os = "macos"),
                        traffic_light_position: cfg!(target_os = "macos")
                            .then(|| point(px(14.0), px(14.0))),
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
