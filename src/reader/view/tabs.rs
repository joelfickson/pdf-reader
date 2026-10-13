use super::super::{Reader, actions::Open};
use gpui::{Context, IntoElement, div, prelude::*, px, rgb};

impl Reader {
    pub(in crate::reader) fn tab_strip(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("document-tabs")
            .h(px(43.0))
            .flex_shrink_0()
            .flex()
            .overflow_x_scroll()
            .px(px(12.0))
            .gap(px(4.0))
            .bg(rgb(0xeeefe9))
            .border_b_1()
            .border_color(rgb(0xd9d9d5))
            .children(self.tabs.entries.iter().enumerate().map(|(index, tab)| {
                let active = index == self.tabs.active;
                div()
                    .id(("tab", tab.session))
                    .h_full()
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .w(px(235.0))
                    .border_b_2()
                    .border_color(if active { rgb(0x2857a5) } else { rgb(0xeeefe9) })
                    .when(active, |item| item.bg(rgb(0xf8f8f5)))
                    .child(
                        div()
                            .id("activate")
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .text_size(px(12.0))
                            .when(active, |item| item.text_color(rgb(0x2857a5)))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.activate_tab(index, window, cx)
                            }))
                            .child(div().text_ellipsis().child(tab.title())),
                    )
                    .child(
                        div()
                            .id("close")
                            .w(px(28.0))
                            .h(px(28.0))
                            .mr(px(6.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_size(px(18.0))
                            .text_color(rgb(0x646970))
                            .hover(|style| style.bg(rgb(0xe1e3dd)))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.close_tab(index, window, cx)
                            }))
                            .child("×"),
                    )
            }))
            .child(
                Self::button("new-tab", "+", Open, !self.picker_open)
                    .h(px(40.0))
                    .text_size(px(22.0)),
            )
    }
}
