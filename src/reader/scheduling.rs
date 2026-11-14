use super::{
    Reader,
    chrome::{Layout, Mode},
    column::{COLUMN_TOP, Column, Spread},
    view,
};
use crate::pdf::{Job, ViewKey};
use gpui::{ScrollStrategy, Window, point, px};
use std::rc::Rc;

impl Reader {
    pub(super) fn available_width(&self, window: &Window) -> f32 {
        (f32::from(window.viewport_size().width)
            - if self.sidebar && self.chrome.mode != Mode::Organize {
                view::SIDEBAR_WIDTH
            } else {
                0.0
            }
            - if self.inspector_visible() {
                view::INSPECTOR_WIDTH
            } else {
                0.0
            }
            - if self.agent.open {
                view::AGENT_WIDTH
            } else {
                0.0
            }
            - view::CANVAS_PADDING * 2.0)
            .max(160.0)
    }

    pub(super) fn continuous(&self) -> bool {
        self.chrome.layout != Layout::Single
            && self.chrome.mode != Mode::Organize
            && self
                .tabs
                .current()
                .is_some_and(|tab| tab.sizes.is_some() && tab.error.is_none())
    }

    pub(super) fn column(&mut self, window: &Window) -> Option<Rc<Column>> {
        if !self.continuous() {
            return None;
        }
        let width = self.available_width(window).round() as u32;
        let spread = match self.chrome.layout {
            Layout::TwoPages => Spread::Pairs,
            Layout::CoverFirst => Spread::CoverFirst,
            Layout::Single | Layout::Continuous => Spread::Single,
        };
        let tab = self.tabs.current_mut()?;
        let key = (width.max(1), tab.zoom, spread);
        if tab.column.as_ref().is_none_or(|column| column.key != key) {
            let replaced = tab.column.is_some();
            tab.column = Some(Rc::new(Column::new(
                tab.sizes.as_ref()?,
                width,
                tab.zoom,
                spread,
            )));
            if replaced {
                tab.pending_scroll = true;
            }
        }
        tab.column.clone()
    }

    pub(super) fn viewport(&self, window: &Window) -> (f32, f32) {
        let Some(tab) = self.tabs.current() else {
            return (0.0, 0.0);
        };
        let bounds = f32::from(tab.scroll.bounds().size.height);
        let height = if bounds > 0.0 {
            bounds
        } else {
            f32::from(window.viewport_size().height)
        };
        (-f32::from(tab.scroll.offset().y) - COLUMN_TOP, height)
    }

    pub(super) fn page_top(&self) -> f32 {
        if !self.continuous() {
            return 0.0;
        }
        self.tabs.current().map_or(0.0, |tab| {
            tab.column
                .as_ref()
                .and_then(|column| column.tops.get(tab.page).copied())
                .unwrap_or(0.0)
        })
    }

    pub(super) fn scroll_to_page(&mut self, page: usize, column: &Column) {
        if let Some(tab) = self.tabs.current_mut() {
            let offset = tab.scroll.offset();
            tab.scroll
                .set_offset(point(offset.x, px(-column.scroll_to(page))));
            tab.restore_scroll = None;
            tab.pending_scroll = false;
        }
    }

    pub(super) fn focus_page(&mut self, page: usize, window: &mut Window) {
        let Some(tab) = self.tabs.current() else {
            return;
        };
        if page == tab.page || page >= tab.count {
            return;
        }
        self.cancel_request(window);
        if let Some(tab) = self.tabs.current_mut() {
            tab.page = page;
            tab.error = None;
            tab.sidebar_scroll
                .scroll_to_item(page, ScrollStrategy::Center);
        }
        self.request(window);
    }

    fn follow_scroll(&mut self, column: &Column, window: &mut Window) {
        let Some(tab) = self.tabs.current_mut() else {
            return;
        };
        if !std::mem::take(&mut tab.follow_scroll) {
            return;
        }
        let dragging = self
            .selection
            .as_ref()
            .is_some_and(|selection| selection.dragging);
        let (top, height) = self.viewport(window);
        let page = column.dominant(top, height);
        if !dragging {
            self.focus_page(page, window);
        }
    }

    pub(super) fn drive_scroll_probe(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
        if self.scroll_probe.is_none() {
            return;
        }
        let Some(column) = self.column(window) else {
            window.request_animation_frame();
            return;
        };
        let (top, height) = self.viewport(window);
        let Some(tab) = self.tabs.current() else {
            return;
        };
        let (session, zoom) = (tab.session, tab.zoom);
        let scale = (window.scale_factor() * 100.0).round() as u32;
        let blank = column.visible(top, height).any(|page| {
            let view = ViewKey {
                session,
                page,
                available_width: column.view_width(page),
                zoom,
                scale,
            };
            self.image_key.as_ref() != Some(&view) && !self.pages.contains_key(&view)
        });
        let end = (column.height + COLUMN_TOP + 96.0 - height).max(0.0);
        if let Some(probe) = self.scroll_probe.as_mut() {
            probe.frame(blank);
            if top + COLUMN_TOP >= end {
                probe.report(column.len(), cx);
                self.scroll_probe = None;
                return;
            }
        }
        if let Some(tab) = self.tabs.current_mut() {
            let offset = tab.scroll.offset();
            let next = (f32::from(-offset.y) + crate::bench::SCROLL_STEP_PX).min(end);
            tab.scroll.set_offset(point(offset.x, px(-next)));
            tab.follow_scroll = true;
        }
        window.request_animation_frame();
    }

    pub(super) fn request(&mut self, window: &mut Window) {
        let column = self.column(window);
        if let Some(column) = &column {
            if self.tabs.current().is_some_and(|tab| tab.pending_scroll) {
                let page = self.tabs.current().map_or(0, |tab| tab.page);
                self.scroll_to_page(page, column);
            }
            self.follow_scroll(column, window);
        }
        let Some(tab) = self.tabs.current() else {
            return;
        };
        if tab.error.is_some() {
            return;
        }
        let width = self.available_width(window);
        let scale = (window.scale_factor() * 100.0).round() as u32;
        let view_for = |page: usize| ViewKey {
            session: tab.session,
            page,
            available_width: match (&column, tab.zoom) {
                (Some(column), _) => column.view_width(page),
                (None, Some(_)) => 0,
                (None, None) => width.round() as u32,
            },
            zoom: tab.zoom,
            scale,
        };
        let view = view_for(tab.page);
        let nearby: Vec<usize> = match &column {
            Some(column) => {
                let (top, height) = self.viewport(window);
                column
                    .around(top, height)
                    .filter(|page| *page != tab.page)
                    .collect()
            }
            None => Vec::new(),
        };
        let visible = if self.sidebar_visible() || self.chrome.mode == Mode::Organize {
            tab.visible.clone()
        } else {
            0..0
        };
        let key = (view.clone(), visible.clone(), nearby.clone());
        if self.last_request.as_ref() == Some(&key) {
            return;
        }
        if self.busy
            && self
                .last_request
                .as_ref()
                .is_some_and(|(pending, _, around)| pending == &view && around == &nearby)
        {
            return;
        }
        let path = tab.path.clone();
        let session = tab.session;
        let sizes = tab.sizes.is_none();
        let thumbnails: Vec<_> = visible
            .clone()
            .filter(|index| {
                (tab.count == 0 || *index < tab.count)
                    && !self.thumbnails.contains_key(&(tab.session, *index))
                    && !tab.failed_thumbnails.contains(index)
            })
            .collect();
        let nearby_views: Vec<ViewKey> = nearby
            .iter()
            .map(|page| view_for(*page))
            .filter(|view| !self.pages.contains_key(view))
            .collect();
        let mut nearby_text: Vec<usize> = nearby
            .iter()
            .copied()
            .filter(|page| !self.texts.contains_key(&(session, *page)))
            .collect();
        for page in std::mem::take(&mut self.wanted_text) {
            if page != tab.page
                && page < tab.count
                && !nearby_text.contains(&page)
                && !self.texts.contains_key(&(session, page))
            {
                nearby_text.push(page);
            }
        }
        let text_key = (view.session, view.page);
        if self.text.is_none() {
            self.text = self.texts.get(&text_key).cloned();
        }
        if self.image_key.as_ref() != Some(&view)
            && let Some(image) = self.pages.take(&view)
        {
            self.cache_current_page(window);
            self.image = Some(image);
            self.image_key = Some(view.clone());
            if let Some(tab) = self.tabs.current_mut()
                && let Some(offset) = tab.restore_scroll.take()
            {
                tab.scroll.set_offset(offset);
            }
        }
        self.reveal_anchor();
        let outline = self.tabs.current().is_some_and(|tab| tab.outline.is_none());
        let render_page = self.image_key.as_ref() != Some(&view);
        let extract_text = self.text.is_none();
        self.last_request = Some(key);
        self.busy = render_page;
        if !render_page
            && !extract_text
            && thumbnails.is_empty()
            && !outline
            && !sizes
            && nearby_views.is_empty()
            && nearby_text.is_empty()
        {
            self.generation = self.worker.cancel();
            return;
        }
        self.generation = self.worker.submit(Job {
            generation: 0,
            path,
            view,
            render_page,
            extract_text,
            thumbnails,
            outline,
            sizes,
            nearby: nearby_views,
            nearby_text,
        });
    }
}
