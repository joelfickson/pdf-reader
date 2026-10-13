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
