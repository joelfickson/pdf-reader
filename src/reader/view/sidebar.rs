use super::super::Reader;
use gpui::{Context, FontWeight, IntoElement, div, img, prelude::*, px, rgb, uniform_list};
use std::ops::Range;

impl Reader {
    pub(in crate::reader) fn sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.tabs.current().map_or(0, |tab| tab.count);
        let sidebar_scroll = self
            .tabs
            .current()
            .map(|tab| tab.sidebar_scroll.clone())
            .unwrap_or_default();
        div()
            .w(px(184.0))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf2f3ee))
            .border_r_1()
            .border_color(rgb(0xd9d9d5))
            .child(
                div()
                    .h(px(54.0))
                    .flex_shrink_0()
                    .px(px(20.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Pages"))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(0x646970))
                            .child(format!("{} pages", count)),
                    ),
            )
            .child(
                uniform_list(
                    "thumbnails",
                    count,
                    cx.processor(|this, range: Range<usize>, window, cx| {
                        if let Some(tab) = this.tabs.current_mut()
                            && tab.visible != range
                        {
                            tab.visible = range.clone();
                            this.request(window);
                        }
                        let session = this.tabs.current().map_or(0, |tab| tab.session);
                        let current_page = this.tabs.current().map_or(0, |tab| tab.page);
                        range
                            .map(|index| {
                                let preview = if let Some(thumbnail) =
                                    this.thumbnails.get(&(session, index))
                                {
                                    let height =
                                        (132.0 * thumbnail.height / thumbnail.width).min(171.0);
                                    let width = height * thumbnail.width / thumbnail.height;
                                    img(thumbnail.image.clone())
                                        .w(px(width))
                                        .h(px(height))
                                        .into_any_element()
                                } else {
                                    div()
                                        .w(px(132.0))
                                        .h(px(171.0))
                                        .bg(rgb(0xfffefa))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(rgb(0x848982))
                                        .text_size(px(11.0))
                                        .child(
                                            if this.tabs.current().is_some_and(|tab| {
                                                tab.failed_thumbnails.contains(&index)
                                            }) {
                                                "No preview"
                                            } else {
                                                "Loading"
                                            },
                                        )
                                        .into_any_element()
                                };
                                div().id(index).h(px(226.0)).px(px(10.0)).pb(px(7.0)).child(
                                    div()
                                        .id("thumbnail")
                                        .size_full()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .justify_center()
                                        .gap(px(9.0))
                                        .cursor_pointer()
                                        .when(index == current_page, |item| item.bg(rgb(0xe1e7ef)))
                                        .hover(|style| style.bg(rgb(0xe5e8e2)))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.go(index, window, cx)
                                        }))
                                        .child(
                                            div()
                                                .h(px(171.0))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(preview),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .when(index == current_page, |label| {
                                                    label
                                                        .text_color(rgb(0x2857a5))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                })
                                                .child((index + 1).to_string()),
                                        ),
                                )
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .track_scroll(sidebar_scroll)
                .flex_1(),
            )
    }
}
