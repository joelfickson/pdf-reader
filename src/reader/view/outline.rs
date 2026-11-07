use super::{super::Reader, SIDEBAR_WIDTH};
use crate::ui::theme::Theme;
use crate::ui::{
    icon::Icon,
    kit::{RADIUS, clickable, empty_state, icon, mono},
};
use gpui::{
    AnyElement, App, Context, FontWeight, Hsla, IntoElement, MouseButton, SharedString, TextRun,
    div, prelude::*, px,
};

const INDENT: f32 = 18.0;
const MAX_INDENT_LEVELS: usize = 4;
const ROW_CHROME: f32 = 6.0 * 2.0 + 10.0 + 10.0 + 12.0 + 6.0 * 2.0;
const DIGIT_WIDTH: f32 = 7.5;

fn fit_title(cx: &App, title: &str, top: bool, width: f32) -> SharedString {
    let mut font = gpui::font(Theme::ui_font());
    if top {
        font.weight = FontWeight::MEDIUM;
    }
    let size = px(if top { 13.0 } else { 12.0 });
    let mut runs = vec![TextRun {
        len: title.len(),
        font: font.clone(),
        color: Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    }];
    cx.text_system().line_wrapper(font, size).truncate_line(
        SharedString::from(title.to_string()),
        px(width),
        "…",
        &mut runs,
    )
}

fn title_width(indent: f32, page: Option<usize>) -> f32 {
    let digits = page.map_or(0, |page| page.to_string().len());
    (SIDEBAR_WIDTH - ROW_CHROME - indent - digits as f32 * DIGIT_WIDTH).max(40.0)
}

impl Reader {
    pub(in crate::reader) fn outline_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.chrome.theme;
        let Some(tab) = self.tabs.current_mut() else {
            return div()
                .flex_1()
                .child(empty_state(
                    &theme,
                    Icon::Outline,
                    "No document open",
                    "Open a PDF to see its table of contents.",
                ))
                .into_any_element();
        };
        let page = tab.page;
        let scroll = tab.outline_scroll.clone();
        let Some(outline) = tab.outline.as_mut() else {
            return div()
                .flex_1()
                .child(empty_state(
                    &theme,
                    Icon::Outline,
                    "Reading the outline",
                    "The table of contents built into the PDF will appear here.",
                ))
                .into_any_element();
        };
        if outline.entries().is_empty() {
            return div()
                .flex_1()
                .child(empty_state(
                    &theme,
                    Icon::Outline,
                    "This PDF has no outline",
                    "Use Pages or Search results to move through it.",
                ))
                .into_any_element();
        }
        let current = outline.current(page);
        let moved = outline.reveal(current);
        let visible = outline.visible();
        if moved && let Some(position) = visible.iter().position(|&row| Some(row) == current) {
            scroll.scroll_to_item(position);
        }
        let hover = theme.hover;
        let rows = visible.into_iter().map(|index| {
            let entry = &outline.entries()[index];
            let selected = Some(index) == current;
            let top = entry.level <= 1;
            let indent = (entry.level.saturating_sub(1)).min(MAX_INDENT_LEVELS) as f32 * INDENT;
            let chevron = outline.has_children(index).then(|| {
                let glyph = if outline.is_expanded(index) {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                };
                clickable(("outline-toggle", index))
                    .size(px(12.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            if let Some(outline) =
                                this.tabs.current_mut().and_then(|tab| tab.outline.as_mut())
                            {
                                outline.toggle(index);
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .child(icon(glyph, theme.muted).size(px(12.0)))
            });
            let target = entry.page.map(|page| (page - 1, entry.anchor));
            clickable(("outline-entry", index))
                .flex_shrink_0()
                .w_full()
                .flex()
                .items_center()
                .gap(px(6.0))
                .py(px(6.0))
                .pr(px(10.0))
                .pl(px(10.0 + indent))
                .rounded(px(RADIUS))
                .when(selected, |row| row.bg(theme.raised))
                .when(!selected, |row| row.hover(move |style| style.bg(hover)))
                .when(target.is_some(), |row| row.cursor_pointer())
                .on_click(cx.listener(move |this, _, window, cx| {
                    if let Some((page, anchor)) = target {
                        this.jump(page, anchor, window, cx);
                    }
                }))
                .child(div().size(px(12.0)).flex_shrink_0().children(chevron))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_size(px(if top { 13.0 } else { 12.0 }))
                        .when(top, |title| title.font_weight(FontWeight::MEDIUM))
                        .text_color(if selected {
                            theme.accent_strong
                        } else if top {
                            theme.ink
                        } else {
                            theme.muted
                        })
                        .child(fit_title(
                            cx,
                            &entry.title,
                            top,
                            title_width(indent, entry.page),
                        )),
                )
                .children(entry.page.map(|page| {
                    mono(page.to_string())
                        .flex_shrink_0()
                        .text_size(px(11.0))
                        .text_color(theme.faint)
                }))
        });
        let truncated = outline.outline.truncated.then(|| {
            div()
                .px(px(10.0))
                .pt(px(10.0))
                .text_size(px(12.0))
                .text_color(theme.muted)
                .child(format!(
                    "Showing the first {} entries.",
                    outline.entries().len()
                ))
        });
        div()
            .id("outline")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&scroll)
            .px(px(6.0))
            .pb(px(16.0))
            .flex()
            .flex_col()
            .children(rows)
            .children(truncated)
            .into_any_element()
    }
}
