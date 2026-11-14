use super::super::{Reader, chrome::Mode};
use crate::ui::{
    icon::Icon,
    kit::clickable,
    kit::{Kind, Tone, icon, icon_button, mono, separator, text_button},
};
use gpui::{Context, FontWeight, IntoElement, Window, div, prelude::*, px};

impl Reader {
    pub(in crate::reader) fn organize(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        let Some(tab) = self.tabs.current() else {
            return div().into_any_element();
        };
        let session = tab.session;
        let count = tab.count;
        let selected = self.chrome.organize_selection.len();
        let action =
            |id: &'static str,
             glyph: Icon,
             text: Option<&'static str>,
             tone: Tone,
             run: fn(&mut Reader, &mut Window, &mut Context<Reader>)| {
                icon_button(id, glyph, &theme, tone, selected > 0)
                    .children(text)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if !this.chrome.organize_selection.is_empty() {
                            run(this, window, cx)
                        }
                    }))
            };
        let toolbar =
            div()
                .h(px(52.0))
                .flex_shrink_0()
                .px(px(16.0))
                .flex()
                .items_center()
                .gap(px(4.0))
                .bg(theme.chrome)
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .mr(px(10.0))
                        .child("Organize pages"),
                )
                .child(
                    mono(format!("{selected} of {count} selected"))
                        .text_color(theme.muted)
                        .flex_1(),
                )
                .child(action(
                    "rotate-left",
                    Icon::RotateLeft,
                    None,
                    Tone::Plain,
                    |this, _, cx| this.rotate_selected(3, cx),
                ))
                .child(action(
                    "rotate-right",
                    Icon::RotateRight,
                    None,
                    Tone::Plain,
                    |this, _, cx| this.rotate_selected(1, cx),
                ))
                .child(action(
                    "extract",
                    Icon::Extract,
                    Some("Extract"),
                    Tone::Plain,
                    |this, window, cx| this.choose_extract_destination(window, cx),
                ))
                .child(
                    icon_button("insert", Icon::Plus, &theme, Tone::Plain, true)
                        .child("Insert file")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.choose_file_to_insert(window, cx)
                        })),
                )
                .child(action(
                    "delete",
                    Icon::Trash,
                    Some("Delete"),
                    Tone::Danger,
                    |this, _, cx| this.delete_selected(cx),
                ))
                .child(separator(&theme))
                .child(
                    text_button("organize-cancel", "Cancel", &theme, Kind::Quiet)
                        .h(px(28.0))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.set_mode(Mode::Organize, window, cx)
                        })),
                )
                .child(
                    text_button("organize-apply", "Apply", &theme, Kind::Primary)
                        .h(px(28.0))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.apply_page_changes(window, cx)),
                        ),
                );
        let pages = (0..count).map(|index| {
            let checked = self.chrome.organize_selection.contains(&index);
            let hairline = theme.hairline;
            clickable(("organize-page", index))
                .group("organize-page")
                .relative()
                .w(px(156.0))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.0))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.chrome.organize_selection.remove(&index) {
                        this.chrome.organize_selection.insert(index);
                    }
                    cx.notify();
                }))
                .child(
                    div()
                        .p(px(3.0))
                        .rounded(px(5.0))
                        .border_2()
                        .border_color(if checked {
                            theme.accent
                        } else {
                            gpui::transparent_black()
                        })
                        .when(!checked, |frame| {
                            frame.group_hover("organize-page", move |style| {
                                style.border_color(hairline)
                            })
                        })
                        .child(self.thumbnail_element(session, index, 132.0, 172.0)),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(10.0))
                        .right(px(14.0))
                        .size(px(20.0))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(if checked { theme.accent } else { theme.raised })
                        .shadow(theme.floating_shadow())
                        .child(
                            icon(
                                Icon::Check,
                                if checked {
                                    theme.on_accent
                                } else {
                                    theme.faint
                                },
                            )
                            .size(px(12.0)),
                        ),
                )
                .child(
                    mono((index + 1).to_string())
                        .text_size(px(11.0))
                        .text_color(if checked { theme.accent } else { theme.faint }),
                )
        });
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(toolbar)
            .child(
                clickable("organize-grid")
                    .flex_1()
                    .overflow_y_scroll()
                    .p(px(28.0))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(22.0))
                            .children(pages)
                            .child(
                                clickable("organize-add-pages")
                                    .w(px(156.0))
                                    .flex()
                                    .justify_center()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.choose_file_to_insert(window, cx)
                                    }))
                                    .child(
                                        div()
                                            .w(px(138.0))
                                            .h(px(178.0))
                                            .rounded(px(5.0))
                                            .border_1()
                                            .border_dashed()
                                            .border_color(theme.hairline)
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .justify_center()
                                            .gap(px(8.0))
                                            .text_size(px(12.0))
                                            .text_color(theme.muted)
                                            .child(icon(Icon::Plus, theme.muted))
                                            .child("Add pages from a file"),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}
