use super::super::{
    Reader,
    actions::{Open, OpenRecent},
    chrome::Overlay,
};
use crate::ui::{
    icon::Icon,
    kit::clickable,
    kit::{Kind, RADIUS, icon, label, mono, text_button},
};
use gpui::{AnyElement, Context, IntoElement, div, prelude::*, px};

const RECENT_ROWS: usize = 8;

impl Reader {
    pub(in crate::reader) fn empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        let quick = |id: &'static str, glyph: Icon, title: &'static str, body: &'static str| {
            let hover = theme.hover;
            clickable(id)
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
        clickable("empty")
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
                    .child(
                        label(&theme, "Restore last session")
                            .mt(px(28.0))
                            .mb(px(8.0)),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme.faint)
                            .child("Restoring sessions is not available yet."),
                    )
                    .child(label(&theme, "Recent").mt(px(28.0)).mb(px(8.0)))
                    .child(self.recent_list(cx)),
            )
    }

    fn recent_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.chrome.theme;
        let entries = self.recent.entries();
        if entries.is_empty() {
            return div()
                .text_size(px(12.0))
                .text_color(theme.faint)
                .child("Files you open appear here.")
                .into_any_element();
        }
        div()
            .flex()
            .flex_col()
            .children(
                entries
                    .iter()
                    .take(RECENT_ROWS)
                    .enumerate()
                    .map(|(index, entry)| {
                        let hover = theme.hover;
                        let path = entry.path.clone();
                        clickable(("recent-file", index))
                            .h(px(36.0))
                            .px(px(10.0))
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .rounded(px(RADIUS))
                            .cursor_pointer()
                            .hover(move |style| style.bg(hover))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_recent(&OpenRecent { path: path.clone() }, window, cx)
                            }))
                            .child(icon(Icon::File, theme.muted))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .text_color(theme.ink)
                                    .child(crate::app::recent_title(&entry.path)),
                            )
                            .child(
                                mono(format!("page {}", entry.page + 1))
                                    .text_size(px(12.0))
                                    .text_color(theme.faint),
                            )
                    }),
            )
            .into_any_element()
    }
}
