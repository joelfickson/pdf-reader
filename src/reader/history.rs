use super::{Reader, actions::*};
use crate::ui::kit::Status;
use gpui::{Context, Window};

impl Reader {
    pub(super) fn sync_recent(&mut self) {
        for tab in &self.tabs.entries {
            self.recent.update(&tab.path, tab.page, tab.zoom);
        }
    }

    pub(super) fn persist_recent(&mut self, cx: &mut Context<Self>) {
        self.sync_recent();
        if let Err(error) = self.recent.save() {
            self.show_toast(
                Status::Warn,
                "Recent files could not be saved",
                &format!("{error:#}"),
                cx,
            );
        }
        cx.set_menus(crate::app::menus(self.recent.entries()));
    }

    pub(super) fn open_recent(
        &mut self,
        action: &OpenRecent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !action.path.is_file() {
            self.recent.remove(&action.path);
            self.persist_recent(cx);
            self.show_toast(
                Status::Warn,
                "This file is no longer available",
                &format!(
                    "{} was moved or deleted, so it was removed from Recent.",
                    crate::app::recent_title(&action.path)
                ),
                cx,
            );
            cx.notify();
            return;
        }
        self.open_paths(vec![action.path.clone()], window, cx);
    }

    pub(super) fn clear_recent(&mut self, _: &ClearRecent, _: &mut Window, cx: &mut Context<Self>) {
        self.recent.clear();
        self.persist_recent(cx);
        cx.notify();
    }
}
