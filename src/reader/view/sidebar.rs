use super::{super::Reader, SIDEBAR_WIDTH};
use crate::reader::chrome::Panel;
use crate::ui::{
    icon::Icon,
    kit::clickable,
    kit::{RADIUS, empty_state, icon, label, mono},
    theme::Paper,
};
use gpui::{AnyElement, Context, IntoElement, div, img, prelude::*, px, uniform_list};
use std::ops::Range;

const THUMB_WIDTH: f32 = 108.0;
const THUMB_HEIGHT: f32 = 140.0;

impl Panel {
    const ALL: [Panel; 6] = [
        Panel::Pages,
        Panel::Outline,
        Panel::Bookmarks,
        Panel::Notes,
        Panel::Results,
        Panel::Attachments,
    ];

    fn icon(self) -> Icon {
        match self {
            Self::Pages => Icon::Pages,
            Self::Outline => Icon::Outline,
            Self::Bookmarks => Icon::Bookmark,
            Self::Notes => Icon::Note,
            Self::Results => Icon::Search,
            Self::Attachments => Icon::Attach,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Pages => "Pages",
            Self::Outline => "Outline",
            Self::Bookmarks => "Bookmarks",
            Self::Notes => "Notes",
            Self::Results => "Search results",
            Self::Attachments => "Attachments",
        }
    }

    fn placeholder(self) -> (&'static str, &'static str) {
        match self {
            Self::Pages => ("", ""),
            Self::Outline => (
                "Outline is not available yet",
                "The table of contents built into the PDF will appear here.",
            ),
            Self::Bookmarks => (
                "Bookmarks are not available yet",
                "Pages you bookmark will be listed here.",
            ),
            Self::Notes => (
                "Notes are not available yet",
                "Highlights, notes, drawings and stamps will be listed here.",
            ),
            Self::Results => ("", ""),
            Self::Attachments => (
                "Attachments are not available yet",
                "Files embedded in the PDF will be listed here.",
            ),
        }
    }
}

pub(super) fn sheet_placeholder(width: f32, height: f32, seed: usize) -> impl IntoElement {
    let line = |w: f32, color| {
        div()
            .h(px(3.0))
            .w(gpui::relative(w))
            .rounded(px(1.0))
            .bg(color)
    };
    div()
        .w(px(width))
        .h(px(height))
        .bg(Paper::base())
        .rounded(px(3.0))
        .p(px(width * 0.1))
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .h(px(6.0))
                .w(gpui::relative(0.7))
                .my(px(4.0))
                .bg(Paper::heading()),
        )
        .children((0..7).map(move |index| {
            line(
                if (index + seed) % 4 == 3 { 0.6 } else { 1.0 },
                Paper::rule(),
            )
        }))
}

impl Reader {
    pub(in crate::reader) fn sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        let panel = self.chrome.panel;
        let count = self.tabs.current().map_or(0, |tab| tab.count);
        let rail = div()
            .flex()
            .gap(px(2.0))
            .px(px(8.0))
            .pt(px(8.0))
            .pb(px(4.0))
            .children(Panel::ALL.iter().map(|item| {
                let item = *item;
                let selected = item == panel;
                let hover = theme.hover;
                clickable(item.title())
                    .flex_1()
                    .h(px(30.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(RADIUS))
                    .when(selected, |button| button.bg(theme.raised))
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.chrome.panel = item;
                        cx.notify();
                    }))
                    .child(icon(
                        item.icon(),
                        if selected { theme.ink } else { theme.faint },
                    ))
            }));
        let header = div()
            .px(px(14.0))
            .pt(px(8.0))
            .pb(px(6.0))
            .min_h(px(34.0))
            .flex()
            .items_center()
            .justify_between()
            .child(label(&theme, panel.title()).text_color(theme.muted))
            .when(panel == Panel::Pages, |header| {
                header.child(
                    mono(count.to_string())
                        .text_size(px(11.0))
                        .text_color(theme.faint),
                )
            })
            .when_some(
                self.search
                    .as_ref()
                    .filter(|search| panel == Panel::Results && !search.hits.is_empty()),
                |header, search| {
                    header.child(
                        mono(search.summary_total())
                            .text_size(px(11.0))
                            .text_color(theme.faint),
                    )
                },
            );
        let body: AnyElement = if panel == Panel::Pages {
            self.thumbnails(cx).into_any_element()
        } else if panel == Panel::Results {
            self.results(cx)
        } else {
            let (title, body) = panel.placeholder();
            div()
                .flex_1()
                .child(empty_state(&theme, panel.icon(), title, body))
                .into_any_element()
        };
        div()
            .w(px(SIDEBAR_WIDTH))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.chrome)
            .child(rail)
            .child(header)
            .child(body)
    }

    fn thumbnails(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.tabs.current().map_or(0, |tab| tab.count);
        let scroll = self
            .tabs
            .current()
            .map(|tab| tab.sidebar_scroll.clone())
            .unwrap_or_default();
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
                let theme = this.chrome.theme;
                let session = this.tabs.current().map_or(0, |tab| tab.session);
                let current_page = this.tabs.current().map_or(0, |tab| tab.page);
                range
                    .map(|index| {
                        this.thumbnails.get(&(session, index));
                        let current = index == current_page;
                        let preview =
                            this.thumbnail_element(session, index, THUMB_WIDTH, THUMB_HEIGHT);
                        let hairline = theme.hairline;
                        clickable(index)
                            .w_full()
                            .h(px(178.0))
                            .flex()
                            .justify_center()
                            .child(
                                clickable("thumbnail")
                                    .group("thumb")
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap(px(6.0))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.go(index, window, cx)
                                    }))
                                    .child(
                                        div()
                                            .p(px(3.0))
                                            .rounded(px(5.0))
                                            .border_2()
                                            .border_color(if current {
                                                theme.accent
                                            } else {
                                                gpui::transparent_black()
                                            })
                                            .when(!current, |frame| {
                                                frame.group_hover("thumb", move |style| {
                                                    style.border_color(hairline)
                                                })
                                            })
                                            .child(preview),
                                    )
                                    .child(
                                        mono((index + 1).to_string())
                                            .text_size(px(11.0))
                                            .text_color(if current {
                                                theme.accent
                                            } else {
                                                theme.faint
                                            }),
                                    ),
                            )
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(scroll)
        .flex_1()
        .pb(px(12.0))
    }

    pub(super) fn thumbnail_element(
        &self,
        session: u64,
        index: usize,
        width: f32,
        height: f32,
    ) -> AnyElement {
        if let Some(thumbnail) = self.thumbnails.peek(&(session, index)) {
            let scale = (width / thumbnail.width).min(height / thumbnail.height);
            div()
                .w(px(width))
                .h(px(height))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    img(thumbnail.image.clone())
                        .w(px(thumbnail.width * scale))
                        .h(px(thumbnail.height * scale))
                        .rounded(px(2.0)),
                )
                .into_any_element()
        } else {
            sheet_placeholder(width, height, index).into_any_element()
        }
    }
}
