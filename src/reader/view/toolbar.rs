use super::super::{Reader, actions::*};
use gpui::{FontWeight, IntoElement, div, prelude::*, px, rgb};

impl Reader {
    pub(in crate::reader) fn toolbar(&self) -> impl IntoElement {
        let tab = self.tabs.current();
        let ready = tab.is_some_and(|tab| tab.count > 0);
        let page = tab.map_or(0, |tab| tab.page);
        let count = tab.map_or(0, |tab| tab.count);
        let zoom = tab.and_then(|tab| tab.zoom);
        div()
            .h(px(58.0))
            .flex_shrink_0()
            .px(px(20.0))
            .flex()
            .items_center()
            .gap(px(12.0))
            .bg(rgb(0xf8f8f5))
            .border_b_1()
            .border_color(rgb(0xd9d9d5))
            .child(
                Self::button("sidebar-toggle", "Pages", ToggleSidebar, ready)
                    .when(self.sidebar && ready, |button| {
                        button.bg(rgb(0xe8edf3)).text_color(rgb(0x2857a5))
                    }),
            )
            .child(
                div().flex_1().min_w_0().overflow_hidden().child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_ellipsis()
                        .child("PDF Reader"),
                ),
            )
            .child(div().w(px(96.0)).children(self.has_selection().then(|| {
                Self::button("copy", "Copy text", CopyText, true).text_color(rgb(0x2857a5))
            })))
            .child(
                Self::button("open", "Open PDF", Open, !self.picker_open).text_color(rgb(0x2857a5)),
            )
            .child(div().w(px(1.0)).h(px(20.0)).bg(rgb(0xd9d9d5)))
            .child(Self::button("previous", "‹", Previous, ready && page > 0).text_size(px(22.0)))
            .child(
                div()
                    .min_w(px(60.0))
                    .text_center()
                    .text_size(px(12.0))
                    .child(if ready {
                        format!("{} / {}", page + 1, count)
                    } else {
                        String::from("No PDF")
                    }),
            )
            .child(Self::button("next", "›", Next, ready && page + 1 < count).text_size(px(22.0)))
            .child(div().w(px(1.0)).h(px(20.0)).bg(rgb(0xd9d9d5)))
            .child(Self::button(
                "zoom-out",
                "−",
                ZoomOut,
                ready && zoom != Some(25),
            ))
            .child(
                div()
                    .min_w(px(42.0))
                    .text_center()
                    .text_size(px(12.0))
                    .child(zoom.map_or("Fit".into(), |zoom| format!("{zoom}%"))),
            )
            .child(Self::button(
                "zoom-in",
                "+",
                ZoomIn,
                ready && zoom != Some(400),
            ))
            .child(Self::button("fit", "Fit", Fit, ready))
    }
}
