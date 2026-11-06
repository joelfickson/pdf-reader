use super::*;
use crate::pdf::{
    Raster,
    text::{Glyph, PageText, TextRect},
};
use gpui::{ExternalPaths, FileDropEvent, MouseButton, TestAppContext, point, px};

#[gpui::test]
fn drop_selection_clipboard_and_tabs(cx: &mut TestAppContext) {
    let (_sender, events) = async_channel::bounded(2);
    let (reader, cx) = cx
        .add_window_view(|window, cx| Reader::new(Vec::new(), Worker::idle(), events, window, cx));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            reader.notice = Some("Drop handler has not run".into());
            cx.notify();
            reader.focus.focus(window);
        })
    });
    cx.run_until_parked();
    let position = point(px(400.0), px(300.0));
    cx.simulate_event(FileDropEvent::Entered {
        position,
        paths: ExternalPaths::default(),
    });
    cx.simulate_event(FileDropEvent::Submit { position });
    cx.read(|cx| assert!(reader.read(cx).notice.is_none()));
    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            reader.open_paths(
                vec![
                    root.join("tests/fixtures/reading.pdf"),
                    root.join("tests/fixtures/selection.pdf"),
                    root.join("README.md"),
                ],
                window,
                cx,
            );
            assert_eq!(reader.tabs.entries.len(), 2);
            assert_eq!(reader.tabs.active, 1);
            assert!(reader.notice.is_some());
            let generation = reader.generation;
            let view = reader.last_request.as_ref().unwrap().0.clone();
            reader.receive(
                Event::Metadata {
                    generation,
                    count: 3,
                },
                window,
                cx,
            );
            reader.receive(
                Event::Page {
                    generation,
                    view: view.clone(),
                    raster: Raster {
                        bytes: vec![255; 400 * 1200 * 4],
                        width: 400,
                        height: 1200,
                        logical_width: 400.0,
                        logical_height: 1200.0,
                        zoom_percent: 100.0,
                    },
                },
                window,
                cx,
            );
            reader.receive(
                Event::Text {
                    generation,
                    key: (view.session, 0),
                    text: Ok(Arc::new(PageText {
                        glyphs: "Café"
                            .chars()
                            .enumerate()
                            .map(|(index, value)| Glyph {
                                value,
                                bounds: Some(TextRect {
                                    left: 0.1 + index as f32 * 0.03,
                                    right: 0.13 + index as f32 * 0.03,
                                    top: 0.1,
                                    bottom: 0.12,
                                }),
                                direction: [1.0, 0.0],
                            })
                            .collect(),
                    })),
                },
                window,
                cx,
            );
        })
    });
    cx.run_until_parked();
    let bounds = cx.read(|cx| reader.read(cx).page_bounds.get());
    assert!(bounds.size.width > px(0.0));
    let start = bounds.origin + point(bounds.size.width * 0.101, bounds.size.height * 0.11);
    let end = bounds.origin + point(bounds.size.width * 0.219, bounds.size.height * 0.11);
    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(end, MouseButton::Left, Default::default());
    cx.simulate_mouse_up(end, MouseButton::Left, Default::default());
    cx.dispatch_action(CopyText);
    assert_eq!(
        cx.read_from_clipboard().and_then(|item| item.text()),
        Some("Café".into())
    );
    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            reader
                .tabs
                .current_mut()
                .unwrap()
                .scroll
                .set_offset(point(px(0.0), px(-100.0)));
            reader.activate_tab(0, window, cx);
            assert!(reader.selection.is_none());
        })
    });
    cx.run_until_parked();
    cx.dispatch_action(NextTab);
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert_eq!(reader.tabs.active, 1);
        assert_eq!(reader.tabs.current().unwrap().scroll.offset().y, px(-100.0));
        assert!(reader.image.is_some());
        assert!(!reader.busy);
    });
    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            reader.open_paths(vec![root.join("tests/fixtures/selection.pdf")], window, cx);
            assert_eq!(reader.tabs.entries.len(), 2);
            reader.activate_tab(0, window, cx);
            let session = reader.tabs.entries[1].session;
            for image in reader.pages.remove_where(|key| key.session == session) {
                image.release(window);
            }
            reader.activate_tab(1, window, cx);
            assert!(reader.busy);
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            let generation = reader.generation;
            let view = reader.last_request.as_ref().unwrap().0.clone();
            reader.receive(
                Event::Error {
                    generation: generation - 1,
                    message: "Stale error".into(),
                },
                window,
                cx,
            );
            assert!(reader.tabs.current().unwrap().error.is_none());
            reader.receive(
                Event::Page {
                    generation,
                    view,
                    raster: Raster {
                        bytes: vec![255; 400 * 1200 * 4],
                        width: 400,
                        height: 1200,
                        logical_width: 400.0,
                        logical_height: 1200.0,
                        zoom_percent: 100.0,
                    },
                },
                window,
                cx,
            );
        })
    });
    cx.run_until_parked();
    cx.read(|cx| {
        assert_eq!(
            reader.read(cx).tabs.current().unwrap().scroll.offset().y,
            px(-100.0)
        )
    });
    cx.dispatch_action(Close);
    cx.dispatch_action(Close);
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert!(reader.tabs.entries.is_empty());
        assert!(reader.image.is_none());
        assert!(reader.text.is_none());
    });
}

#[gpui::test]
fn palette_find_and_dismiss_keyboard_flow(cx: &mut TestAppContext) {
    cx.update(|cx| cx.bind_keys(crate::app::bindings()));
    let (_sender, events) = async_channel::bounded(2);
    let (reader, cx) = cx
        .add_window_view(|window, cx| Reader::new(Vec::new(), Worker::idle(), events, window, cx));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            reader.open_paths(vec![root.join("tests/fixtures/reading.pdf")], window, cx);
            let generation = reader.generation;
            reader.receive(
                Event::Metadata {
                    generation,
                    count: 5,
                },
                window,
                cx,
            );
            reader.focus.focus(window);
        })
    });
    cx.run_until_parked();
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    let page = |cx: &mut gpui::VisualTestContext| {
        cx.read(|cx| reader.read(cx).tabs.current().unwrap().page)
    };

    cx.simulate_keystrokes(&format!("{modifier}-k 3->3 enter"));
    cx.read(|cx| assert_eq!(reader.read(cx).chrome.overlay, chrome::Overlay::None));
    assert_eq!(page(cx), 2);

    cx.simulate_keystrokes(&format!("{modifier}-f a->a left"));
    assert_eq!(page(cx), 2);
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert!(reader.chrome.find_open);
        assert_eq!(reader.chrome.find.text, "a");
    });

    cx.simulate_keystrokes("escape left");
    cx.read(|cx| assert!(!reader.read(cx).chrome.find_open));
    assert_eq!(page(cx), 1);

    cx.simulate_keystrokes(&format!("{modifier}-g 5->5 enter"));
    assert_eq!(page(cx), 4);

    cx.simulate_keystrokes(&format!("{modifier}-shift-a"));
    cx.read(|cx| assert_eq!(reader.read(cx).chrome.mode, chrome::Mode::Annotate));
    cx.simulate_keystrokes("escape");
    cx.read(|cx| assert_eq!(reader.read(cx).chrome.mode, chrome::Mode::Read));

    cx.simulate_keystrokes(&format!("{modifier}-d"));
    cx.read(|cx| {
        let toast = reader.read(cx).chrome.toast.clone().unwrap();
        assert_eq!(
            toast.title.as_ref(),
            "Bookmarking pages is not available yet"
        );
    });
}

#[gpui::test]
fn find_searches_steps_between_matches_and_clears(cx: &mut TestAppContext) {
    cx.update(|cx| cx.bind_keys(crate::app::bindings()));
    let (_sender, events) = async_channel::bounded(2);
    let (reader, cx) = cx
        .add_window_view(|window, cx| Reader::new(Vec::new(), Worker::idle(), events, window, cx));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            reader.open_paths(vec![root.join("tests/fixtures/reading.pdf")], window, cx);
            let generation = reader.generation;
            reader.receive(
                Event::Metadata {
                    generation,
                    count: 5,
                },
                window,
                cx,
            );
            reader.focus.focus(window);
        })
    });
    cx.run_until_parked();
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    let state = |cx: &mut gpui::VisualTestContext| {
        cx.read(|cx| {
            let reader = reader.read(cx);
            (
                reader.tabs.current().unwrap().page,
                reader.search.as_ref().map(|search| search.current),
                reader
                    .search
                    .as_ref()
                    .map(|search| search.summary())
                    .unwrap_or_default(),
            )
        })
    };
    let hit = |start: usize| crate::pdf::search::Hit {
        glyphs: start..start + 4,
        before: "own ".into(),
        matched: "pace".into(),
        after: ".".into(),
    };

    cx.simulate_keystrokes(&format!("{modifier}-f p a c e"));
    cx.read(|cx| assert!(reader.read(cx).search.is_none()));
    cx.executor()
        .advance_clock(search::SEARCH_DELAY + std::time::Duration::from_millis(10));
    cx.run_until_parked();
    let id = cx.read(|cx| {
        let search = reader.read(cx).search.as_ref().unwrap();
        assert_eq!(search.query.text, "pace");
        assert!(!search.query.match_case);
        search.id
    });
    assert_eq!(state(cx).2, "Searching");

    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            reader.receive(
                Event::SearchBatch {
                    search: id + 99,
                    scanned: 5,
                    pages: vec![(4, vec![hit(0)])],
                },
                window,
                cx,
            );
            reader.receive(
                Event::SearchBatch {
                    search: id,
                    scanned: 4,
                    pages: vec![(1, vec![hit(10)]), (3, vec![hit(5), hit(30)])],
                },
                window,
                cx,
            );
        })
    });
    assert_eq!(state(cx), (1, Some(Some(0)), "1 of 3".into()));

    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            reader.receive(
                Event::SearchDone {
                    search: id,
                    skipped: 0,
                    truncated: false,
                },
                window,
                cx,
            )
        })
    });
    cx.simulate_keystrokes("enter");
    assert_eq!(state(cx), (3, Some(Some(1)), "2 of 3".into()));
    cx.simulate_keystrokes("down");
    assert_eq!(state(cx), (3, Some(Some(2)), "3 of 3".into()));
    cx.simulate_keystrokes("enter");
    assert_eq!(state(cx), (1, Some(Some(0)), "1 of 3".into()));
    cx.simulate_keystrokes("shift-enter");
    assert_eq!(state(cx), (3, Some(Some(2)), "3 of 3".into()));
    cx.simulate_keystrokes("up");
    assert_eq!(state(cx), (3, Some(Some(1)), "2 of 3".into()));

    cx.update(|_, cx| reader.update(cx, |reader, cx| reader.toggle_find_option(false, cx)));
    cx.read(|cx| {
        let search = reader.read(cx).search.as_ref().unwrap();
        assert!(search.id > id);
        assert!(search.query.match_case);
        assert!(search.hits.is_empty());
    });

    cx.simulate_keystrokes("escape");
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert!(!reader.chrome.find_open);
        assert!(reader.search.is_none());
    });
}

fn load_document(
    reader: &mut Reader,
    window: &mut gpui::Window,
    cx: &mut gpui::Context<Reader>,
    count: usize,
) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    reader.open_paths(vec![root.join("tests/fixtures/reading.pdf")], window, cx);
    let generation = reader.generation;
    let view = reader.last_request.as_ref().unwrap().0.clone();
    reader.receive(Event::Metadata { generation, count }, window, cx);
    reader.receive(
        Event::Page {
            generation,
            view: view.clone(),
            raster: Raster {
                bytes: vec![255; 400 * 520 * 4],
                width: 400,
                height: 520,
                logical_width: 400.0,
                logical_height: 520.0,
                zoom_percent: 100.0,
            },
        },
        window,
        cx,
    );
    reader.receive(
        Event::Text {
            generation,
            key: (view.session, 0),
            text: Ok(Arc::new(PageText {
                glyphs: "Selected words on the page"
                    .chars()
                    .enumerate()
                    .map(|(index, value)| Glyph {
                        value,
                        bounds: Some(TextRect {
                            left: 0.1 + index as f32 * 0.02,
                            right: 0.12 + index as f32 * 0.02,
                            top: 0.2,
                            bottom: 0.23,
                        }),
                        direction: [1.0, 0.0],
                    })
                    .collect(),
            })),
        },
        window,
        cx,
    );
}

fn sweep_mouse(cx: &mut gpui::VisualTestContext) {
    let size = cx.update(|window, _| window.viewport_size());
    let mut y = px(4.0);
    while y < size.height {
        let mut x = px(4.0);
        while x < size.width {
            cx.simulate_mouse_move(point(x, y), None, Default::default());
            x += px(48.0);
        }
        y += px(36.0);
    }
}

#[gpui::test]
fn every_screen_renders_and_survives_hover_in_both_themes(cx: &mut TestAppContext) {
    use crate::ui::theme::ThemeChoice;
    use chrome::{InspectorTab, Mode, Overlay, Panel, Tool};

    let (_sender, events) = async_channel::bounded(2);
    let (reader, cx) = cx
        .add_window_view(|window, cx| Reader::new(Vec::new(), Worker::idle(), events, window, cx));
    cx.run_until_parked();
    sweep_mouse(cx);

    type Setup = fn(&mut Reader, &mut gpui::Window, &mut gpui::Context<Reader>);
    let screens: Vec<(&str, Setup)> = vec![
        ("reading", |_, _, _| {}),
        ("selection", |r, _, cx| {
            r.selection = Some(selection::Selection {
                anchor: 0,
                head: 8,
                dragging: false,
            });
            cx.notify();
        }),
        ("palette", |r, _, cx| r.set_overlay(Overlay::Palette, cx)),
        ("palette query", |r, _, cx| {
            r.set_overlay(Overlay::Palette, cx);
            r.chrome.palette.insert("2");
        }),
        ("palette empty result", |r, _, cx| {
            r.set_overlay(Overlay::Palette, cx);
            r.chrome.palette.insert("zzzz");
        }),
        ("find", |r, _, cx| {
            r.open_find(cx);
            r.chrome.find.insert("words");
        }),
        ("page input", |r, _, cx| r.start_page_input(cx)),
        ("zoom menu", |r, _, cx| r.set_overlay(Overlay::ZoomMenu, cx)),
        ("view menu", |r, _, cx| r.set_overlay(Overlay::ViewMenu, cx)),
        ("annotate", |r, w, cx| r.set_mode(Mode::Annotate, w, cx)),
        ("stamps", |r, w, cx| {
            r.set_mode(Mode::Annotate, w, cx);
            r.chrome.tool = Tool::Stamp;
            r.set_overlay(Overlay::Stamps, cx);
        }),
        ("annotate with find", |r, w, cx| {
            r.set_mode(Mode::Annotate, w, cx);
            r.open_find(cx);
        }),
        ("outline", |r, _, _| r.chrome.panel = Panel::Outline),
        ("bookmarks", |r, _, _| r.chrome.panel = Panel::Bookmarks),
        ("results", |r, _, _| r.chrome.panel = Panel::Results),
        ("attachments", |r, _, _| r.chrome.panel = Panel::Attachments),
        ("inspector document", |r, w, cx| {
            r.set_inspector(InspectorTab::Document, w, cx)
        }),
        ("inspector selection empty", |r, w, cx| {
            r.set_inspector(InspectorTab::Selection, w, cx)
        }),
        ("inspector selection", |r, w, cx| {
            r.selection = Some(selection::Selection {
                anchor: 0,
                head: 8,
                dragging: false,
            });
            r.set_inspector(InspectorTab::Selection, w, cx);
        }),
        ("inspector signatures", |r, w, cx| {
            r.set_inspector(InspectorTab::Signatures, w, cx)
        }),
        ("organize", |r, w, cx| {
            r.set_mode(Mode::Organize, w, cx);
            r.chrome.organize_selection.insert(1);
        }),
        ("sign draw", |r, _, cx| r.set_overlay(Overlay::Sign, cx)),
        ("sign type", |r, _, cx| {
            r.chrome.sign_tab = 1;
            r.set_overlay(Overlay::Sign, cx);
        }),
        ("sign image", |r, _, cx| {
            r.chrome.sign_tab = 2;
            r.set_overlay(Overlay::Sign, cx);
        }),
        ("export", |r, _, cx| r.set_overlay(Overlay::Export, cx)),
        ("combine", |r, _, cx| r.set_overlay(Overlay::Combine, cx)),
        ("print", |r, _, cx| r.set_overlay(Overlay::Print, cx)),
        ("settings general", |r, _, cx| {
            r.set_overlay(Overlay::Settings, cx)
        }),
        ("settings reading", |r, _, cx| {
            r.chrome.settings_section = 1;
            r.set_overlay(Overlay::Settings, cx);
        }),
        ("settings notes", |r, _, cx| {
            r.chrome.settings_section = 2;
            r.set_overlay(Overlay::Settings, cx);
        }),
        ("settings updates", |r, _, cx| {
            r.chrome.settings_section = 3;
            r.set_overlay(Overlay::Settings, cx);
        }),
        ("shortcuts", |r, _, cx| {
            r.set_overlay(Overlay::Shortcuts, cx)
        }),
        ("toast", |r, _, cx| r.unavailable("Bookmarking pages", cx)),
        ("notice", |r, _, cx| {
            r.notice = Some("Only PDF files can be opened.".into());
            cx.notify();
        }),
        ("sidebar hidden", |r, _, cx| {
            r.sidebar = false;
            cx.notify();
        }),
        ("error", |r, _, cx| {
            r.tabs.current_mut().unwrap().error = Some("Could not open this PDF.".into());
            cx.notify();
        }),
    ];

    for choice in [ThemeChoice::Dark, ThemeChoice::Light] {
        cx.update(|window, cx| reader.update(cx, |r, cx| r.set_theme_choice(choice, window, cx)));
        cx.run_until_parked();
        sweep_mouse(cx);
        for (name, setup) in &screens {
            cx.update(|window, cx| {
                reader.update(cx, |r, cx| {
                    while !r.tabs.entries.is_empty() {
                        r.close_tab(0, window, cx);
                    }
                    r.chrome = chrome::Chrome::new(r.chrome.theme);
                    r.chrome.theme_choice = choice;
                    r.sidebar = true;
                    r.notice = None;
                    load_document(r, window, cx, 6);
                    setup(r, window, cx);
                })
            });
            cx.run_until_parked();
            sweep_mouse(cx);
            cx.read(|cx| {
                let r = reader.read(cx);
                assert!(
                    r.tabs.current().is_some(),
                    "{name}: document closed unexpectedly"
                );
            });
        }
    }
}
