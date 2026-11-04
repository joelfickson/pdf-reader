use super::{super::Reader, CANVAS_PADDING};
use crate::reader::actions::Open;
use crate::ui::{
    icon::Icon,
    kit::{Kind, icon, text_button},
};
use gpui::{Context, IntoElement, div, prelude::*, px};

impl Reader {
    pub(in crate::reader) fn canvas(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        let content = if let Some(error) = self.tabs.current().and_then(|tab| tab.error.as_ref()) {
            div()
                .mt(px(120.0))
                .w(px(420.0))
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(icon(Icon::File, theme.faint))
                .child(
                    div()
                        .text_size(px(17.0))
                        .text_color(theme.ink)
                        .child("This PDF could not be displayed"),
                )
                .child(div().text_color(theme.muted).child(error.clone()))
                .child(
                    div().flex().child(
                        text_button("error-open", "Open another PDF", &theme, Kind::Primary)
                            .on_click(|_, window, cx| window.dispatch_action(Box::new(Open), cx)),
                    ),
                )
                .into_any_element()
        } else if self.image.is_some() {
            self.page_element(cx).into_any_element()
        } else {
            div()
                .mt(px(120.0))
                .text_color(theme.muted)
                .child("Opening page")
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
                    .pt(px(28.0))
                    .px(px(CANVAS_PADDING))
                    .pb(px(96.0))
                    .flex()
                    .items_start()
                    .child(div().mx_auto().flex_shrink_0().child(content)),
            )
    }
}
