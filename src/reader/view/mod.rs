mod canvas;
mod sidebar;
mod tabs;
mod text_layer;
mod toolbar;

use super::Reader;
use gpui::{
    Context, Div, IntoElement, Render, SharedString, Stateful, Window, div, prelude::*, px, rgb,
};

impl Reader {
    pub(in crate::reader) fn button<A: gpui::Action + Clone>(
        id: &'static str,
        label: impl Into<SharedString>,
        action: A,
        enabled: bool,
    ) -> Stateful<Div> {
        div()
            .id(id)
            .h(px(32.0))
            .px(px(10.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0))
            .text_size(px(13.0))
            .when(enabled, |button| {
                button
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(0xe8e8e3)).text_color(rgb(0x292c30)))
                    .on_click(move |_, window, cx| {
                        window.dispatch_action(Box::new(action.clone()), cx)
                    })
            })
            .when(!enabled, |button| button.text_color(rgb(0xa8aaa7)))
            .child(label.into())
    }
}

impl Render for Reader {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.request(window);
        let tab = self.tabs.current();
        let status = if self.busy {
            "Rendering…".into()
        } else if tab.is_some_and(|tab| tab.error.is_some()) {
            "Unable to display document".into()
        } else if tab.is_some_and(|tab| tab.count > 0) {
            tab.map(|tab| format!("Page {} of {}", tab.page + 1, tab.count))
                .unwrap_or_default()
        } else {
            "Ready to open a PDF".into()
        };
        let toolbar = self.toolbar().into_any_element();
        let sidebar = (self.sidebar && tab.is_some_and(|tab| tab.count > 0))
            .then(|| self.sidebar(cx).into_any_element());
        let canvas = self.canvas(cx).into_any_element();
        let tabs = self.tab_strip(cx).into_any_element();
        let selection_status = self.selection_status();
        div()
            .id("reader-root")
            .group("reader")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xeaeae6))
            .text_color(rgb(0x292c30))
            .font_family(if cfg!(target_os = "macos") {
                "Avenir Next"
            } else {
                "Segoe UI"
            })
            .text_size(px(13.0))
            .track_focus(&self.focus)
            .key_context("Reader")
            .on_action(cx.listener(Self::open))
            .on_action(cx.listener(Self::previous))
            .on_action(cx.listener(Self::next))
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
                cx.listener(|this, _, _, _| {
                    if let Some(selection) = &mut this.selection {
                        selection.dragging = false;
                    }
                }),
            )
            .on_mouse_up_out(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    if let Some(selection) = &mut this.selection {
                        selection.dragging = false;
                    }
                }),
            )
            .child(tabs)
            .child(toolbar)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .children(sidebar)
                    .child(canvas)
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .size_full()
                            .invisible()
                            .group_drag_over::<gpui::ExternalPaths>("reader", |style| {
                                style.visible()
                            })
                            .bg(gpui::rgba(0xedf1f7f2))
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(12.0))
                            .text_color(rgb(0x254f91))
                            .child(div().text_size(px(36.0)).child("Drop PDFs to open"))
                            .child("Each file opens in a new tab. Your place stays saved."),
                    ),
            )
            .child(
                div()
                    .h(px(28.0))
                    .flex_shrink_0()
                    .px(px(20.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .bg(rgb(0xf8f8f5))
                    .border_t_1()
                    .border_color(rgb(0xd9d9d5))
                    .text_size(px(11.0))
                    .text_color(rgb(0x646970))
                    .child(status)
                    .child(selection_status),
            )
    }
}
