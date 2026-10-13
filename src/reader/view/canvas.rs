use super::super::{Reader, actions::Open};
use gpui::{Context, IntoElement, div, prelude::*, px, rgb};

impl Reader {
    pub(in crate::reader) fn canvas(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let content = if let Some(error) = self.tabs.current().and_then(|tab| tab.error.as_ref()) {
            div()
                .max_w(px(520.0))
                .p(px(36.0))
                .flex()
                .flex_col()
                .gap(px(16.0))
                .child(
                    div()
                        .text_size(px(25.0))
                        .child("Could not display this PDF"),
                )
                .child(div().text_color(rgb(0x646970)).child(error.clone()))
                .child(
                    Self::button("error-open", "Open another PDF", Open, !self.picker_open)
                        .text_color(rgb(0x2857a5)),
                )
                .into_any_element()
        } else if self.image.is_some() {
            self.page_element(cx).into_any_element()
        } else if self.tabs.current().is_some() {
            div()
                .p(px(64.0))
                .text_color(rgb(0x646970))
                .child("Opening page…")
                .into_any_element()
        } else {
            div()
                .w(px(480.0))
                .mt(px(130.0))
                .flex()
                .flex_col()
                .gap(px(20.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(0x646970))
                        .child("PDF READER"),
                )
                .child(
                    div()
                        .font_family(if cfg!(target_os = "macos") {
                            "Iowan Old Style"
                        } else {
                            "Palatino Linotype"
                        })
                        .text_size(px(48.0))
                        .line_height(px(54.0))
                        .child("A little room\nto read."),
                )
                .child(
                    div()
                        .text_size(px(15.0))
                        .text_color(rgb(0x646970))
                        .child("Drop a PDF here, or choose a file to start reading."),
                )
                .child(
                    div().flex().child(
                        Self::button("empty-open", "Open PDF", Open, !self.picker_open)
                            .bg(rgb(0x2857a5))
                            .text_color(rgb(0xffffff)),
                    ),
                )
                .child(div().text_size(px(11.0)).text_color(rgb(0x646970)).child(
                    if cfg!(target_os = "macos") {
                        "Command + O to open"
                    } else {
                        "Ctrl + O to open"
                    },
                ))
                .into_any_element()
        };
        div()
            .id("document-canvas")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_scroll()
            .when_some(self.tabs.current(), |canvas, tab| {
                canvas.track_scroll(&tab.scroll)
            })
            .child(
                div()
                    .min_w_full()
                    .min_h_full()
                    .p(px(48.0))
                    .flex()
                    .items_start()
                    .child(div().mx_auto().flex_shrink_0().child(content)),
            )
    }
}
