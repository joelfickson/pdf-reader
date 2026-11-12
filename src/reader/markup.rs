use super::{
    Reader, TEXT_PAGES,
    actions::*,
    chrome::{Mode, Tool},
    selection::Selection,
};
use crate::pdf::{
    SaveJob,
    markup::{self, Markup, MarkupKind, PageMarks},
};
use crate::ui::{kit::Status, theme::Marker};
use gpui::{Context, PromptLevel, Window};

pub(super) const MAX_UNSAVED_MARKUPS: usize = 2_000;
const QUOTE_CHARACTERS: usize = 240;

struct Refusal {
    status: Status,
    title: &'static str,
    message: String,
}

impl Tool {
    pub(super) fn markup(self) -> Option<MarkupKind> {
        match self {
            Self::Highlight => Some(MarkupKind::Highlight),
            Self::Underline => Some(MarkupKind::Underline),
            Self::Strike => Some(MarkupKind::Strikeout),
            _ => None,
        }
    }
}

fn quote(parts: &[String]) -> String {
    let joined = parts
        .iter()
        .flat_map(|part| part.split_whitespace())
        .collect::<Vec<_>>()
        .join(" ");
    if joined.chars().count() <= QUOTE_CHARACTERS {
        return joined;
    }
    let mut shortened: String = joined.chars().take(QUOTE_CHARACTERS).collect();
    shortened.push('…');
    shortened
}

impl Reader {
    pub(super) fn unsaved_tabs(&self) -> usize {
        self.tabs
            .entries
            .iter()
            .filter(|tab| tab.modified())
            .count()
    }

    fn collect_marks(&self, selection: &Selection) -> Result<(Vec<PageMarks>, String), Refusal> {
        let pages = selection.pages();
        let span = pages.end() - pages.start() + 1;
        if span > TEXT_PAGES {
            return Err(Refusal {
                status: Status::Warn,
                title: "Selection is too long to mark",
                message: format!(
                    "It spans {span} pages. Marking works across at most {TEXT_PAGES} pages at a time."
                ),
            });
        }
        let mut marks = Vec::new();
        let mut parts = Vec::new();
        for page in pages {
            match self.page_text(page) {
                Some(Ok(text)) => {
                    let range = selection.on_page(page, text.glyphs.len());
                    let rects = markup::lines(&text.glyphs[range.clone()]);
                    if !rects.is_empty() {
                        marks.push(PageMarks { page, rects });
                        parts.push(text.content(range));
                    }
                }
                Some(Err(message)) => {
                    return Err(Refusal {
                        status: Status::Warn,
                        title: "Selection could not be marked",
                        message: format!("Page {}: {message}", page + 1),
                    });
                }
                None => {
                    return Err(Refusal {
                        status: Status::Info,
                        title: "Selection is still loading",
                        message: format!(
                            "The text of page {} has not loaded yet. Try again in a moment.",
                            page + 1
                        ),
                    });
                }
            }
        }
        if marks.is_empty() {
            return Err(Refusal {
                status: Status::Info,
                title: "Nothing to mark",
                message: "The selection has no visible text.".into(),
            });
        }
        Ok((marks, quote(&parts)))
    }

    pub(super) fn mark_selection(
        &mut self,
        kind: MarkupKind,
        color: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(selection) = self
            .selection
            .clone()
            .filter(|selection| !selection.is_empty())
        else {
            self.show_toast(
                Status::Info,
                "Select text first",
                &format!("{} applies to the selected text.", kind.label()),
                cx,
            );
            return;
        };
        let Some(tab) = self.tabs.current() else {
            return;
        };
        if tab.saving {
            self.show_toast(
                Status::Info,
                "Still saving",
                "Wait for the save to finish, then mark the text again.",
                cx,
            );
            return;
        }
        if tab.unsaved().len() >= MAX_UNSAVED_MARKUPS {
            self.show_toast(
                Status::Warn,
                "Save before adding more",
                &format!("A document holds at most {MAX_UNSAVED_MARKUPS} unsaved marks."),
                cx,
            );
            return;
        }
        match self.collect_marks(&selection) {
            Ok((pages, quote)) => {
                if let Some(tab) = self.tabs.current_mut() {
                    tab.markups.push(Markup {
                        kind,
                        color: Marker::ALL[color % Marker::ALL.len()],
                        quote,
                        pages,
                    });
                    tab.undone.clear();
                }
                self.selection = None;
                self.copied = false;
                cx.notify();
            }
            Err(refusal) => self.show_toast(refusal.status, refusal.title, &refusal.message, cx),
        }
    }

    pub(super) fn finish_drag(&mut self, cx: &mut Context<Self>) {
        let Some(selection) = self.selection.as_mut() else {
            return;
        };
        let dragged = std::mem::take(&mut selection.dragging);
        cx.notify();
        if dragged
            && self.chrome.mode == Mode::Annotate
            && self.has_selection()
            && let Some(kind) = self.chrome.tool.markup()
        {
            self.mark_selection(kind, self.chrome.marker, cx);
        }
    }

    pub(super) fn choose_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        self.chrome.tool = tool;
        if let Some(kind) = tool.markup()
            && self.has_selection()
        {
            self.mark_selection(kind, self.chrome.marker, cx);
        }
        cx.notify();
    }

    pub(super) fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.current_mut() else {
            return;
        };
        if tab.saving || tab.unsaved().is_empty() {
            return;
        }
        if let Some(markup) = tab.markups.pop() {
            tab.undone.push(markup);
            cx.notify();
        }
    }

    pub(super) fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.current_mut() else {
            return;
        };
        if tab.saving {
            return;
        }
        if let Some(markup) = tab.undone.pop() {
            tab.markups.push(markup);
            cx.notify();
        }
    }

    pub(super) fn save_document(&mut self, cx: &mut Context<Self>) {
        if self.tabs.current().is_some_and(|tab| !tab.modified()) {
            self.show_toast(
                Status::Info,
                "Nothing to save",
                "This document has no unsaved changes.",
                cx,
            );
            return;
        }
        self.save_tab(self.tabs.active, cx);
    }

    fn save_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.entries.get_mut(index) else {
            return;
        };
        if tab.saving || !tab.modified() {
            return;
        }
        tab.saving = true;
        self.worker.save(SaveJob {
            session: tab.session,
            path: tab.path.clone(),
            markups: tab.unsaved().to_vec(),
        });
        cx.notify();
    }

    pub(super) fn finish_save(
        &mut self,
        session: u64,
        result: Result<(), String>,
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
        tab.saving = false;
        let close = std::mem::take(&mut tab.close_after_save);
        let title = tab.title();
        match result {
            Ok(()) => {
                tab.saved = tab.markups.len();
                tab.undone.clear();
                if close {
                    self.close_tab_now(index, window, cx);
                } else {
                    self.refresh_document(index, window);
                }
                self.show_toast(
                    Status::Info,
                    "Saved",
                    &format!("Changes written to {title}."),
                    cx,
                );
            }
            Err(message) => self.show_toast(
                Status::Warn,
                "Changes could not be saved",
                &format!("{title}: {message}"),
                cx,
            ),
        }
    }

    fn refresh_document(&mut self, index: usize, window: &mut Window) {
        let active = index == self.tabs.active;
        let session = self.tabs.entries[index].session;
        if active {
            let tab = &mut self.tabs.entries[index];
            if tab.restore_scroll.is_none() {
                tab.restore_scroll = Some(tab.scroll.offset());
            }
            self.reset_request(window);
        }
        for image in self.pages.remove_where(|key| key.session == session) {
            image.release(window);
        }
        for image in self.thumbnails.remove_where(|key| key.0 == session) {
            image.release(window);
        }
        self.tabs.entries[index].failed_thumbnails.clear();
        if active {
            self.request(window);
        }
    }

    pub(super) fn confirm_close(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.entries.get(index) else {
            return;
        };
        let session = tab.session;
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Save changes to {}?", tab.title()),
            Some("Your highlights and other marks are lost if you do not save them."),
            &["Save", "Discard", "Cancel"],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let Ok(answer) = answer.await else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(index) = this
                    .tabs
                    .entries
                    .iter()
                    .position(|tab| tab.session == session)
                else {
                    return;
                };
                match answer {
                    0 => {
                        this.tabs.entries[index].close_after_save = true;
                        this.save_tab(index, cx);
                    }
                    1 => {
                        this.tabs.entries[index].discard();
                        this.close_tab_now(index, window, cx);
                    }
                    _ => {}
                }
            });
        })
        .detach();
    }

    pub(super) fn confirm_discard(
        &mut self,
        action: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: fn(&mut Self, &mut Window, &mut Context<Self>),
    ) {
        let unsaved = self.unsaved_tabs();
        if unsaved == 0 {
            then(self, window, cx);
            return;
        }
        let detail = if unsaved == 1 {
            "One document has changes that have not been saved.".to_string()
        } else {
            format!("{unsaved} documents have changes that have not been saved.")
        };
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("{action} without saving?"),
            Some(&detail),
            &["Discard Changes", "Cancel"],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            if answer.await != Ok(0) {
                return;
            }
            let _ = this.update_in(cx, |this, window, cx| {
                for tab in &mut this.tabs.entries {
                    tab.discard();
                }
                then(this, window, cx);
            });
        })
        .detach();
    }

    pub(super) fn quit(&mut self, _: &Quit, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_discard("Quit", window, cx, |_, _, cx| cx.quit());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_collapse_whitespace_and_stay_bounded() {
        assert_eq!(
            quote(&["step\r\naside ".into(), " for  now".into()]),
            "step aside for now"
        );
        let long = quote(&["é".repeat(QUOTE_CHARACTERS + 50)]);
        assert_eq!(long.chars().count(), QUOTE_CHARACTERS + 1);
        assert!(long.ends_with('…'));
    }
}
