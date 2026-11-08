mod canvas;
mod dialogs;
mod empty;
mod floating;
mod inspector;
mod organize;
mod outline;
mod results;
mod settings;
mod sidebar;
mod text_layer;
mod titlebar;

use super::{
    Reader,
    chrome::{Input, Mode},
};
use crate::ui::{kit::clickable, theme::Theme};
use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px};

pub(super) const SIDEBAR_WIDTH: f32 = 220.0;
pub(super) const INSPECTOR_WIDTH: f32 = 280.0;
pub(super) const CANVAS_PADDING: f32 = 32.0;

impl Reader {
    pub(super) fn has_document(&self) -> bool {
        self.tabs.current().is_some_and(|tab| tab.count > 0)
    }

    pub(super) fn sidebar_visible(&self) -> bool {
        self.sidebar && self.has_document() && self.chrome.mode != Mode::Organize
    }

    pub(super) fn inspector_visible(&self) -> bool {
        self.chrome.inspector.is_some() && self.tabs.current().is_some()
    }

    fn stage(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        let content = if self.chrome.mode == Mode::Organize && self.has_document() {
            self.organize(cx).into_any_element()
        } else if self.tabs.current().is_none() {
            self.empty(cx).into_any_element()
        } else {
            self.canvas(window, cx).into_any_element()
        };
        clickable("stage")
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(content)
            .children(self.floating_layers(window, cx))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .size_full()
                    .invisible()
                    .group_drag_over::<gpui::ExternalPaths>("reader", |style| style.visible())
                    .bg(theme.drop)
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(20.0))
                            .text_color(theme.ink)
                            .child("Drop PDFs to open"),
                    )
                    .child(
                        div()
                            .text_color(theme.muted)
                            .child("Each file opens in a new tab."),
                    ),
            )
    }
}

impl Render for Reader {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.drive_scroll_probe(window, cx);
        self.request(window);
        let theme = self.chrome.theme;
        let sidebar = self
            .sidebar_visible()
            .then(|| self.sidebar(cx).into_any_element());
        let inspector = self
            .inspector_visible()
            .then(|| self.inspector(cx).into_any_element());
        let titlebar = self.titlebar(window, cx).into_any_element();
        let stage = self.stage(window, cx).into_any_element();
        let modal = self.modal(window, cx);
        let key_context = if self.chrome.input == Input::None {
            "Reader"
        } else {
            "Reader TextInput"
        };
        clickable("reader-root")
            .group("reader")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.canvas)
            .text_color(theme.ink)
            .font_family(Theme::ui_font())
            .text_size(px(13.0))
            .track_focus(&self.focus)
            .key_context(key_context)
            .on_action(cx.listener(Self::open))
            .on_action(cx.listener(Self::open_recent))
            .on_action(cx.listener(Self::clear_recent))
            .on_action(cx.listener(Self::previous))
            .on_action(cx.listener(Self::next))
            .on_action(cx.listener(Self::first_page))
            .on_action(cx.listener(Self::last_page))
            .on_action(cx.listener(Self::go_back))
            .on_action(cx.listener(Self::go_forward))
            .on_action(cx.listener(Self::zoom_in))
            .on_action(cx.listener(Self::zoom_out))
            .on_action(cx.listener(Self::fit))
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_action(cx.listener(Self::close))
            .on_action(cx.listener(Self::next_tab))
            .on_action(cx.listener(Self::previous_tab))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::clear_selection))
            .on_action(cx.listener(Self::dismiss))
            .on_action(cx.listener(Self::toggle_palette))
            .on_action(cx.listener(Self::toggle_find))
            .on_action(cx.listener(Self::toggle_annotate))
            .on_action(cx.listener(Self::toggle_organize))
            .on_action(cx.listener(Self::toggle_inspector))
            .on_action(cx.listener(Self::show_export))
            .on_action(cx.listener(Self::show_print))
            .on_action(cx.listener(Self::show_settings))
            .on_action(cx.listener(Self::show_shortcuts))
            .on_action(cx.listener(Self::go_to_page))
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(Self::bookmark))
            .on_action(cx.listener(Self::toggle_split))
            .on_action(cx.listener(Self::toggle_present))
            .on_key_down(cx.listener(Self::key_down))
            .on_drop(
                cx.listener(|this, paths: &gpui::ExternalPaths, window, cx| {
                    this.open_paths(paths.paths().to_vec(), window, cx)
                }),
            )
            .on_mouse_move(
                cx.listener(|this, event: &gpui::MouseMoveEvent, window, cx| {
                    if event.pressed_button == Some(gpui::MouseButton::Left)
                        && this
                            .selection
                            .as_ref()
                            .is_some_and(|selection| selection.dragging)
                    {
                        this.select_at(event.position, this.page_bounds.get(), false, window, cx);
                    }
                }),
            )
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if let Some(selection) = &mut this.selection {
                        selection.dragging = false;
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if let Some(selection) = &mut this.selection {
                        selection.dragging = false;
                        cx.notify();
                    }
                }),
            )
            .child(titlebar)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .children(sidebar)
                    .child(stage)
                    .children(inspector),
            )
            .children(modal)
    }
}
