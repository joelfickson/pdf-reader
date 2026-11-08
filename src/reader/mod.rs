mod actions;
mod agent;
mod cache;
mod chrome;
mod column;
mod commands;
mod handlers;
mod history;
mod images;
mod jumps;
mod navigation;
mod outline;
#[cfg(debug_assertions)]
mod preview;
mod scheduling;
mod search;
mod selection;
mod tabs;
#[cfg(test)]
mod tests;
mod view;

use crate::pdf::{
    Event, ViewKey, Worker,
    text::{Link, PageText},
};
use crate::recent::RecentFiles;
use crate::ui::theme::Theme;
pub(crate) use actions::*;
use cache::PageCache;
use chrome::Chrome;
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
    last_request: Option<(ViewKey, Range<usize>, Vec<usize>)>,
    busy: bool,
    picker_open: bool,
    notice: Option<String>,
    copied: bool,
    page_bounds: Rc<Cell<gpui::Bounds<gpui::Pixels>>>,
    chrome: Chrome,
    search: Option<search::SearchState>,
    reveal: bool,
    recent: RecentFiles,
    pressed_link: Option<Link>,
    agent: agent::AgentPanel,
    scroll_probe: Option<crate::bench::ScrollProbe>,
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
        cx.observe_window_appearance(window, |this, window, cx| {
            this.chrome.theme = Theme::resolve(this.chrome.theme_choice, window.appearance());
            cx.notify();
        })
        .detach();
        cx.on_app_quit(|this, cx| {
            this.persist_recent(cx);
            async {}
        })
        .detach();
        let (recent, recent_error) = match RecentFiles::load() {
            Ok(recent) => (recent, None),
            Err(error) => (
                RecentFiles::default(),
                Some(format!("Recent files could not be loaded. {error:#}")),
            ),
        };
        let theme = Theme::resolve(Default::default(), window.appearance());
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
            chrome: Chrome::new(theme),
            search: None,
            reveal: false,
            recent,
            pressed_link: None,
            agent: agent::AgentPanel::default(),
            scroll_probe: crate::bench::scrolling().then(Default::default),
        };
        reader.notice = recent_error;
        cx.set_menus(crate::app::menus(reader.recent.entries()));
        #[cfg(debug_assertions)]
        reader.preview_theme(window, cx);
        reader.open_paths(paths, window, cx);
        reader
    }

    fn receive(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        match event.generation() {
            None => {
                if let Event::Outline { session, outline } = event {
                    if let Some(tab) = self
                        .tabs
                        .entries
                        .iter_mut()
                        .find(|tab| tab.session == session && tab.outline.is_none())
                    {
                        tab.outline = Some(outline::OutlineTree::new(outline));
                        cx.notify();
                    }
                } else {
                    self.receive_search(event, window, cx);
                }
                return;
            }
            Some(generation) if !self.current_result(generation, &event) => {
                self.keep_nearby(event, window, cx);
                return;
            }
            Some(_) => {}
        }
        let Some(tab) = self.tabs.current_mut() else {
            return;
        };
        match event {
            Event::Metadata { count, sizes, .. } => {
                tab.count = count;
                if let Some(sizes) = sizes
                    && tab.sizes.is_none()
                {
                    tab.sizes = Some(sizes);
                    tab.column = None;
                    tab.pending_scroll = tab.restore_scroll.is_none();
                }
                if tab.page >= count {
                    tab.page = count.saturating_sub(1);
                    self.reset_request(window);
                    self.request(window);
                }
            }
            Event::Page { view, raster, .. } => {
                crate::bench::mark("page_received");
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
                        self.reveal_match();
                        self.reveal_anchor();
                        #[cfg(debug_assertions)]
                        self.preview_after_page(window, cx);
                    }
                    Err(error) => tab.error = Some(error.to_string()),
                }
            }
            Event::Text { key, text, .. } => {
                if key == (tab.session, tab.page) {
                    self.text = Some(text.clone());
                    self.texts.insert(key, text);
                    self.reveal_match();
                    self.reveal_anchor();
                    #[cfg(debug_assertions)]
                    self.preview_after_text(window, cx);
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
            Event::SearchBatch { .. }
            | Event::SearchDone { .. }
            | Event::SearchFailed { .. }
            | Event::Outline { .. } => {}
        }
        cx.notify();
    }

    fn current_result(&self, generation: u64, event: &Event) -> bool {
        if generation != self.generation {
            return false;
        }
        let Some(tab) = self.tabs.current() else {
            return false;
        };
        match event {
            Event::Page { view, .. } => view.session == tab.session && view.page == tab.page,
            Event::Text { key, .. } => *key == (tab.session, tab.page),
            _ => true,
        }
    }

    fn keep_nearby(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        let open = |session: u64| self.tabs.entries.iter().any(|tab| tab.session == session);
        match event {
            Event::Page { view, raster, .. } if open(view.session) => {
                if let Ok(image) = PageImage::new(raster) {
                    for old in self.pages.insert(view, image) {
                        old.release(window);
                    }
                    cx.notify();
                }
            }
            Event::Text { key, text, .. } if open(key.0) => {
                self.texts.insert(key, text);
                cx.notify();
            }
            _ => {}
        }
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
