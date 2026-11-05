use super::{
    Reader,
    chrome::{InspectorTab, Mode, Overlay, Panel},
    selection::Selection,
};
use crate::ui::theme::ThemeChoice;
use gpui::{Context, Window};
use std::sync::atomic::{AtomicBool, Ordering};

static APPLIED: AtomicBool = AtomicBool::new(false);
static SELECTED: AtomicBool = AtomicBool::new(false);

fn requested() -> Option<String> {
    std::env::var("PDF_READER_PREVIEW").ok()
}

impl Reader {
    pub(super) fn preview_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match std::env::var("PDF_READER_THEME").as_deref() {
            Ok("light") => self.set_theme_choice(ThemeChoice::Light, window, cx),
            Ok("dark") => self.set_theme_choice(ThemeChoice::Dark, window, cx),
            _ => {}
        }
    }

    pub(super) fn preview_after_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = requested() else {
            return;
        };
        if APPLIED.swap(true, Ordering::SeqCst) {
            return;
        }
        match state.as_str() {
            "palette" => self.set_overlay(Overlay::Palette, cx),
            "palette-query" => {
                self.set_overlay(Overlay::Palette, cx);
                self.chrome.palette.insert("zoom");
            }
            "find" => {
                self.open_find(cx);
                self.chrome.find.insert("capacity");
            }
            "page-input" => {
                self.start_page_input(cx);
                self.chrome.page_input.insert("2");
            }
            "zoom-menu" => self.set_overlay(Overlay::ZoomMenu, cx),
            "view-menu" => self.set_overlay(Overlay::ViewMenu, cx),
            "annotate" => self.set_mode(Mode::Annotate, window, cx),
            "stamps" => {
                self.set_mode(Mode::Annotate, window, cx);
                self.chrome.tool = super::chrome::Tool::Stamp;
                self.set_overlay(Overlay::Stamps, cx);
            }
            "outline" => self.chrome.panel = Panel::Outline,
            "results" => {
                self.chrome.panel = Panel::Results;
                self.open_find(cx);
                self.chrome.find.insert("capacity");
            }
            "inspector-document" => self.set_inspector(InspectorTab::Document, window, cx),
            "inspector-selection" => self.set_inspector(InspectorTab::Selection, window, cx),
            "inspector-signatures" => self.set_inspector(InspectorTab::Signatures, window, cx),
            "organize" => {
                self.set_mode(Mode::Organize, window, cx);
                self.chrome.organize_selection.extend([1, 2]);
            }
            "sign" => self.set_overlay(Overlay::Sign, cx),
            "sign-type" => {
                self.chrome.sign_tab = 1;
                self.set_overlay(Overlay::Sign, cx);
            }
            "export" => self.set_overlay(Overlay::Export, cx),
            "combine" => self.set_overlay(Overlay::Combine, cx),
            "print" => self.set_overlay(Overlay::Print, cx),
            "settings" => self.set_overlay(Overlay::Settings, cx),
            "settings-reading" => {
                self.chrome.settings_section = 1;
                self.set_overlay(Overlay::Settings, cx);
            }
            "shortcuts" => self.set_overlay(Overlay::Shortcuts, cx),
            "toast" => self.unavailable("Bookmarking pages", cx),
            "notice" => {
                self.notice = Some("Only PDF files can be opened. Other items were skipped.".into())
            }
            "no-sidebar" => self.sidebar = false,
            _ => {}
        }
        cx.notify();
    }

    pub(super) fn preview_after_text(&mut self, cx: &mut Context<Self>) {
        let Some(state) = requested() else {
            return;
        };
        if !matches!(state.as_str(), "selection" | "inspector-selection") {
            return;
        }
        if SELECTED.swap(true, Ordering::SeqCst) {
            return;
        }
        if let Some(Ok(text)) = &self.text {
            let end = text.glyphs.len().min(60);
            self.selection = Some(Selection {
                anchor: 0,
                head: end,
                dragging: false,
            });
            cx.notify();
        }
    }
}
