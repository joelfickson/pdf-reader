use super::{Reader, actions::*};
use gpui::{Context, PathPromptOptions, ScrollStrategy, Window, point, px};
use std::path::PathBuf;

impl Reader {
    pub(super) fn open(&mut self, _: &Open, window: &mut Window, cx: &mut Context<Self>) {
        if self.picker_open {
            return;
        }
        self.picker_open = true;
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Open PDFs".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = paths.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.picker_open = false;
                match result {
                    Ok(Ok(Some(paths))) => this.open_paths(paths, window, cx),
                    Ok(Ok(None)) => {}
                    _ => {
                        this.notice =
                            Some("The file picker could not be opened. Please try again.".into())
                    }
                }
                this.focus.focus(window);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn open_paths(
        &mut self,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.notice = None;
        let mut target = None;
        for path in paths {
            if !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
                || path.is_dir()
            {
                self.notice =
                    Some("Only PDF files can be opened. Other items were skipped.".into());
                continue;
            }
            let existing = self.tabs.entries.len();
            let index = self.tabs.open(path);
            let opened = self.tabs.entries.len() > existing;
            let tab = &mut self.tabs.entries[index];
            if opened && let Some(entry) = self.recent.get(&tab.path) {
                tab.page = entry.page;
                tab.zoom = entry.zoom;
            }
            let path = tab.path.clone();
            self.recent.touch(&path);
            target = Some(index);
        }
        if let Some(index) = target {
            self.persist_recent(cx);
            self.activate_tab(index, window, cx);
        }
        cx.notify();
    }

    pub(super) fn activate_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.entries.len() {
            return;
        }
        if let Some(tab) = self.tabs.current_mut()
            && tab.restore_scroll.is_none()
        {
            tab.restore_scroll = Some(tab.scroll.offset());
        }
        self.sync_recent();
        self.reset_request(window);
        self.tabs.active = index;
        self.clear_search();
        self.restart_search(cx);
        window.set_window_title(&format!(
            "{} - Agentic PDF",
            self.tabs.entries[index].title()
        ));
        self.focus.focus(window);
        self.request(window);
        cx.notify();
    }

    pub(super) fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .tabs
            .entries
            .get(index)
            .is_some_and(|tab| tab.modified())
        {
            self.confirm_close(index, window, cx);
        } else {
            self.close_tab_now(index, window, cx);
        }
    }

    pub(super) fn close_tab_now(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.entries.len() {
            return;
        }
        self.sync_recent();
        let active = index == self.tabs.active;
        if active {
            self.reset_request(window);
        }
        if let Some(tab) = self.tabs.close(index) {
            self.worker.close(tab.session);
            for image in self.pages.remove_where(|key| key.session == tab.session) {
                image.release(window);
            }
            for image in self.thumbnails.remove_where(|key| key.0 == tab.session) {
                image.release(window);
            }
            self.texts.remove_where(|key| key.0 == tab.session);
        }
        if active {
            if self.tabs.current().is_some() {
                self.activate_tab(self.tabs.active, window, cx);
            } else {
                window.set_window_title("Agentic PDF");
                self.notice = None;
            }
        }
        self.persist_recent(cx);
        cx.notify();
    }

    pub(super) fn close(&mut self, _: &Close, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.current().is_some() {
            self.close_tab(self.tabs.active, window, cx);
        } else {
            window.remove_window();
        }
    }

    pub(super) fn next_tab(&mut self, _: &NextTab, window: &mut Window, cx: &mut Context<Self>) {
        if !self.tabs.entries.is_empty() {
            self.activate_tab((self.tabs.active + 1) % self.tabs.entries.len(), window, cx);
        }
    }

    pub(super) fn previous_tab(
        &mut self,
        _: &PreviousTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let count = self.tabs.entries.len();
        if count > 0 {
            self.activate_tab((self.tabs.active + count - 1) % count, window, cx);
        }
    }

    pub(super) fn go(&mut self, page: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.current() else {
            return;
        };
        if page >= tab.count || page == tab.page {
            return;
        }
        if let Some(column) = self.column(window) {
            self.focus_page(page, window);
            self.scroll_to_page(page, &column);
            cx.notify();
            return;
        }
        self.reset_request(window);
        if let Some(tab) = self.tabs.current_mut() {
            tab.page = page;
            tab.error = None;
            tab.scroll.set_offset(point(px(0.0), px(0.0)));
            tab.restore_scroll = None;
            tab.sidebar_scroll
                .scroll_to_item(page, ScrollStrategy::Center);
        }
        self.request(window);
        cx.notify();
    }

    pub(super) fn previous(&mut self, _: &Previous, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.current() {
            self.go(tab.page.saturating_sub(1), window, cx);
        }
    }
    pub(super) fn next(&mut self, _: &Next, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.current() {
            self.go(tab.page + 1, window, cx);
        }
    }
    fn change_zoom(&mut self, delta: i32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.current_mut() else {
            return;
        };
        let current = tab.zoom.unwrap_or_else(|| {
            self.image
                .as_ref()
                .map_or(100, |image| image.zoom_percent.round() as u32)
        });
        tab.zoom = Some((current as i32 + delta).clamp(25, 400) as u32);
        self.request(window);
        cx.notify();
    }
    pub(super) fn zoom_in(&mut self, _: &ZoomIn, window: &mut Window, cx: &mut Context<Self>) {
        self.change_zoom(25, window, cx);
    }
    pub(super) fn zoom_out(&mut self, _: &ZoomOut, window: &mut Window, cx: &mut Context<Self>) {
        self.change_zoom(-25, window, cx);
    }
    pub(super) fn fit(&mut self, _: &Fit, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.current_mut() {
            tab.zoom = None;
        }
        self.request(window);
        cx.notify();
    }
    pub(super) fn toggle_sidebar(
        &mut self,
        _: &ToggleSidebar,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sidebar = !self.sidebar;
        self.request(window);
        cx.notify();
    }
}
