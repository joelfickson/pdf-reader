use super::{
    Reader,
    actions::*,
    chrome::{Layout, Panel},
    view,
};
use crate::preferences::{DefaultZoom, Preferences, Session, bookmark_title, fit_page_zoom};
use crate::ui::kit::Status;
use gpui::{Context, Window};
use std::{path::PathBuf, time::Duration};

const AUTOSAVE_DELAY: Duration = Duration::from_secs(2);

impl Reader {
    pub(super) fn update_preferences(
        &mut self,
        change: impl FnOnce(&mut Preferences),
        cx: &mut Context<Self>,
    ) {
        change(&mut self.preferences);
        self.preferences.continuous = self.chrome.layout == Layout::Continuous;
        self.preferences.theme = self.chrome.theme_choice;
        if let Err(error) = self.preferences.save(self.data.as_deref()) {
            self.show_toast(
                Status::Warn,
                "Settings could not be saved",
                &format!("{error:#}"),
                cx,
            );
        }
        cx.notify();
    }

    pub(super) fn opening_zoom(&self) -> (Option<u32>, bool) {
        match self.preferences.default_zoom {
            DefaultZoom::FitWidth => (None, false),
            DefaultZoom::Actual => (Some(100), false),
            DefaultZoom::FitPage => (None, true),
        }
    }

    fn page_points(&self) -> Option<(f32, f32)> {
        let tab = self.tabs.current()?;
        if let Some(size) = tab.sizes.as_ref().and_then(|sizes| sizes.get(tab.page)) {
            return Some(*size);
        }
        let image = self.image.as_ref()?;
        let scale = image.zoom_percent / 100.0 * 96.0 / 72.0;
        (scale > 0.0).then(|| (image.width / scale, image.height / scale))
    }

    pub(super) fn fit_page_now(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let height = self.viewport(window).1 - view::CANVAS_PADDING * 2.0;
        let available = (self.available_width(window), height);
        let Some(zoom) = self
            .page_points()
            .and_then(|page| fit_page_zoom(available, page))
        else {
            return false;
        };
        if let Some(tab) = self.tabs.current_mut() {
            tab.fit_page_pending = false;
        }
        self.set_zoom(Some(zoom), window, cx);
        true
    }

    pub(super) fn fit_page(&mut self, _: &FitPage, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.current().is_none_or(|tab| tab.count == 0) {
            return;
        }
        if !self.fit_page_now(window, cx)
            && let Some(tab) = self.tabs.current_mut()
        {
            tab.fit_page_pending = true;
        }
    }

    pub(super) fn actual_size(
        &mut self,
        _: &ActualSize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tabs.current().is_some_and(|tab| tab.count > 0) {
            self.set_zoom(Some(100), window, cx);
        }
    }

    pub(super) fn toggle_bookmark(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.current() else {
            return;
        };
        let (path, page) = (tab.path.clone(), tab.page);
        let content = match self.page_text(page) {
            Some(Ok(text)) => Some(text.content(0..text.glyphs.len().min(400))),
            _ => None,
        };
        let title = bookmark_title(page, content.as_deref());
        match self.bookmarks.toggle(&path, page, title) {
            Ok(added) => {
                if added {
                    self.chrome.panel = Panel::Bookmarks;
                    self.sidebar = true;
                }
                if let Err(error) = self.bookmarks.save(self.data.as_deref()) {
                    self.show_toast(
                        Status::Warn,
                        "Bookmarks could not be saved",
                        &format!("{error:#}"),
                        cx,
                    );
                }
            }
            Err(limit) => self.show_toast(
                Status::Warn,
                "Too many bookmarks",
                &format!("A document holds at most {limit} bookmarks. Remove one first."),
                cx,
            ),
        }
        cx.notify();
    }

    pub(super) fn restore_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.preferences.reopen_tabs {
            return;
        }
        let session = match Session::load(self.data.as_deref()) {
            Ok(session) => session,
            Err(error) => {
                self.notice = Some(format!("The last session could not be reopened. {error:#}"));
                return;
            }
        };
        let active = session.paths.get(session.active).cloned();
        let present: Vec<PathBuf> = session
            .paths
            .into_iter()
            .filter(|path| path.is_file())
            .collect();
        if present.is_empty() {
            return;
        }
        self.open_paths(present, window, cx);
        if let Some(index) = active.and_then(|path| {
            let path = path.canonicalize().unwrap_or(path);
            self.tabs.entries.iter().position(|tab| tab.path == path)
        }) {
            self.activate_tab(index, window, cx);
        }
    }

    pub(super) fn save_session(&self) -> anyhow::Result<()> {
        if !self.preferences.reopen_tabs {
            return Ok(());
        }
        Session {
            paths: self
                .tabs
                .entries
                .iter()
                .map(|tab| tab.path.clone())
                .collect(),
            active: self.tabs.active,
        }
        .save(self.data.as_deref())
    }

    pub(super) fn schedule_autosave(&mut self, cx: &mut Context<Self>) {
        if !self.preferences.autosave {
            return;
        }
        self.autosave_edits += 1;
        let scheduled = self.autosave_edits;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(AUTOSAVE_DELAY).await;
            let _ = this.update(cx, |this, cx| {
                if this.autosave_edits == scheduled && this.preferences.autosave {
                    this.autosave(cx);
                }
            });
        })
        .detach();
    }

    fn autosave(&mut self, cx: &mut Context<Self>) {
        if self.placing_signature().is_some() || self.signing.drawing {
            self.schedule_autosave(cx);
            return;
        }
        for index in 0..self.tabs.entries.len() {
            self.save_tab(index, cx);
        }
    }
}
