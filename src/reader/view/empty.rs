use super::super::{Reader, actions::Open, chrome::Overlay, commands::modifier};
use crate::ui::{
    icon::Icon,
    kit::{Kind, RADIUS, icon, kbd, label, text_button},
};
use gpui::{Context, IntoElement, div, prelude::*, px};

impl Reader {
    pub(in crate::reader) fn empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        let m = modifier();
        let quick = |id: &'static str, glyph: Icon, title: &'static str, body: &'static str| {
            let hover = theme.hover;
            div()
                .id(id)
                .flex_1()
                .p(px(12.0))
                .flex()
                .flex_col()
                .gap(px(6.0))
                .rounded(px(RADIUS))
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .child(icon(glyph, theme.muted))
                .child(div().text_color(theme.ink).child(title))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(theme.muted)
                        .child(body),
                )
        };
        let key = |text: &'static str, keys: String| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(12.0))
                .text_color(theme.muted)
                .child(text)
                .child(kbd(&theme, keys))
        };
        div()
            .id("empty")
            .flex_1()
            .overflow_y_scroll()
            .flex()
            .justify_center()
            .child(
                div()
                    .w(px(560.0))
                    .mt(px(96.0))
                    .mb(px(48.0))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(170.0))
                            .rounded(px(RADIUS))
                            .border_1()
                            .border_dashed()
                            .border_color(theme.hairline)
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(10.0))
                            .text_color(theme.muted)
                            .child(icon(Icon::Upload, theme.faint).size(px(28.0)))
                            .child("Drop PDFs anywhere")
                            .child(
                                text_button("empty-open", "Open PDF", &theme, Kind::Primary)
                                    .on_click(|_, window, cx| {
                                        window.dispatch_action(Box::new(Open), cx)
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .mt(px(20.0))
                            .flex()
                            .gap(px(4.0))
                            .child(
                                quick(
                                    "quick-combine",
                                    Icon::Layout,
                                    "Combine files",
                                    "Merge PDFs and images into one PDF",
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| this.set_overlay(Overlay::Combine, cx),
                                )),
                            )
                            .child(
                                quick(
                                    "quick-sign",
                                    Icon::Pen,
                                    "Fill and sign",
                                    "Complete a form or add a signature",
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| this.unavailable("Filling and signing", cx),
                                )),
                            )
                            .child(
                                quick(
                                    "quick-convert",
                                    Icon::Export,
                                    "Convert",
                                    "PDF to images or text",
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| this.unavailable("Converting files", cx),
                                )),
                            ),
                    )
                    .child(label(&theme, "Recent").mt(px(28.0)).mb(px(8.0)))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme.faint)
                            .child("Recent files are not available yet."),
                    )
                    .child(label(&theme, "Keys").mt(px(28.0)).mb(px(8.0)))
                    .child(
                        div()
                            .flex()
                            .gap(px(24.0))
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap(px(8.0))
                                    .child(key("Command palette", format!("{m} K")))
                                    .child(key("Open", format!("{m} O"))),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap(px(8.0))
                                    .child(key("Settings", format!("{m} ,")))
                                    .child(key("All shortcuts", "?".into())),
                            ),
                    ),
            )
    }
}
