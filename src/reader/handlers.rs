use super::{
    Reader,
    actions::*,
    chrome::{Input, InspectorTab, Mode, Overlay},
    commands::Command,
};
use crate::ui::field::FieldKey;
use gpui::{Context, KeyDownEvent, Window};

impl Reader {
    pub(super) fn toggle_palette(
        &mut self,
        _: &TogglePalette,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_overlay(Overlay::Palette, cx);
    }

    pub(super) fn toggle_find(&mut self, _: &ToggleFind, _: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.current().is_none_or(|tab| tab.count == 0) {
            return;
        }
        if self.chrome.find_open && self.chrome.input == Input::Find {
            self.close_find(cx);
        } else {
            self.open_find(cx);
        }
    }

    pub(super) fn toggle_annotate(
        &mut self,
        _: &ToggleAnnotate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_mode(Mode::Annotate, window, cx);
    }

    pub(super) fn toggle_organize(
        &mut self,
        _: &ToggleOrganize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_mode(Mode::Organize, window, cx);
    }

    pub(super) fn toggle_inspector(
        &mut self,
        _: &ToggleInspector,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tab = self.chrome.inspector.unwrap_or(InspectorTab::Document);
        self.set_inspector(tab, window, cx);
    }

    pub(super) fn show_export(
        &mut self,
        _: &ShowExport,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_command(Command::Export, window, cx);
    }

    pub(super) fn show_print(
        &mut self,
        _: &ShowPrint,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_command(Command::Print, window, cx);
    }

    pub(super) fn show_settings(
        &mut self,
        _: &ShowSettings,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_overlay(Overlay::Settings, cx);
    }

    pub(super) fn show_shortcuts(
        &mut self,
        _: &ShowShortcuts,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_overlay(Overlay::Shortcuts, cx);
    }

    pub(super) fn go_to_page(&mut self, _: &GoToPage, _: &mut Window, cx: &mut Context<Self>) {
        self.start_page_input(cx);
    }

    pub(super) fn save(&mut self, _: &Save, window: &mut Window, cx: &mut Context<Self>) {
        self.run_command(Command::Save, window, cx);
    }

    pub(super) fn bookmark(&mut self, _: &Bookmark, window: &mut Window, cx: &mut Context<Self>) {
        self.run_command(Command::Bookmark, window, cx);
    }

    pub(super) fn toggle_split(
        &mut self,
        _: &ToggleSplit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_command(Command::SplitView, window, cx);
    }

    pub(super) fn toggle_present(
        &mut self,
        _: &TogglePresent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_command(Command::Presentation, window, cx);
    }

    pub(super) fn first_page(
        &mut self,
        _: &FirstPage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.jump(0, None, window, cx);
    }

    pub(super) fn last_page(&mut self, _: &LastPage, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(count) = self.tabs.current().map(|tab| tab.count)
            && count > 0
        {
            self.jump(count - 1, None, window, cx);
        }
    }

    pub(super) fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        if self.chrome.overlay != Overlay::None {
            self.set_overlay(Overlay::None, cx);
        } else if self.chrome.input == Input::Page {
            self.chrome.input = Input::None;
        } else if self.chrome.find_open {
            self.close_find(cx);
        } else if self.chrome.mode != Mode::Read {
            self.chrome.mode = Mode::Read;
            self.request(window);
        } else if self.has_selection() {
            self.selection = None;
            self.copied = false;
        } else if self.chrome.inspector.is_some() {
            self.chrome.inspector = None;
            self.request(window);
        }
        cx.notify();
    }

    pub(super) fn key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;
        if self.chrome.input != Input::None
            && (keystroke.modifiers.platform || keystroke.modifiers.control)
            && keystroke.key == "v"
        {
            if let Some(pasted) = cx.read_from_clipboard().and_then(|item| item.text()) {
                let line = pasted.lines().next().unwrap_or_default().to_string();
                let edited = match self.chrome.input {
                    Input::Palette => self.chrome.palette.insert(&line),
                    Input::Find => self.chrome.find.insert(&line),
                    Input::Page => self.chrome.page_input.insert(&line),
                    Input::None => false,
                };
                if edited && self.chrome.input == Input::Palette {
                    self.chrome.palette_index = 0;
                }
                if edited && self.chrome.input == Input::Find {
                    self.schedule_search(cx);
                }
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        match self.chrome.input {
            Input::None => {
                if keystroke.key_char.as_deref() == Some("?")
                    && !keystroke.modifiers.platform
                    && !keystroke.modifiers.control
                {
                    self.set_overlay(Overlay::Shortcuts, cx);
                    cx.stop_propagation();
                }
            }
            Input::Palette => {
                let entries = self.palette_entries();
                match self.chrome.palette.handle(keystroke) {
                    FieldKey::Edited => self.chrome.palette_index = 0,
                    FieldKey::Down => {
                        if !entries.is_empty() {
                            self.chrome.palette_index =
                                (self.chrome.palette_index + 1) % entries.len();
                        }
                    }
                    FieldKey::Up => {
                        if !entries.is_empty() {
                            self.chrome.palette_index =
                                (self.chrome.palette_index + entries.len() - 1) % entries.len();
                        }
                    }
                    FieldKey::Submit => {
                        if let Some(entry) = entries.get(self.chrome.palette_index).copied() {
                            self.run_entry(entry, window, cx);
                        }
                    }
                    FieldKey::Cancel => self.set_overlay(Overlay::None, cx),
                    FieldKey::Moved => {}
                    FieldKey::Ignored => return,
                }
                cx.stop_propagation();
                cx.notify();
            }
            Input::Find => match self.chrome.find.handle(keystroke) {
                FieldKey::Ignored => {}
                FieldKey::Cancel => self.close_find(cx),
                key @ (FieldKey::Submit | FieldKey::Down | FieldKey::Up) => {
                    let forward = match key {
                        FieldKey::Submit => !keystroke.modifiers.shift,
                        other => other == FieldKey::Down,
                    };
                    self.restart_search(cx);
                    self.step_match(forward, window, cx);
                    cx.stop_propagation();
                    cx.notify();
                }
                FieldKey::Edited => {
                    self.schedule_search(cx);
                    cx.stop_propagation();
                }
                FieldKey::Moved => {
                    cx.stop_propagation();
                    cx.notify();
                }
            },
            Input::Page => match self.chrome.page_input.handle(keystroke) {
                FieldKey::Ignored => {}
                FieldKey::Submit => {
                    self.chrome.input = Input::None;
                    let count = self.tabs.current().map_or(0, |tab| tab.count);
                    match self.chrome.page_input.text.parse::<usize>() {
                        Ok(number) if (1..=count).contains(&number) => {
                            self.jump(number - 1, None, window, cx)
                        }
                        _ => self.unavailable_reason(
                            "No such page",
                            &format!("Enter a page from 1 to {count}."),
                            cx,
                        ),
                    }
                    cx.stop_propagation();
                    cx.notify();
                }
                FieldKey::Cancel => {
                    self.chrome.input = Input::None;
                    cx.notify();
                }
                _ => {
                    cx.stop_propagation();
                    cx.notify();
                }
            },
        }
    }
}
