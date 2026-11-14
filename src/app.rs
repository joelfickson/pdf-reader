use crate::{pdf::Worker, reader::*, recent::RecentFile, ui::icon::Assets};
use gpui::AppContext;
use gpui::{
    App, Application, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds,
    WindowOptions, point, px, size,
};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

const RECENT_MENU_ITEMS: usize = 10;

fn decode_percent(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

pub(crate) fn paths_from_urls(urls: &[String]) -> Vec<PathBuf> {
    urls.iter()
        .filter_map(|url| {
            let rest = url.strip_prefix("file://")?;
            let path = rest.strip_prefix("localhost").unwrap_or(rest);
            path.starts_with('/')
                .then(|| decode_percent(path))
                .flatten()
                .map(PathBuf::from)
        })
        .collect()
}

pub(crate) fn recent_title(path: &std::path::Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

const READER: &str = "Reader";
const READER_NOT_TYPING: &str = "Reader && !TextInput";

pub(crate) fn menus(recent: &[RecentFile]) -> Vec<Menu> {
    let mut recent_items: Vec<MenuItem> = recent
        .iter()
        .take(RECENT_MENU_ITEMS)
        .map(|entry| {
            MenuItem::action(
                recent_title(&entry.path),
                OpenRecent {
                    path: entry.path.clone(),
                },
            )
        })
        .collect();
    if !recent_items.is_empty() {
        recent_items.push(MenuItem::separator());
    }
    recent_items.push(MenuItem::action("Clear Menu", ClearRecent));
    vec![
        Menu {
            name: "Agentic PDF".into(),
            items: vec![
                MenuItem::action("Check for Updates…", CheckForUpdates),
                MenuItem::separator(),
                MenuItem::action("Settings…", ShowSettings),
                MenuItem::separator(),
                MenuItem::action("Quit Agentic PDF", Quit),
            ],
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("Open PDF…", Open),
                MenuItem::submenu(Menu {
                    name: "Open Recent".into(),
                    items: recent_items,
                }),
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
                MenuItem::action("Undo", Undo),
                MenuItem::action("Redo", Redo),
                MenuItem::separator(),
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
                MenuItem::action("Back", GoBack),
                MenuItem::action("Forward", GoForward),
                MenuItem::separator(),
                MenuItem::action("Zoom In", ZoomIn),
                MenuItem::action("Zoom Out", ZoomOut),
                MenuItem::action("Fit to Width", Fit),
                MenuItem::action("Fit Page", FitPage),
                MenuItem::action("Actual Size", ActualSize),
                MenuItem::separator(),
                MenuItem::action("Toggle Sidebar", ToggleSidebar),
                MenuItem::action("Toggle Inspector", ToggleInspector),
                MenuItem::action("Agent", ToggleAgent),
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
    let (back, forward) = if cfg!(target_os = "macos") {
        ("cmd-[", "cmd-]")
    } else {
        ("alt-left", "alt-right")
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
        KeyBinding::new(&format!("{m}-home"), FirstPage, any),
        KeyBinding::new(&format!("{m}-end"), LastPage, any),
        KeyBinding::new(back, GoBack, any),
        KeyBinding::new(forward, GoForward, any),
        KeyBinding::new(&format!("{m}-="), ZoomIn, any),
        KeyBinding::new(&format!("{m}--"), ZoomOut, any),
        KeyBinding::new(&format!("{m}-0"), Fit, any),
        KeyBinding::new(&format!("{m}-9"), FitPage, any),
        KeyBinding::new(&format!("{m}-1"), ActualSize, any),
        KeyBinding::new(&format!("{m}-b"), ToggleSidebar, any),
        KeyBinding::new(&format!("{m}-w"), Close, any),
        KeyBinding::new(&format!("{m}-k"), TogglePalette, any),
        KeyBinding::new(&format!("{m}-f"), ToggleFind, any),
        KeyBinding::new(&format!("{m}-g"), GoToPage, any),
        KeyBinding::new(&format!("{m}-i"), ToggleInspector, any),
        KeyBinding::new(&format!("{m}-j"), ToggleAgent, any),
        KeyBinding::new(&format!("{m}-e"), ShowExport, any),
        KeyBinding::new(&format!("{m}-p"), ShowPrint, any),
        KeyBinding::new(&format!("{m}-s"), Save, any),
        KeyBinding::new(&format!("{m}-z"), Undo, reading),
        KeyBinding::new(&format!("{m}-shift-z"), Redo, reading),
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
    let (open_sender, opened_files) = async_channel::unbounded();
    let application = Application::new().with_assets(Assets);
    application.on_open_urls(move |urls| {
        let paths = paths_from_urls(&urls);
        if !paths.is_empty() {
            let _ = open_sender.try_send(paths);
        }
    });
    application.run(move |cx: &mut App| {
        crate::bench::mark("app_ready");
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.set_menus(menus(&[]));
        cx.bind_keys(bindings());
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit()
            }
        })
        .detach();
        let mut bounds = Bounds::centered(None, size(px(1180.0), px(860.0)), cx);
        if cfg!(debug_assertions) && std::env::var_os("AGENTICPDF_PREVIEW").is_some() {
            bounds.origin = point(px(24.0), px(48.0));
        }
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(820.0), px(520.0))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Agentic PDF".into()),
                    appears_transparent: cfg!(target_os = "macos"),
                    traffic_light_position: cfg!(target_os = "macos")
                        .then(|| point(px(14.0), px(14.0))),
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Reader::new(paths, worker, events, window, cx)),
        );
        crate::bench::mark("window_opened");
        let window = match opened {
            Ok(window) => window,
            Err(error) => {
                *error_slot.borrow_mut() = Some(error);
                cx.quit();
                return;
            }
        };
        cx.spawn(async move |cx| {
            while let Ok(paths) = opened_files.recv().await {
                let delivered = window.update(cx, |reader, window, cx| {
                    reader.open_paths(paths, window, cx);
                    window.activate_window();
                });
                if delivered.is_err() {
                    return;
                }
            }
        })
        .detach();
        cx.activate(true);
    });
    if let Some(error) = startup_error.take() {
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_urls_decode_to_paths_and_other_urls_are_ignored() {
        let urls = [
            "file:///Users/me/Documents/Office%20Lease.pdf".to_string(),
            "file://localhost/tmp/caf%C3%A9.pdf".to_string(),
            "https://example.com/a.pdf".to_string(),
            "file:///broken%2".to_string(),
            "file:///bad%ZZ.pdf".to_string(),
            "file://server/share.pdf".to_string(),
        ];
        assert_eq!(
            paths_from_urls(&urls),
            [
                PathBuf::from("/Users/me/Documents/Office Lease.pdf"),
                PathBuf::from("/tmp/café.pdf"),
            ]
        );
    }

    #[test]
    fn recent_menu_lists_files_then_clear() {
        let recent: Vec<RecentFile> = (0..12)
            .map(|index| RecentFile {
                path: PathBuf::from(format!("/docs/{index}.pdf")),
                page: 0,
                zoom: None,
            })
            .collect();
        let file_menu = |recent: &[RecentFile]| {
            let menus = menus(recent);
            let file = menus.into_iter().find(|menu| menu.name == "File").unwrap();
            file.items
                .into_iter()
                .find_map(|item| match item {
                    MenuItem::Submenu(menu) if menu.name == "Open Recent" => Some(menu.items.len()),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(file_menu(&recent), RECENT_MENU_ITEMS + 2);
        assert_eq!(file_menu(&[]), 1);
    }
}
