use super::*;
use crate::pdf::{
    Raster, ViewKey,
    text::{Geometry, Glyph, Link, LinkTarget, PageText, TextRect},
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
                    sizes: None,
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
                        links: Vec::new(),
                        geometry: Geometry::UNIT,
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
                    sizes: None,
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
        let reader = reader.read(cx);
        let tab = reader.tabs.current().unwrap();
        assert_eq!(reader.bookmarks.of(&tab.path)[0].page, 4);
        assert_eq!(reader.bookmarks.of(&tab.path)[0].title, "Page 5");
        assert_eq!(reader.chrome.panel, chrome::Panel::Bookmarks);
    });
    cx.simulate_keystrokes(&format!("{modifier}-d"));
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert!(
            reader
                .bookmarks
                .of(&reader.tabs.current().unwrap().path)
                .is_empty()
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
                    sizes: None,
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

#[gpui::test]
fn recent_files_restore_position_clamp_and_prune(cx: &mut TestAppContext) {
    cx.update(|cx| cx.bind_keys(crate::app::bindings()));
    let (_sender, events) = async_channel::bounded(2);
    let (reader, cx) = cx
        .add_window_view(|window, cx| Reader::new(Vec::new(), Worker::idle(), events, window, cx));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let reading = root
        .join("tests/fixtures/reading.pdf")
        .canonicalize()
        .unwrap();
    let selection = root
        .join("tests/fixtures/selection.pdf")
        .canonicalize()
        .unwrap();
    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            reader.recent.touch(&reading);
            reader.recent.update(&reading, 7, Some(150));
            reader.open_paths(vec![reading.clone()], window, cx);
            let tab = reader.tabs.current().unwrap();
            assert_eq!((tab.page, tab.zoom), (7, Some(150)));
            assert_eq!(reader.last_request.as_ref().unwrap().0.page, 7);
            let generation = reader.generation;
            reader.receive(
                Event::Metadata {
                    generation,
                    count: 3,
                    sizes: None,
                },
                window,
                cx,
            );
            assert_eq!(reader.tabs.current().unwrap().page, 2);
            assert_ne!(reader.generation, generation);
            assert_eq!(reader.last_request.as_ref().unwrap().0.page, 2);
            reader.focus.focus(window);
        })
    });
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    cx.simulate_keystrokes(&format!("{modifier}-home"));
    cx.read(|cx| assert_eq!(reader.read(cx).tabs.current().unwrap().page, 0));
    cx.simulate_keystrokes(&format!("{modifier}-end"));
    cx.read(|cx| assert_eq!(reader.read(cx).tabs.current().unwrap().page, 2));

    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            reader.open_paths(vec![selection.clone()], window, cx);
            assert_eq!(reader.recent.entries()[0].path, selection);
            assert_eq!(reader.recent.get(&reading).unwrap().page, 2);
            reader.close_tab(0, window, cx);
            assert_eq!(reader.tabs.entries.len(), 1);
            assert_eq!(reader.recent.get(&reading).unwrap().page, 2);

            let missing = root.join("tests/fixtures/missing.pdf");
            reader.recent.touch(&missing);
            reader.open_recent(
                &OpenRecent {
                    path: missing.clone(),
                },
                window,
                cx,
            );
            assert!(reader.recent.get(&missing).is_none());
            assert_eq!(
                reader.chrome.toast.as_ref().unwrap().title.as_ref(),
                "This file is no longer available"
            );
            assert_eq!(reader.tabs.entries.len(), 1);

            reader.open_recent(
                &OpenRecent {
                    path: reading.clone(),
                },
                window,
                cx,
            );
            assert_eq!(reader.tabs.entries.len(), 2);
            assert_eq!(reader.tabs.current().unwrap().page, 2);
            reader.clear_recent(&ClearRecent, window, cx);
            assert!(reader.recent.entries().is_empty());
        })
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
    reader.receive(
        Event::Metadata {
            generation,
            count,
            sizes: None,
        },
        window,
        cx,
    );
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
                links: Vec::new(),
                geometry: Geometry::UNIT,
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

#[gpui::test]
fn marks_selection_undoes_saves_and_guards_closing(cx: &mut TestAppContext) {
    use crate::pdf::markup::{Edit, MarkupKind};
    use crate::ui::theme::Marker;

    cx.update(|cx| cx.bind_keys(crate::app::bindings()));
    let (_sender, events) = async_channel::bounded(2);
    let (reader, cx) = cx
        .add_window_view(|window, cx| Reader::new(Vec::new(), Worker::idle(), events, window, cx));
    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            load_document(reader, window, cx, 3);
            reader.focus.focus(window);
        })
    });
    cx.run_until_parked();
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    let drag = |cx: &mut gpui::VisualTestContext, from: f32, to: f32| {
        let bounds = cx.read(|cx| reader.read(cx).page_bounds.get());
        assert!(bounds.size.width > px(0.0));
        let at = |x: f32| bounds.origin + point(bounds.size.width * x, bounds.size.height * 0.215);
        cx.simulate_mouse_down(at(from), MouseButton::Left, Default::default());
        cx.simulate_mouse_move(at(to), MouseButton::Left, Default::default());
        cx.simulate_mouse_up(at(to), MouseButton::Left, Default::default());
    };

    drag(cx, 0.101, 0.259);
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert!(reader.has_selection());
        assert!(reader.tabs.current().unwrap().edits.is_empty());
    });
    cx.update(|_, cx| {
        reader.update(cx, |reader, cx| {
            reader.mark_selection(MarkupKind::Highlight, 1, cx)
        })
    });
    cx.read(|cx| {
        let reader = reader.read(cx);
        let tab = reader.tabs.current().unwrap();
        assert!(reader.selection.is_none());
        assert!(tab.modified());
        let Edit::Markup(markup) = &tab.edits[0] else {
            panic!("expected a text mark");
        };
        assert_eq!(markup.kind, MarkupKind::Highlight);
        assert_eq!(markup.color, Marker::ALL[1]);
        assert_eq!(markup.quote, "Selected");
        assert_eq!(markup.pages.len(), 1);
        assert_eq!(markup.pages[0].page, 0);
        let rect = markup.pages[0].rects[0];
        assert_eq!(markup.pages[0].rects.len(), 1);
        assert!((rect.left - 0.1).abs() < 1e-5 && (rect.right - 0.26).abs() < 1e-5);
    });

    cx.simulate_keystrokes(&format!("{modifier}-shift-a"));
    cx.update(|_, cx| {
        reader.update(cx, |reader, cx| {
            reader.choose_tool(chrome::Tool::Underline, cx)
        })
    });
    cx.run_until_parked();
    drag(cx, 0.281, 0.379);
    let marks = |cx: &mut gpui::VisualTestContext| {
        cx.read(|cx| {
            let tab = &reader.read(cx).tabs.entries[0];
            (tab.edits.len(), tab.undone.len(), tab.modified())
        })
    };
    assert_eq!(marks(cx), (2, 0, true));
    cx.read(|cx| {
        let Edit::Markup(markup) = &reader.read(cx).tabs.entries[0].edits[1] else {
            panic!("expected a text mark");
        };
        assert_eq!(markup.kind, MarkupKind::Underline);
        assert_eq!(markup.quote, "words");
    });

    cx.simulate_keystrokes(&format!("{modifier}-z"));
    assert_eq!(marks(cx), (1, 1, true));
    cx.simulate_keystrokes(&format!("{modifier}-shift-z"));
    assert_eq!(marks(cx), (2, 0, true));

    cx.dispatch_action(Save);
    let session = cx.read(|cx| {
        let reader = reader.read(cx);
        assert!(reader.tabs.entries[0].saving);
        assert_eq!(reader.worker.queued_saves(), [2]);
        reader.tabs.entries[0].session
    });
    cx.simulate_keystrokes(&format!("{modifier}-z"));
    assert_eq!(marks(cx), (2, 0, true));
    let offset = point(px(0.0), px(-40.0));
    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            reader.tabs.entries[0].scroll.set_offset(offset);
            reader.receive(
                Event::Saved {
                    session,
                    result: Ok(()),
                },
                window,
                cx,
            );
            assert!(reader.image.is_none());
            assert!(reader.last_request.is_some());
            assert_eq!(reader.tabs.entries[0].restore_scroll, Some(offset));
        })
    });
    assert_eq!(marks(cx), (2, 0, false));
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert_eq!(
            reader.chrome.toast.as_ref().unwrap().title.as_ref(),
            "Saved"
        );
        assert!(!reader.tabs.entries[0].saving);
    });
    cx.simulate_keystrokes(&format!("{modifier}-z"));
    assert_eq!(marks(cx), (2, 0, false));
    cx.dispatch_action(Close);
    cx.run_until_parked();
    assert!(!cx.has_pending_prompt());
    cx.read(|cx| assert!(reader.read(cx).tabs.entries.is_empty()));

    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            load_document(reader, window, cx, 3);
            reader.selection = Some(selection::Selection::within(0, 0..8));
            reader.mark_selection(MarkupKind::Strikeout, 0, cx);
            reader.save_document(cx);
            let session = reader.tabs.entries[0].session;
            reader.receive(
                Event::Saved {
                    session,
                    result: Err("The disk is full.".into()),
                },
                window,
                cx,
            );
            assert!(reader.tabs.entries[0].modified());
            assert_eq!(
                reader.chrome.toast.as_ref().unwrap().title.as_ref(),
                "Changes could not be saved"
            );
        })
    });
    cx.dispatch_action(Close);
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    cx.read(|cx| assert_eq!(reader.read(cx).tabs.entries.len(), 1));
    cx.dispatch_action(Close);
    cx.simulate_prompt_answer("Save");
    cx.run_until_parked();
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert!(reader.tabs.entries[0].saving);
        assert!(reader.tabs.entries[0].close_after_save);
    });
    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            let session = reader.tabs.entries[0].session;
            reader.receive(
                Event::Saved {
                    session,
                    result: Ok(()),
                },
                window,
                cx,
            );
            assert!(reader.tabs.entries.is_empty());
            load_document(reader, window, cx, 3);
            reader.selection = Some(selection::Selection::within(0, 0..8));
            reader.mark_selection(MarkupKind::Highlight, 0, cx);
        })
    });
    cx.dispatch_action(Close);
    cx.simulate_prompt_answer("Discard");
    cx.run_until_parked();
    cx.read(|cx| assert!(reader.read(cx).tabs.entries.is_empty()));
}

#[gpui::test]
fn draws_places_adjusts_and_saves_a_signature(cx: &mut TestAppContext) {
    use crate::pdf::markup::Edit;

    cx.update(|cx| cx.bind_keys(crate::app::bindings()));
    let (_sender, events) = async_channel::bounded(2);
    let (reader, cx) = cx
        .add_window_view(|window, cx| Reader::new(Vec::new(), Worker::idle(), events, window, cx));
    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            load_document(reader, window, cx, 3);
            reader.focus.focus(window);
            reader.set_overlay(chrome::Overlay::Sign, cx);
        })
    });
    cx.run_until_parked();
    cx.update(|_, cx| reader.update(cx, |reader, cx| reader.place_signature(cx)));
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert_eq!(reader.chrome.overlay, chrome::Overlay::Sign);
        assert_eq!(
            reader.chrome.toast.as_ref().unwrap().title.as_ref(),
            "Draw your signature first"
        );
        assert!(reader.tabs.entries[0].edits.is_empty());
    });

    let pad = cx.read(|cx| reader.read(cx).signing.pad.get());
    assert!(pad.size.width > px(300.0));
    let stroke = |cx: &mut gpui::VisualTestContext, points: &[(f32, f32)]| {
        let at = |index: usize| pad.origin + point(px(points[index].0), px(points[index].1));
        cx.simulate_mouse_down(at(0), MouseButton::Left, Default::default());
        for index in 1..points.len() {
            cx.simulate_mouse_move(at(index), MouseButton::Left, Default::default());
        }
        cx.simulate_mouse_up(at(points.len() - 1), MouseButton::Left, Default::default());
    };
    stroke(cx, &[(60.0, 100.0), (160.0, 60.0), (260.0, 100.0)]);
    cx.read(|cx| {
        let signing = &reader.read(cx).signing;
        assert!(!signing.drawing);
        assert_eq!(
            signing.strokes,
            [vec![[60.0, 100.0], [160.0, 60.0], [260.0, 100.0]]]
        );
    });
    cx.update(|_, cx| reader.update(cx, |reader, cx| reader.clear_pad(cx)));
    cx.read(|cx| assert!(reader.read(cx).signing.strokes.is_empty()));
    stroke(cx, &[(60.0, 100.0), (160.0, 60.0), (260.0, 100.0)]);
    stroke(cx, &[(60.0, 120.0), (260.0, 120.0)]);
    cx.simulate_mouse_move(
        pad.origin + point(px(200.0), px(20.0)),
        None,
        Default::default(),
    );
    cx.read(|cx| assert_eq!(reader.read(cx).signing.strokes.len(), 2));

    cx.update(|_, cx| reader.update(cx, |reader, cx| reader.place_signature(cx)));
    cx.run_until_parked();
    let bounds = |cx: &mut gpui::VisualTestContext| {
        cx.read(|cx| match reader.read(cx).tabs.entries[0].edits.last() {
            Some(Edit::Signature(signature)) => signature.bounds,
            _ => panic!("expected a signature"),
        })
    };
    let placed = bounds(cx);
    cx.read(|cx| {
        let reader = reader.read(cx);
        let tab = &reader.tabs.entries[0];
        assert_eq!(reader.chrome.overlay, chrome::Overlay::None);
        assert!(tab.modified());
        assert_eq!(tab.edits.len(), 1);
        let Edit::Signature(signature) = &tab.edits[0] else {
            panic!("expected a signature");
        };
        assert_eq!(signature.page, 0);
        assert_eq!(signature.color, signing::INKS[0]);
        assert_eq!(signature.strokes.len(), 2);
        assert_eq!(signature.strokes[1], [[0.0, 1.0], [1.0, 1.0]]);
        assert!(reader.placing_signature().is_some());
        assert_eq!(tab.edits[0].label(), "Signature");
    });
    assert!((placed.right - placed.left - 0.32).abs() < 1e-5);
    assert!((placed.left - 0.34).abs() < 1e-5);
    let shape = (placed.bottom - placed.top) / (placed.right - placed.left);

    let frame = cx.read(|cx| reader.read(cx).page_bounds.get());
    assert!(frame.size.width > px(0.0));
    let on_page =
        |x: f32, y: f32| frame.origin + point(frame.size.width * x, frame.size.height * y);
    let drag = |cx: &mut gpui::VisualTestContext, from: (f32, f32), to: (f32, f32)| {
        cx.simulate_mouse_down(
            on_page(from.0, from.1),
            MouseButton::Left,
            Default::default(),
        );
        cx.simulate_mouse_move(on_page(to.0, to.1), MouseButton::Left, Default::default());
        cx.simulate_mouse_up(on_page(to.0, to.1), MouseButton::Left, Default::default());
    };
    let center = (
        (placed.left + placed.right) / 2.0,
        (placed.top + placed.bottom) / 2.0,
    );
    drag(cx, center, (center.0 + 0.1, center.1 + 0.15));
    let moved = bounds(cx);
    assert!((moved.left - placed.left - 0.1).abs() < 0.01);
    assert!((moved.top - placed.top - 0.15).abs() < 0.01);
    assert!((moved.right - moved.left - 0.32).abs() < 1e-4);
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert!(reader.selection.is_none());
        assert!(reader.signing.placing.unwrap().drag.is_none());
    });

    drag(
        cx,
        (moved.right, moved.bottom),
        (moved.right + 0.1, moved.bottom + 0.3),
    );
    let grown = bounds(cx);
    assert!((grown.left - moved.left).abs() < 1e-4 && (grown.top - moved.top).abs() < 1e-4);
    assert!((grown.right - moved.right - 0.1).abs() < 0.01);
    assert!(((grown.bottom - grown.top) / (grown.right - grown.left) - shape).abs() < 1e-3);

    cx.simulate_keystrokes("enter");
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert!(reader.signing.placing.is_none());
        assert_eq!(reader.tabs.entries[0].edits.len(), 1);
    });
    assert_eq!(bounds(cx), grown);

    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    cx.simulate_keystrokes(&format!("{modifier}-z"));
    cx.read(|cx| assert!(!reader.read(cx).tabs.entries[0].modified()));
    cx.simulate_keystrokes(&format!("{modifier}-shift-z"));
    assert_eq!(bounds(cx), grown);

    cx.update(|_, cx| reader.update(cx, |reader, cx| reader.place_signature(cx)));
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert_eq!(reader.tabs.entries[0].edits.len(), 2);
        assert!(reader.placing_signature().is_some());
    });
    cx.simulate_keystrokes("escape");
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert_eq!(reader.tabs.entries[0].edits.len(), 1);
        assert!(reader.signing.placing.is_none());
        assert!(reader.tabs.entries[0].undone.is_empty());
    });

    cx.update(|_, cx| reader.update(cx, |reader, cx| reader.place_signature(cx)));
    cx.run_until_parked();
    drag(cx, (0.05, 0.95), (0.06, 0.96));
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert!(reader.signing.placing.is_none());
        assert_eq!(reader.tabs.entries[0].edits.len(), 2);
    });
    cx.dispatch_action(Save);
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert!(reader.tabs.entries[0].saving);
        assert_eq!(reader.worker.queued_saves(), [2]);
    });
}

#[gpui::test]
fn preferences_drive_opening_zoom_fit_page_bookmarks_and_autosave(cx: &mut TestAppContext) {
    use crate::pdf::markup::MarkupKind;
    use crate::preferences::DefaultZoom;

    cx.update(|cx| cx.bind_keys(crate::app::bindings()));
    let (_sender, events) = async_channel::bounded(2);
    let (reader, cx) = cx
        .add_window_view(|window, cx| Reader::new(Vec::new(), Worker::idle(), events, window, cx));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            assert!(reader.preferences.last_page);
            assert!(!reader.preferences.autosave);
            reader.update_preferences(
                |preferences| preferences.default_zoom = DefaultZoom::Actual,
                cx,
            );
            reader.open_paths(vec![root.join("tests/fixtures/links.pdf")], window, cx);
            assert_eq!(reader.tabs.current().unwrap().zoom, Some(100));
            reader.close_tab_now(0, window, cx);

            reader.update_preferences(
                |preferences| preferences.default_zoom = DefaultZoom::FitPage,
                cx,
            );
            load_document(reader, window, cx, 3);
            reader.focus.focus(window);
        })
    });
    cx.run_until_parked();
    let fitted = cx.read(|cx| {
        let reader = reader.read(cx);
        let tab = reader.tabs.current().unwrap();
        assert!(!tab.fit_page_pending);
        tab.zoom
            .expect("fit page sets a zoom once the page is known")
    });
    assert!((25..=400).contains(&fitted));

    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    cx.simulate_keystrokes(&format!("{modifier}-1"));
    cx.read(|cx| assert_eq!(reader.read(cx).tabs.current().unwrap().zoom, Some(100)));
    cx.simulate_keystrokes(&format!("{modifier}-9"));
    cx.read(|cx| {
        let zoom = reader.read(cx).tabs.current().unwrap().zoom;
        assert!(zoom.is_some_and(|zoom| (25..=400).contains(&zoom)));
    });

    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            reader
                .recent
                .update(&reader.tabs.entries[0].path.clone(), 2, Some(150));
            reader.close_tab_now(0, window, cx);
            reader.update_preferences(|preferences| preferences.last_page = false, cx);
            load_document(reader, window, cx, 3);
            let tab = reader.tabs.current().unwrap();
            assert_eq!(tab.page, 0);
            assert_ne!(tab.zoom, Some(150));

            reader.toggle_bookmark(cx);
            let path = reader.tabs.entries[0].path.clone();
            assert_eq!(
                reader.bookmarks.of(&path)[0].title,
                "Selected words on the page"
            );
            assert_eq!(reader.chrome.panel, chrome::Panel::Bookmarks);

            reader.selection = Some(selection::Selection::within(0, 0..8));
            reader.mark_selection(MarkupKind::Highlight, 0, cx);
        })
    });
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(3));
    cx.run_until_parked();
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert!(!reader.tabs.entries[0].saving);
        assert!(reader.worker.queued_saves().is_empty());
    });
    cx.update(|_, cx| {
        reader.update(cx, |reader, cx| {
            reader.update_preferences(|preferences| preferences.autosave = true, cx);
            reader.selection = Some(selection::Selection::within(0, 9..14));
            reader.mark_selection(MarkupKind::Underline, 0, cx);
            assert!(!reader.tabs.entries[0].saving);
        })
    });
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    cx.read(|cx| assert!(!reader.read(cx).tabs.entries[0].saving));
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(2));
    cx.run_until_parked();
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert!(reader.tabs.entries[0].saving);
        assert_eq!(reader.worker.queued_saves(), [2]);
    });
}

#[gpui::test]
fn last_session_tabs_reopen_only_when_asked_and_skip_missing_files(cx: &mut TestAppContext) {
    let (_sender, events) = async_channel::bounded(2);
    let (reader, cx) = cx
        .add_window_view(|window, cx| Reader::new(Vec::new(), Worker::idle(), events, window, cx));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let store = std::env::temp_dir().join(format!("agenticpdf-session-ui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store);
    let fixtures = [
        root.join("tests/fixtures/reading.pdf"),
        root.join("tests/fixtures/links.pdf"),
        root.join("tests/fixtures/outline.pdf"),
    ];
    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            reader.data = Some(store.clone());
            reader.open_paths(fixtures.to_vec(), window, cx);
            reader.activate_tab(1, window, cx);
            reader.save_session().unwrap();
            assert!(!store.join("session.tsv").exists());

            reader.update_preferences(|preferences| preferences.reopen_tabs = true, cx);
            assert!(store.join("preferences.conf").exists());
            reader.save_session().unwrap();
            let saved = std::fs::read_to_string(store.join("session.tsv")).unwrap();
            let listed = format!(
                "{saved}tab\t{}\n",
                root.join("tests/fixtures/gone.pdf").display()
            );
            std::fs::write(store.join("session.tsv"), listed).unwrap();
            while !reader.tabs.entries.is_empty() {
                reader.close_tab_now(0, window, cx);
            }

            reader.restore_session(window, cx);
            assert_eq!(reader.tabs.entries.len(), 3);
            assert_eq!(reader.tabs.active, 1);
            assert!(reader.tabs.current().unwrap().path.ends_with("links.pdf"));

            while !reader.tabs.entries.is_empty() {
                reader.close_tab_now(0, window, cx);
            }
            reader.update_preferences(|preferences| preferences.reopen_tabs = false, cx);
            reader.restore_session(window, cx);
            assert!(reader.tabs.entries.is_empty());
        })
    });
    std::fs::remove_dir_all(&store).unwrap();
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
            r.selection = Some(selection::Selection::within(0, 0..8));
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
        ("marked notes", |r, w, cx| {
            use crate::pdf::markup::MarkupKind;
            r.set_mode(Mode::Annotate, w, cx);
            for (range, kind, color) in [
                (0..8, MarkupKind::Highlight, 0),
                (9..14, MarkupKind::Underline, 2),
                (15..17, MarkupKind::Strikeout, 3),
            ] {
                r.selection = Some(selection::Selection::within(0, range));
                r.mark_selection(kind, color, cx);
            }
            assert_eq!(r.tabs.current().unwrap().edits.len(), 3);
        }),
        ("sign drawn", |r, _, cx| {
            r.signing.strokes = vec![vec![[40.0, 90.0], [120.0, 50.0], [220.0, 96.0]]];
            r.set_overlay(Overlay::Sign, cx);
        }),
        ("placing signature", |r, _, cx| {
            r.signing.strokes = vec![vec![[40.0, 90.0], [120.0, 50.0], [220.0, 96.0]]];
            r.place_signature(cx);
            assert!(r.placing_signature().is_some());
        }),
        ("bookmarks", |r, _, cx| {
            let path = r.tabs.current().unwrap().path.clone();
            if r.bookmarks.of(&path).is_empty() {
                r.toggle_bookmark(cx);
            }
            r.chrome.panel = Panel::Bookmarks;
            assert_eq!(r.bookmarks.of(&path).len(), 1);
        }),
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
        ("agent-empty", |r, _, _| r.agent.open = true),
        ("agent-conversation", |r, _, _| {
            use crate::agent::conversation::{
                Choice, ChoiceKind, PlanStep, Status, ToolRow, Update,
            };
            r.agent.open = true;
            let conversation = &mut r.agent.conversation;
            conversation.ask("Summarize this document.", "reading.pdf, 3 pages");
            conversation.apply(Update::Plan(vec![
                PlanStep {
                    text: "Get the document outline".into(),
                    status: Status::Done,
                },
                PlanStep {
                    text: "Read every page".into(),
                    status: Status::Running,
                },
            ]));
            conversation.apply(Update::ToolStarted(ToolRow {
                id: "t1".into(),
                title: "Read text".into(),
                detail: "From page 1".into(),
                status: Status::Done,
            }));
            conversation.apply(Update::Text(
                "## Summary\nA short essay about reading (p. 1).\n\n- Margins give rhythm (p. 2)"
                    .into(),
            ));
            conversation.permission(
                1,
                "highlight".into(),
                vec![
                    Choice {
                        id: "yes".into(),
                        name: "Allow once".into(),
                        kind: ChoiceKind::AllowOnce,
                    },
                    Choice {
                        id: "no".into(),
                        name: "Reject".into(),
                        kind: ChoiceKind::RejectOnce,
                    },
                ],
            );
            conversation.notice("Stopped.");
        }),
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
            r.selection = Some(selection::Selection::within(0, 0..8));
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
                        r.close_tab_now(0, window, cx);
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

fn link_page(links: Vec<Link>) -> TextResult {
    Ok(Arc::new(PageText {
        glyphs: Vec::new(),
        links,
        geometry: Geometry::UNIT,
    }))
}

#[gpui::test]
fn links_jump_and_back_forward_retrace_each_tab(cx: &mut TestAppContext) {
    let (_sender, events) = async_channel::bounded(2);
    let (reader, cx) = cx
        .add_window_view(|window, cx| Reader::new(Vec::new(), Worker::idle(), events, window, cx));
    let rect = |top: f32| TextRect {
        left: 0.1,
        top,
        right: 0.4,
        bottom: top + 0.05,
    };
    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            load_document(reader, window, cx, 9);
            let generation = reader.generation;
            let session = reader.tabs.current().unwrap().session;
            reader.receive(
                Event::Text {
                    generation,
                    key: (session, 0),
                    text: link_page(vec![
                        Link {
                            bounds: rect(0.5),
                            target: LinkTarget::Page {
                                index: 6,
                                anchor: Some((0.0, 0.25)),
                            },
                        },
                        Link {
                            bounds: rect(0.6),
                            target: LinkTarget::Uri("javascript:alert(1)".into()),
                        },
                    ]),
                },
                window,
                cx,
            );
        })
    });
    cx.run_until_parked();
    let click = |cx: &mut gpui::VisualTestContext, y: f32| {
        let bounds = cx.read(|cx| reader.read(cx).page_bounds.get());
        assert!(bounds.size.width > px(0.0));
        let at = bounds.origin + point(bounds.size.width * 0.25, bounds.size.height * y);
        cx.simulate_mouse_down(at, MouseButton::Left, Default::default());
        cx.simulate_mouse_up(at, MouseButton::Left, Default::default());
    };
    let page = |cx: &mut gpui::VisualTestContext| {
        cx.read(|cx| reader.read(cx).tabs.current().unwrap().page)
    };

    click(cx, 0.625);
    assert_eq!(page(cx), 0);
    assert_eq!(
        cx.read(|cx| reader
            .read(cx)
            .chrome
            .toast
            .as_ref()
            .unwrap()
            .title
            .to_string()),
        "This link was not opened"
    );

    click(cx, 0.525);
    assert_eq!(page(cx), 6);
    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            assert!(reader.selection.is_none());
            let generation = reader.generation;
            let view = reader.last_request.as_ref().unwrap().0.clone();
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
            assert!(reader.tabs.current().unwrap().pending_anchor.is_some());
            reader.receive(
                Event::Text {
                    generation,
                    key: (view.session, 6),
                    text: link_page(Vec::new()),
                },
                window,
                cx,
            );
            let tab = reader.tabs.current().unwrap();
            assert!(tab.pending_anchor.is_none());
            assert_eq!(tab.scroll.offset().y, px(-(28.0 + 0.75 * 1200.0 - 16.0)));
        })
    });

    cx.dispatch_action(GoBack);
    assert_eq!(page(cx), 0);
    cx.dispatch_action(GoBack);
    assert_eq!(page(cx), 0);
    cx.dispatch_action(GoForward);
    assert_eq!(page(cx), 6);
    cx.dispatch_action(GoBack);
    cx.dispatch_action(LastPage);
    assert_eq!(page(cx), 8);
    cx.dispatch_action(GoForward);
    assert_eq!(page(cx), 8);
    cx.dispatch_action(GoBack);
    assert_eq!(page(cx), 0);

    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            reader.open_paths(vec![root.join("tests/fixtures/selection.pdf")], window, cx);
            let generation = reader.generation;
            reader.receive(
                Event::Metadata {
                    generation,
                    count: 3,
                    sizes: None,
                },
                window,
                cx,
            );
        })
    });
    cx.dispatch_action(GoBack);
    assert_eq!(page(cx), 0);
    cx.dispatch_action(PreviousTab);
    cx.dispatch_action(GoForward);
    assert_eq!(page(cx), 8);
}

#[gpui::test]
fn outline_panel_expands_jumps_and_follows_reading(cx: &mut TestAppContext) {
    use crate::pdf::tools::{Outline, OutlineEntry};

    let (_sender, events) = async_channel::bounded(2);
    let (reader, cx) = cx
        .add_window_view(|window, cx| Reader::new(Vec::new(), Worker::idle(), events, window, cx));
    let entry = |title: &str, level, page, anchor| OutlineEntry {
        title: title.into(),
        level,
        page: Some(page),
        anchor,
    };
    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            load_document(reader, window, cx, 9);
            let session = reader.tabs.current().unwrap().session;
            reader.receive(
                Event::Outline {
                    session: session + 100,
                    outline: Arc::new(Outline {
                        entries: vec![entry("Elsewhere", 1, 1, None)],
                        truncated: false,
                    }),
                },
                window,
                cx,
            );
            assert!(reader.tabs.current().unwrap().outline.is_none());
            reader.receive(
                Event::Outline {
                    session,
                    outline: Arc::new(Outline {
                        entries: vec![
                            entry("Introduction", 1, 1, None),
                            entry("Part one", 1, 3, None),
                            entry("Chapter one", 2, 5, Some((0.0, 0.25))),
                            entry("Section", 3, 7, None),
                        ],
                        truncated: false,
                    }),
                },
                window,
                cx,
            );
            reader.chrome.panel = chrome::Panel::Outline;
        })
    });
    cx.run_until_parked();
    let page = |cx: &mut gpui::VisualTestContext| {
        cx.read(|cx| reader.read(cx).tabs.current().unwrap().page)
    };
    let center = |cx: &mut gpui::VisualTestContext, selector: &'static str| {
        cx.debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} is not visible"))
            .center()
    };
    assert!(cx.debug_bounds("outline-entry-1").is_some());
    assert!(cx.debug_bounds("outline-entry-2").is_none());

    let toggle = center(cx, "outline-toggle-1");
    cx.simulate_click(toggle, Default::default());
    cx.run_until_parked();
    assert!(cx.debug_bounds("outline-entry-2").is_some());
    assert!(cx.debug_bounds("outline-entry-3").is_none());
    assert_eq!(page(cx), 0);

    let chapter = center(cx, "outline-entry-2");
    cx.simulate_click(chapter, Default::default());
    assert_eq!(page(cx), 4);
    assert_eq!(
        cx.read(|cx| reader.read(cx).tabs.current().unwrap().pending_anchor),
        Some((0.0, 0.25))
    );
    cx.dispatch_action(GoBack);
    assert_eq!(page(cx), 0);

    cx.update(|window, cx| reader.update(cx, |reader, cx| reader.go(6, window, cx)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("outline-entry-3").is_some());
}

#[gpui::test]
fn continuous_layout_tracks_scrolling_navigation_and_tabs(cx: &mut TestAppContext) {
    use gpui::{ScrollDelta, ScrollWheelEvent};

    let (_sender, events) = async_channel::bounded(2);
    let (reader, cx) = cx
        .add_window_view(|window, cx| Reader::new(Vec::new(), Worker::idle(), events, window, cx));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let open = |reader: &mut Reader,
                window: &mut gpui::Window,
                cx: &mut gpui::Context<Reader>,
                file: &str| {
        reader.open_paths(vec![root.join(file)], window, cx);
        let generation = reader.generation;
        reader.receive(
            Event::Metadata {
                generation,
                count: 9,
                sizes: Some(Arc::new(vec![(612.0, 792.0); 9])),
            },
            window,
            cx,
        );
        reader.request(window);
        let view = reader.last_request.as_ref().unwrap().0.clone();
        let width = view.available_width as f32;
        reader.receive(
            Event::Page {
                generation: reader.generation,
                view,
                raster: Raster {
                    bytes: vec![255; 40 * 52 * 4],
                    width: 40,
                    height: 52,
                    logical_width: width,
                    logical_height: width * 792.0 / 612.0,
                    zoom_percent: 100.0,
                },
            },
            window,
            cx,
        );
    };
    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            open(reader, window, cx, "tests/fixtures/reading.pdf");
            assert!(reader.continuous());
        })
    });
    cx.run_until_parked();
    let state = |cx: &mut gpui::VisualTestContext| {
        cx.read(|cx| {
            let reader = reader.read(cx);
            let tab = reader.tabs.current().unwrap();
            (
                tab.page,
                f32::from(tab.scroll.offset().y),
                reader.last_request.as_ref().unwrap().2.clone(),
            )
        })
    };
    let (page, offset, nearby) = state(cx);
    assert_eq!((page, offset), (0, 0.0));
    assert!(nearby.contains(&1), "nearby pages requested: {nearby:?}");

    cx.dispatch_action(Next);
    cx.run_until_parked();
    let column = cx.read(|cx| {
        reader
            .read(cx)
            .tabs
            .current()
            .unwrap()
            .column
            .clone()
            .unwrap()
    });
    let (page, offset, _) = state(cx);
    assert_eq!(page, 1);
    assert_eq!(offset, -column.scroll_to(1));

    let bounds = cx
        .debug_bounds("document-canvas")
        .expect("the document canvas is on screen");
    cx.simulate_event(ScrollWheelEvent {
        position: bounds.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-(column.heights[0] * 3.0)))),
        ..Default::default()
    });
    cx.run_until_parked();
    let (page, offset, nearby) = state(cx);
    let viewport = cx.read(|cx| {
        f32::from(
            reader
                .read(cx)
                .tabs
                .current()
                .unwrap()
                .scroll
                .bounds()
                .size
                .height,
        )
    });
    assert!(offset < -column.scroll_to(1));
    assert_eq!(
        page,
        column.dominant(-offset - column::COLUMN_TOP, viewport)
    );
    assert!(page >= 3, "scrolled to page {page}");
    assert!(nearby.contains(&(page + 1)));

    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            open(reader, window, cx, "tests/fixtures/selection.pdf");
            assert_eq!(reader.tabs.active, 1);
        })
    });
    cx.run_until_parked();
    assert_eq!(state(cx).0, 0);
    cx.dispatch_action(PreviousTab);
    cx.run_until_parked();
    let (restored_page, restored, _) = state(cx);
    assert_eq!((restored_page, restored), (page, offset));
}

fn line_text(content: &str, top: f32) -> TextResult {
    Ok(Arc::new(PageText {
        links: Vec::new(),
        geometry: Geometry::UNIT,
        glyphs: content
            .chars()
            .enumerate()
            .map(|(index, value)| Glyph {
                value,
                bounds: Some(TextRect {
                    left: 0.1 + index as f32 * 0.02,
                    right: 0.12 + index as f32 * 0.02,
                    top,
                    bottom: top + 0.03,
                }),
                direction: [1.0, 0.0],
            })
            .collect(),
    }))
}

#[gpui::test]
fn continuous_selection_spans_pages_copies_and_survives_scrolling(cx: &mut TestAppContext) {
    use gpui::{ClipboardItem, ScrollDelta, ScrollWheelEvent};

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
                    count: 4,
                    sizes: Some(Arc::new(vec![(612.0, 792.0); 4])),
                },
                window,
                cx,
            );
            reader.request(window);
            assert!(reader.continuous());
            let view = reader.last_request.as_ref().unwrap().0.clone();
            let session = view.session;
            let width = view.available_width as f32;
            for page in 0..3 {
                reader.receive(
                    Event::Page {
                        generation: reader.generation,
                        view: ViewKey {
                            page,
                            ..view.clone()
                        },
                        raster: Raster {
                            bytes: vec![255; 40 * 52 * 4],
                            width: 40,
                            height: 52,
                            logical_width: width,
                            logical_height: width * 792.0 / 612.0,
                            zoom_percent: 100.0,
                        },
                    },
                    window,
                    cx,
                );
            }
            for (page, content, top) in [
                (0, "First page ends here", 0.9),
                (1, "Second page starts", 0.05),
                (2, "Third", 0.05),
            ] {
                reader.receive(
                    Event::Text {
                        generation: reader.generation,
                        key: (session, page),
                        text: line_text(content, top),
                    },
                    window,
                    cx,
                );
            }
            let column = reader.tabs.current().unwrap().column.clone().unwrap();
            let (_, viewport) = reader.viewport(window);
            let tab = reader.tabs.current_mut().unwrap();
            tab.scroll.set_offset(point(
                px(0.0),
                px(-(column::COLUMN_TOP + column.tops[1] - viewport / 2.0)),
            ));
            cx.notify();
        })
    });
    cx.run_until_parked();
    let frames = cx.read(|cx| reader.read(cx).page_frames.borrow().clone());
    let (first, second) = (frames[&0], frames[&1]);
    let at = |bounds: gpui::Bounds<gpui::Pixels>, x: f32, y: f32| {
        bounds.origin + point(bounds.size.width * x, bounds.size.height * y)
    };
    let start = at(first, 0.1 + 6.0 * 0.02 + 0.002, 0.915);
    let end = at(second, 0.1 + 5.0 * 0.02 + 0.018, 0.065);
    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(end, MouseButton::Left, Default::default());
    cx.simulate_mouse_up(end, MouseButton::Left, Default::default());
    cx.read(|cx| {
        let reader = reader.read(cx);
        let selection = reader.selection.as_ref().expect("a selection was made");
        assert_eq!(reader.tabs.current().unwrap().page, 0);
        assert_eq!(selection.pages(), 0..=1);
        assert_eq!(selection.on_page(0, 20), 6..20);
        assert_eq!(selection.on_page(1, 18), 0..6);
    });
    cx.dispatch_action(CopyText);
    assert_eq!(
        cx.read_from_clipboard().and_then(|item| item.text()),
        Some("page ends here\nSecond".into())
    );

    let column = cx.read(|cx| {
        reader
            .read(cx)
            .tabs
            .current()
            .unwrap()
            .column
            .clone()
            .unwrap()
    });
    let canvas = cx
        .debug_bounds("document-canvas")
        .expect("the document canvas is on screen");
    cx.simulate_event(ScrollWheelEvent {
        position: canvas.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-column.heights[0]))),
        ..Default::default()
    });
    cx.run_until_parked();
    cx.write_to_clipboard(ClipboardItem::new_string(String::new()));
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert_ne!(reader.tabs.current().unwrap().page, 0);
        assert!(reader.has_selection());
    });
    cx.dispatch_action(CopyText);
    assert_eq!(
        cx.read_from_clipboard().and_then(|item| item.text()),
        Some("page ends here\nSecond".into())
    );

    cx.update(|window, cx| {
        reader.update(cx, |reader, cx| {
            let session = reader.tabs.current().unwrap().session;
            reader.selection = Some(selection::Selection {
                anchor: selection::Caret { page: 0, glyph: 6 },
                head: selection::Caret { page: 2, glyph: 5 },
                dragging: false,
            });
            reader.texts.take(&(session, 1));
            if reader.tabs.current().unwrap().page == 1 {
                reader.text = None;
            }
            reader.copy(&CopyText, window, cx);
            let toast = reader.chrome.toast.as_ref().expect("copy explains the gap");
            assert_eq!(toast.title.to_string(), "Selection is still loading");
            assert!(toast.message.contains("page 2"));
            assert!(reader.has_selection());
        })
    });
    assert_eq!(
        cx.read_from_clipboard().and_then(|item| item.text()),
        Some("page ends here\nSecond".into())
    );
}

#[gpui::test]
fn update_ready_control_popover_and_settings_states(cx: &mut TestAppContext) {
    use super::updates::Phase;
    use crate::update::{Asset, Release, Version};
    use std::time::Instant;

    let (_sender, events) = async_channel::bounded(2);
    let (reader, cx) = cx
        .add_window_view(|window, cx| Reader::new(Vec::new(), Worker::idle(), events, window, cx));
    let asset = |name: &str| Asset {
        name: name.into(),
        size: 1,
        url: String::new(),
    };
    let release = || Release {
        version: Version::parse_tag("v9.9.9").unwrap(),
        tag: "v9.9.9".into(),
        page_url: String::new(),
        notes: vec![
            "Pages scroll as one continuous column, even when the note is long enough to wrap"
                .into(),
            "Agent panel for installed agents".into(),
        ],
        archive: asset("archive.zip"),
        sums: asset("SHA256SUMS.txt"),
    };
    assert!(cx.debug_bounds("update-ready").is_none());
    cx.update(|_, cx| {
        reader.update(cx, |reader, cx| {
            reader.updates.phase = Phase::Ready {
                release: Box::new(release()),
                staged: PathBuf::from("/nonexistent"),
            };
            cx.notify();
        })
    });
    cx.run_until_parked();
    let control = cx
        .debug_bounds("update-ready")
        .expect("Update ready is visible");
    cx.simulate_click(control.center(), Default::default());
    cx.run_until_parked();
    let popover = cx.debug_bounds("update-popover").expect("popover is open");
    let restart = cx.debug_bounds("update-restart").unwrap();
    let later = cx.debug_bounds("update-later").unwrap();
    assert!(cx.debug_bounds("update-notes").is_some());
    assert_eq!(later.left() - restart.right(), px(8.0));
    assert_eq!(popover.size.width, px(340.0));
    assert!(popover.top() >= control.bottom());

    cx.simulate_click(later.center(), Default::default());
    cx.run_until_parked();
    cx.read(|cx| {
        let reader = reader.read(cx);
        assert!(reader.updates.dismissed);
        assert!(!reader.updates.shows_control());
        assert_eq!(reader.chrome.overlay, chrome::Overlay::None);
    });

    let now = Instant::now();
    let phases = [
        Phase::Unavailable("Updates are available in release builds only"),
        Phase::Idle,
        Phase::Checking,
        Phase::Current { checked: now },
        Phase::Downloading {
            version: "9.9.9".into(),
            received: 14_200_000,
            total: 31_000_000,
        },
        Phase::Ready {
            release: Box::new(release()),
            staged: PathBuf::from("/nonexistent"),
        },
        Phase::Installing {
            version: "9.9.9".into(),
        },
        Phase::Failed {
            message: "The 9.9.9 download did not match its checksum and was removed".into(),
            checked: now,
        },
    ];
    for phase in phases {
        cx.update(|_, cx| {
            reader.update(cx, |reader, cx| {
                reader.updates.phase = phase;
                reader.chrome.settings_section = super::updates::UPDATES_SECTION;
                reader.chrome.overlay = chrome::Overlay::Settings;
                cx.notify();
            })
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("update-automatic").is_some());
    }
    let installing = cx.update(|_, cx| {
        reader.update(cx, |reader, _| {
            reader.updates.phase = Phase::Installing {
                version: "9.9.9".into(),
            };
            reader.updates.shows_control()
        })
    });
    assert!(installing);
}
