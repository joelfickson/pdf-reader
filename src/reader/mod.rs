mod actions;
mod cache;
mod images;
mod navigation;
mod scheduling;
mod selection;
mod tabs;
#[cfg(test)]
mod tests;
mod view;

use crate::pdf::{Event, ViewKey, Worker, text::PageText};
pub(crate) use actions::*;
use cache::PageCache;
use gpui::{Context, FocusHandle, Window};
use images::PageImage;
use selection::Selection;
use std::{cell::Cell, ops::Range, path::PathBuf, rc::Rc, sync::Arc};
use tabs::Tabs;

type TextResult = Result<Arc<PageText>, String>;

pub(crate) struct Reader {
    focus: FocusHandle,
    worker: Worker,
    tabs: Tabs,
    generation: u64,
    sidebar: bool,
    image: Option<PageImage>,
    image_key: Option<ViewKey>,
    pages: PageCache<PageImage, ViewKey>,
    thumbnails: PageCache<PageImage, (u64, usize)>,
    texts: PageCache<TextResult, (u64, usize)>,
    text: Option<TextResult>,
    selection: Option<Selection>,
    last_request: Option<(ViewKey, Range<usize>)>,
    busy: bool,
    picker_open: bool,
    notice: Option<String>,
    copied: bool,
    page_bounds: Rc<Cell<gpui::Bounds<gpui::Pixels>>>,
}

impl Reader {
    pub(crate) fn new(
        paths: Vec<PathBuf>,
        worker: Worker,
        events: async_channel::Receiver<Event>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.spawn_in(window, async move |this, cx| {
            while let Ok(event) = events.recv().await {
                if this
                    .update_in(cx, |this, window, cx| this.receive(event, window, cx))
                    .is_err()
                {
                    return;
                }
            }
            let _ = this.update_in(cx, |this, _, cx| {
                this.notice = Some(
                    "The PDF renderer stopped unexpectedly. Please reopen the application.".into(),
                );
                this.busy = false;
                cx.notify();
            });
        })
        .detach();
        let focus = cx.focus_handle();
        focus.focus(window);
        let mut reader = Self {
            focus,
            worker,
            tabs: Tabs::default(),
            generation: 0,
            sidebar: true,
            image: None,
            image_key: None,
            pages: PageCache::new(8, 64 * 1024 * 1024, PageImage::byte_len),
            thumbnails: PageCache::new(64, 16 * 1024 * 1024, PageImage::byte_len),
            texts: PageCache::new(32, 16 * 1024 * 1024, |text| match text {
                Ok(text) => text.byte_len(),
                Err(message) => message.len(),
            }),
            text: None,
            selection: None,
            last_request: None,
            busy: false,
            picker_open: false,
            notice: None,
            copied: false,
            page_bounds: Rc::new(Cell::new(gpui::Bounds::default())),
        };
        reader.open_paths(paths, window, cx);
        reader
    }

    fn receive(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        if event.generation() != self.generation {
            return;
        }
        let Some(tab) = self.tabs.current_mut() else {
            return;
        };
        match event {
            Event::Metadata { count, .. } => tab.count = count,
            Event::Page { view, raster, .. } => {
                self.busy = false;
                match PageImage::new(raster) {
                    Ok(image) => {
                        tab.error = None;
                        if let Some(offset) = tab.restore_scroll.take() {
                            tab.scroll.set_offset(offset);
                        }
                        self.cache_current_page(window);
                        self.image = Some(image);
                        self.image_key = Some(view);
                    }
                    Err(error) => tab.error = Some(error.to_string()),
                }
            }
            Event::Text { key, text, .. } => {
                if key == (tab.session, tab.page) {
                    self.text = Some(text.clone());
                    self.texts.insert(key, text);
                }
            }
            Event::Thumbnail { page, raster, .. } => match PageImage::new(raster) {
                Ok(image) => {
                    for old in self.thumbnails.insert((tab.session, page), image) {
                        old.release(window);
                    }
                }
                Err(_) => {
                    tab.failed_thumbnails.insert(page);
                }
            },
            Event::ThumbnailFailed { page, .. } => {
                tab.failed_thumbnails.insert(page);
            }
            Event::Error { message, .. } => {
                tab.error = Some(message);
                self.busy = false;
            }
        }
        cx.notify();
    }

    fn cache_current_page(&mut self, window: &mut Window) {
        if let Some(image) = self.image.take() {
            if let Some(key) = self.image_key.take() {
                for old in self.pages.insert(key, image) {
                    old.release(window);
                }
            } else {
                image.release(window);
            }
        }
    }

    fn reset_request(&mut self, window: &mut Window) {
        self.cache_current_page(window);
        self.generation = self.worker.cancel();
        self.last_request = None;
        self.page_bounds.set(gpui::Bounds::default());
        self.text = None;
        self.selection = None;
        self.copied = false;
        self.busy = false;
    }
}
