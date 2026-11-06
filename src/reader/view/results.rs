use super::super::Reader;
use crate::ui::{
    icon::Icon,
    kit::{RADIUS, clickable, empty_state, label},
};
use gpui::{AnyElement, Context, HighlightStyle, IntoElement, StyledText, div, prelude::*, px};

const RESULT_ROWS: usize = 200;
const ELLIPSIS: &str = "...";
const FIND_SHORTCUT: &str = if cfg!(target_os = "macos") {
    "Cmd F"
} else {
    "Ctrl F"
};

impl Reader {
    pub(in crate::reader) fn results(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.chrome.theme;
        let Some(search) = &self.search else {
            return div()
                .flex_1()
                .child(empty_state(
                    &theme,
                    Icon::Search,
                    "No search yet",
                    &format!(
                        "Find in the document with {FIND_SHORTCUT}. Every match is listed here with its surrounding text."
                    ),
                ))
                .into_any_element();
        };
        if search.hits.is_empty() {
            let (title, body) = match (&search.error, search.done) {
                (Some(error), _) => ("Search failed", error.as_str()),
                (None, true) => (
                    "No matches",
                    "Try a shorter phrase, or turn off Match case and Whole word.",
                ),
                (None, false) => ("Searching", "Matches appear here as each page is searched."),
            };
            return div()
                .flex_1()
                .child(empty_state(&theme, Icon::Search, title, body))
                .into_any_element();
        }
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut previous_page = None;
        for (index, (page, hit)) in search.hits.iter().take(RESULT_ROWS).enumerate() {
            if previous_page != Some(*page) {
                previous_page = Some(*page);
                rows.push(
                    label(&theme, format!("Page {}", page + 1))
                        .flex_shrink_0()
                        .px(px(10.0))
                        .pt(px(8.0))
                        .pb(px(4.0))
                        .into_any_element(),
                );
            }
            let lead = if hit.before.is_empty() { "" } else { ELLIPSIS };
            let tail = if hit.after.is_empty() { "" } else { ELLIPSIS };
            let start = lead.len() + hit.before.len();
            let end = start + hit.matched.len();
            let text = format!("{lead}{}{}{}{tail}", hit.before, hit.matched, hit.after);
            let current = search.current == Some(index);
            let hover = theme.hover;
            rows.push(
                clickable(("search-result", index))
                    .flex_shrink_0()
                    .w_full()
                    .p(px(10.0))
                    .rounded(px(RADIUS))
                    .text_size(px(12.0))
                    .text_color(theme.ink)
                    .line_clamp(2)
                    .cursor_pointer()
                    .when(current, |row| row.bg(theme.raised))
                    .when(!current, |row| row.hover(move |style| style.bg(hover)))
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.show_match(index, window, cx)),
                    )
                    .child(StyledText::new(text).with_highlights([(
                        start..end,
                        HighlightStyle {
                            background_color: Some(theme.found),
                            ..Default::default()
                        },
                    )]))
                    .into_any_element(),
            );
        }
        let mut notes = Vec::new();
        if search.hits.len() > RESULT_ROWS || search.truncated {
            notes.push(format!(
                "Showing the first {RESULT_ROWS} of {} matches.",
                search.summary_total()
            ));
        }
        if search.skipped > 0 {
            notes.push(format!(
                "{} {} too much text to search.",
                search.skipped,
                if search.skipped == 1 {
                    "page has"
                } else {
                    "pages have"
                }
            ));
        }
        div()
            .id("search-results")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(6.0))
            .pb(px(16.0))
            .flex()
            .flex_col()
            .children(rows)
            .children(notes.into_iter().map(|note| {
                div()
                    .px(px(10.0))
                    .pt(px(10.0))
                    .text_size(px(12.0))
                    .text_color(theme.muted)
                    .child(note)
            }))
            .into_any_element()
    }
}
