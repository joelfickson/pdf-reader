use super::{Reader, view};
use crate::pdf::{Job, ViewKey};
use gpui::Window;

impl Reader {
    pub(super) fn request(&mut self, window: &mut Window) {
        let Some(tab) = self.tabs.current() else {
            return;
        };
        if tab.error.is_some() {
            return;
        }
        let width = (f32::from(window.viewport_size().width)
            - if self.sidebar && self.chrome.mode != super::chrome::Mode::Organize {
                view::SIDEBAR_WIDTH
            } else {
                0.0
            }
            - if self.inspector_visible() {
                view::INSPECTOR_WIDTH
            } else {
                0.0
            }
            - view::CANVAS_PADDING * 2.0)
            .max(160.0);
        let view = ViewKey {
            session: tab.session,
            page: tab.page,
            available_width: if tab.zoom.is_some() {
                0
            } else {
                width.round() as u32
            },
            zoom: tab.zoom,
            scale: (window.scale_factor() * 100.0).round() as u32,
        };
        let visible = if self.sidebar_visible() || self.chrome.mode == super::chrome::Mode::Organize
        {
            tab.visible.clone()
        } else {
            0..0
        };
        if self.last_request.as_ref() == Some(&(view.clone(), visible.clone())) {
            return;
        }
        if self.busy
            && self
                .last_request
                .as_ref()
                .is_some_and(|(pending, _)| pending == &view)
        {
            return;
        }
        let path = tab.path.clone();
        let thumbnails: Vec<_> = visible
            .clone()
            .filter(|index| {
                (tab.count == 0 || *index < tab.count)
                    && !self.thumbnails.contains_key(&(tab.session, *index))
                    && !tab.failed_thumbnails.contains(index)
            })
            .collect();
        let key = (view.session, view.page);
        if self.text.is_none() {
            self.text = self.texts.get(&key).cloned();
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
        let render_page = self.image_key.as_ref() != Some(&view);
        let extract_text = self.text.is_none();
        self.last_request = Some((view.clone(), visible));
        self.busy = render_page;
        if !render_page && !extract_text && thumbnails.is_empty() {
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
        });
    }
}
