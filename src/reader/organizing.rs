use super::{
    Reader,
    chrome::{Mode, Overlay},
};
use crate::pdf::{OrganizeJob, pages::PageOp};
use crate::ui::kit::Status;
use gpui::{Context, PathPromptOptions, Window};
use std::path::PathBuf;

fn pages_label(count: usize) -> String {
    if count == 1 {
        "1 page".into()
    } else {
        format!("{count} pages")
    }
}

impl Reader {
    fn selected_pages(&self) -> Vec<usize> {
        self.chrome.organize_selection.iter().copied().collect()
    }

    pub(super) fn can_organize(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(tab) = self.tabs.current() else {
            return false;
        };
        if tab.modified() || tab.saving {
            self.show_toast(
                Status::Info,
                "Save your changes first",
                "Page changes start from the saved PDF. Save or undo your marks, then try again.",
                cx,
            );
            return false;
        }
        true
    }

    pub(super) fn page_op(&mut self, op: PageOp, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.current_mut() else {
            return;
        };
        tab.page_ops.push_back(op.clone());
        self.worker.organize(OrganizeJob {
            session: tab.session,
            path: tab.path.clone(),
            op,
        });
        cx.notify();
    }

    pub(super) fn rotate_selected(&mut self, turns: u8, cx: &mut Context<Self>) {
        let pages = self.selected_pages();
        if !pages.is_empty() {
            self.page_op(PageOp::Rotate { pages, turns }, cx);
        }
    }

    pub(super) fn delete_selected(&mut self, cx: &mut Context<Self>) {
        let pages = self.selected_pages();
        let count = self.tabs.current().map_or(0, |tab| tab.count);
        if pages.is_empty() {
            return;
        }
        if pages.len() >= count {
            self.show_toast(
                Status::Info,
                "A PDF needs at least one page",
                "Leave at least one page unselected before deleting.",
                cx,
            );
            return;
        }
        self.page_op(PageOp::Delete { pages }, cx);
    }

    pub(super) fn insert_pages_from(&mut self, file: PathBuf, cx: &mut Context<Self>) {
        self.page_op(PageOp::Insert { file }, cx);
    }

    pub(super) fn extract_pages_to(&mut self, to: PathBuf, cx: &mut Context<Self>) {
        let pages = self.selected_pages();
        if !pages.is_empty() {
            self.page_op(PageOp::Extract { pages, to }, cx);
        }
    }

    pub(super) fn choose_file_to_insert(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.picker_open {
            return;
        }
        self.picker_open = true;
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Insert pages".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = chosen.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.picker_open = false;
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(file) = paths.into_iter().next() {
                            this.insert_pages_from(file, cx);
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => this.show_toast(
                        Status::Warn,
                        "The file picker could not be opened",
                        "Please try again.",
                        cx,
                    ),
                }
                this.focus.focus(window);
            });
        })
        .detach();
    }

    pub(super) fn choose_extract_destination(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.current() else {
            return;
        };
        if self.chrome.organize_selection.is_empty() || self.picker_open {
            return;
        }
        let folder = tab.path.parent().map(PathBuf::from).unwrap_or_default();
        let stem = tab
            .path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        self.picker_open = true;
        let chosen = cx.prompt_for_new_path(&folder, Some(&format!("{stem} pages.pdf")));
        cx.spawn_in(window, async move |this, cx| {
            let result = chosen.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.picker_open = false;
                match result {
                    Ok(Ok(Some(to))) => this.extract_pages_to(to, cx),
                    Ok(Ok(None)) => {}
                    _ => this.show_toast(
                        Status::Warn,
                        "The save dialog could not be opened",
                        "Please try again.",
                        cx,
                    ),
                }
                this.focus.focus(window);
            });
        })
        .detach();
    }

    pub(super) fn apply_page_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.current() else {
            return;
        };
        if tab.page_changes == 0 && tab.page_ops.is_empty() {
            self.chrome.mode = Mode::Read;
            self.request(window);
            cx.notify();
            return;
        }
        self.page_op(PageOp::Apply, cx);
    }

    pub(super) fn cancel_page_changes(&mut self, cx: &mut Context<Self>) {
        if self
            .tabs
            .current()
            .is_some_and(|tab| tab.page_changes > 0 || !tab.page_ops.is_empty())
        {
            self.page_op(PageOp::Cancel, cx);
        }
    }

    pub(super) fn rotate_current_page(&mut self, cx: &mut Context<Self>) {
        if !self.can_organize(cx) {
            return;
        }
        let Some(page) = self.tabs.current().map(|tab| tab.page) else {
            return;
        };
        self.page_op(
            PageOp::Rotate {
                pages: vec![page],
                turns: 1,
            },
            cx,
        );
        self.page_op(PageOp::Apply, cx);
    }

    fn reload_pages(&mut self, index: usize, count: Option<usize>, window: &mut Window) {
        let active = index == self.tabs.active;
        if active {
            self.reset_request(window);
        }
        let tab = &mut self.tabs.entries[index];
        let session = tab.session;
        if let Some(count) = count {
            tab.count = count;
            tab.page = tab.page.min(count.saturating_sub(1));
        }
        tab.sizes = None;
        tab.column = None;
        tab.outline = None;
        tab.failed_thumbnails.clear();
        tab.visible = 0..tab.count.min(48);
        for image in self.pages.remove_where(|key| key.session == session) {
            image.release(window);
        }
        for image in self.thumbnails.remove_where(|key| key.0 == session) {
            image.release(window);
        }
        self.texts.remove_where(|key| key.0 == session);
        if active {
            self.chrome.organize_selection.clear();
            self.clear_search();
            self.request(window);
        }
    }

    pub(super) fn finish_page_op(
        &mut self,
        session: u64,
        result: Result<usize, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self
            .tabs
            .entries
            .iter()
            .position(|tab| tab.session == session)
        else {
            return;
        };
        let tab = &mut self.tabs.entries[index];
        let Some(op) = tab.page_ops.pop_front() else {
            return;
        };
        let title = tab.title();
        let count = match result {
            Ok(count) => count,
            Err(message) => {
                let failed = match op {
                    PageOp::Rotate { .. } => "Pages could not be rotated",
                    PageOp::Delete { .. } => "Pages could not be deleted",
                    PageOp::Insert { .. } => "Pages could not be inserted",
                    PageOp::Extract { .. } => "Pages could not be extracted",
                    PageOp::Apply => "Page changes could not be saved",
                    PageOp::Cancel => "Page changes could not be discarded",
                };
                if !matches!(op, PageOp::Extract { .. }) {
                    self.reload_pages(index, None, window);
                }
                self.show_toast(Status::Warn, failed, &format!("{title}: {message}"), cx);
                return;
            }
        };
        match op {
            PageOp::Extract { pages, to } => self.show_toast(
                Status::Info,
                &format!("Extracted {}", pages_label(pages.len())),
                &format!("Written to {}.", to.display()),
                cx,
            ),
            PageOp::Apply => {
                tab.page_changes = 0;
                self.reload_pages(index, Some(count), window);
                if index == self.tabs.active && self.chrome.mode == Mode::Organize {
                    self.chrome.mode = Mode::Read;
                    self.chrome.overlay = Overlay::None;
                    self.request(window);
                }
                self.show_toast(
                    Status::Info,
                    "Pages saved",
                    &format!("{title} now has {}.", pages_label(count)),
                    cx,
                );
            }
            PageOp::Cancel => {
                tab.page_changes = 0;
                self.reload_pages(index, Some(count), window);
            }
            PageOp::Rotate { .. } | PageOp::Delete { .. } | PageOp::Insert { .. } => {
                tab.page_changes += 1;
                self.reload_pages(index, Some(count), window);
            }
        }
        cx.notify();
    }
}
