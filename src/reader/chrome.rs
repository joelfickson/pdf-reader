use super::Reader;
use crate::ui::{
    field::TextField,
    kit::Status,
    theme::{Theme, ThemeChoice},
};
use gpui::{Context, SharedString, Window};
use std::{collections::BTreeSet, time::Duration};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Overlay {
    None,
    Palette,
    ZoomMenu,
    ViewMenu,
    Stamps,
    Sign,
    Export,
    Combine,
    Print,
    Settings,
    Shortcuts,
}

impl Overlay {
    pub fn is_modal(self) -> bool {
        matches!(
            self,
            Self::Palette
                | Self::Sign
                | Self::Export
                | Self::Combine
                | Self::Print
                | Self::Settings
                | Self::Shortcuts
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Mode {
    Read,
    Annotate,
    Organize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Panel {
    Pages,
    Outline,
    Bookmarks,
    Notes,
    Results,
    Attachments,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum InspectorTab {
    Selection,
    Document,
    Signatures,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Input {
    None,
    Palette,
    Find,
    Page,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Tool {
    Select,
    Highlight,
    Underline,
    Strike,
    Note,
    Draw,
    Shape,
    Text,
    Stamp,
}

#[derive(Clone)]
pub(super) struct Toast {
    pub id: u64,
    pub status: Status,
    pub title: SharedString,
    pub message: SharedString,
}

pub(super) struct Chrome {
    pub overlay: Overlay,
    pub mode: Mode,
    pub panel: Panel,
    pub inspector: Option<InspectorTab>,
    pub find_open: bool,
    pub find: TextField,
    pub palette: TextField,
    pub palette_index: usize,
    pub page_input: TextField,
    pub input: Input,
    pub toast: Option<Toast>,
    toast_counter: u64,
    pub theme_choice: ThemeChoice,
    pub theme: Theme,
    pub organize_selection: BTreeSet<usize>,
    pub tool: Tool,
    pub marker: usize,
    pub sign_tab: usize,
    pub export_format: usize,
    pub export_pages: usize,
    pub export_dpi: usize,
    pub include_notes: bool,
    pub print_pages: usize,
    pub print_scale: usize,
    pub print_sides: usize,
    pub combine_fit: usize,
    pub sign_ink: usize,
    pub settings_section: usize,
}

impl Chrome {
    pub fn new(theme: Theme) -> Self {
        Self {
            overlay: Overlay::None,
            mode: Mode::Read,
            panel: Panel::Pages,
            inspector: None,
            find_open: false,
            find: TextField::default(),
            palette: TextField::default(),
            palette_index: 0,
            page_input: TextField::digits(6),
            input: Input::None,
            toast: None,
            toast_counter: 0,
            theme_choice: ThemeChoice::System,
            theme,
            organize_selection: BTreeSet::new(),
            tool: Tool::Highlight,
            marker: 0,
            sign_tab: 0,
            export_format: 2,
            export_pages: 0,
            export_dpi: 1,
            include_notes: true,
            print_pages: 0,
            print_scale: 0,
            print_sides: 0,
            combine_fit: 0,
            sign_ink: 0,
            settings_section: 0,
        }
    }
}

impl Reader {
    pub(super) fn set_overlay(&mut self, overlay: Overlay, cx: &mut Context<Self>) {
        self.chrome.overlay = if self.chrome.overlay == overlay {
            Overlay::None
        } else {
            overlay
        };
        if self.chrome.overlay == Overlay::Palette {
            self.chrome.palette.clear();
            self.chrome.palette_index = 0;
            self.chrome.input = Input::Palette;
        } else if self.chrome.input == Input::Palette {
            self.chrome.input = Input::None;
        }
        cx.notify();
    }

    pub(super) fn set_mode(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        let next = if self.chrome.mode == mode {
            Mode::Read
        } else {
            mode
        };
        if next != Mode::Read && self.tabs.current().is_none_or(|tab| tab.count == 0) {
            self.unavailable_reason("Open a PDF first", "Page tools need an open document.", cx);
            return;
        }
        self.chrome.mode = next;
        self.chrome.overlay = Overlay::None;
        if next == Mode::Organize {
            self.chrome.organize_selection.clear();
            if let Some(tab) = self.tabs.current_mut() {
                tab.visible = 0..tab.count.min(48);
            }
        }
        if next == Mode::Annotate {
            self.chrome.panel = Panel::Notes;
            self.sidebar = true;
        }
        self.request(window);
        cx.notify();
    }

    pub(super) fn set_inspector(
        &mut self,
        tab: InspectorTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.chrome.inspector = if self.chrome.inspector == Some(tab) {
            None
        } else {
            Some(tab)
        };
        self.request(window);
        cx.notify();
    }

    pub(super) fn open_find(&mut self, cx: &mut Context<Self>) {
        self.chrome.find_open = true;
        self.chrome.input = Input::Find;
        self.chrome.overlay = Overlay::None;
        cx.notify();
    }

    pub(super) fn close_find(&mut self, cx: &mut Context<Self>) {
        self.chrome.find_open = false;
        if self.chrome.input == Input::Find {
            self.chrome.input = Input::None;
        }
        cx.notify();
    }

    pub(super) fn start_page_input(&mut self, cx: &mut Context<Self>) {
        if self.tabs.current().is_none_or(|tab| tab.count == 0) {
            return;
        }
        self.chrome.page_input.clear();
        self.chrome.input = Input::Page;
        self.chrome.overlay = Overlay::None;
        cx.notify();
    }

    pub(super) fn unavailable(&mut self, feature: &str, cx: &mut Context<Self>) {
        self.unavailable_reason(
            &format!("{feature} is not available yet"),
            "This part of the reader is designed but not built.",
            cx,
        );
    }

    pub(super) fn unavailable_reason(
        &mut self,
        title: &str,
        message: &str,
        cx: &mut Context<Self>,
    ) {
        self.show_toast(Status::Info, title, message, cx);
    }

    pub(super) fn show_toast(
        &mut self,
        status: Status,
        title: &str,
        message: &str,
        cx: &mut Context<Self>,
    ) {
        self.chrome.toast_counter += 1;
        let id = self.chrome.toast_counter;
        self.chrome.toast = Some(Toast {
            id,
            status,
            title: title.to_string().into(),
            message: message.to_string().into(),
        });
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(4500))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this
                    .chrome
                    .toast
                    .as_ref()
                    .is_some_and(|toast| toast.id == id)
                {
                    this.chrome.toast = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn set_theme_choice(
        &mut self,
        choice: ThemeChoice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.chrome.theme_choice = choice;
        self.chrome.theme = Theme::resolve(choice, window.appearance());
        cx.notify();
    }
}
