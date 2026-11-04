use super::{super::Reader, INSPECTOR_WIDTH};
use crate::reader::{actions::CopyText, chrome::InspectorTab};
use crate::ui::{
    icon::Icon,
    kit::{Kind, RADIUS, Status, empty_state, label, mono, status, text_button},
    theme::Theme,
};
use gpui::{AnyElement, Context, IntoElement, SharedString, div, prelude::*, px};
use std::path::Path;

fn size_label(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn folder_label(path: &Path) -> String {
    let folder = path.parent().unwrap_or(path);
    if let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from)
        && let Ok(rest) = folder.strip_prefix(&home)
    {
        return format!("~/{}", rest.display());
    }
    folder.display().to_string()
}

fn row(theme: &Theme, key: &str, value: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .gap(px(10.0))
        .text_size(px(12.0))
        .child(
            div()
                .w(px(92.0))
                .flex_none()
                .text_color(theme.muted)
                .child(key.to_string()),
        )
        .child(div().flex_1().min_w_0().text_color(theme.ink).child(value))
}

fn section(theme: &Theme, title: &str) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(label(theme, title.to_string()).mb(px(2.0)))
}

impl InspectorTab {
    const ALL: [InspectorTab; 3] = [Self::Selection, Self::Document, Self::Signatures];

    fn title(self) -> &'static str {
        match self {
            Self::Selection => "Selection",
            Self::Document => "Document",
            Self::Signatures => "Placing signatures",
        }
    }
}

impl Reader {
    pub(in crate::reader) fn inspector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        let current = self.chrome.inspector.unwrap_or(InspectorTab::Document);
        let tabs = div()
            .flex()
            .gap(px(2.0))
            .px(px(8.0))
            .pt(px(8.0))
            .pb(px(4.0))
            .children(InspectorTab::ALL.iter().map(|tab| {
                let tab = *tab;
                let selected = tab == current;
                let hover = theme.hover;
                div()
                    .id(tab.title())
                    .flex_1()
                    .h(px(28.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(RADIUS))
                    .text_size(px(12.0))
                    .text_color(if selected { theme.ink } else { theme.muted })
                    .when(selected, |button| button.bg(theme.raised))
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.chrome.inspector = Some(tab);
                        cx.notify();
                    }))
                    .child(tab.title())
            }));
        let body: AnyElement = match current {
            InspectorTab::Selection => self.inspector_selection(cx),
            InspectorTab::Document => self.inspector_document(),
            InspectorTab::Signatures => empty_state(
                &theme,
                Icon::Sign,
                "Signature checking is not available yet",
                "Digital signatures in this PDF, who signed and whether the file changed afterwards will be shown here.",
            )
            .into_any_element(),
        };
        div()
            .id("inspector")
            .w(px(INSPECTOR_WIDTH))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.chrome)
            .child(tabs)
            .child(
                div()
                    .id("inspector-body")
                    .flex_1()
                    .overflow_y_scroll()
                    .px(px(16.0))
                    .py(px(10.0))
                    .child(body),
            )
    }

    fn inspector_selection(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.chrome.theme;
        let selected = match (&self.text, &self.selection) {
            (Some(Ok(text)), Some(selection)) if !selection.range().is_empty() => {
                Some(text.content(selection.range()))
            }
            _ => None,
        };
        let Some(content) = selected else {
            return empty_state(
                &theme,
                Icon::Cursor,
                "Nothing selected",
                "Select text on the page to see it here. Annotation details, colour, opacity and comments will appear here once annotations are built.",
            )
            .into_any_element();
        };
        let words = content.split_whitespace().count();
        let preview: String = content.chars().take(280).collect();
        div()
            .flex()
            .flex_col()
            .gap(px(18.0))
            .child(
                section(&theme, "Text selection")
                    .child(row(
                        &theme,
                        "Page",
                        mono((self.tabs.current().map_or(0, |tab| tab.page) + 1).to_string()),
                    ))
                    .child(row(&theme, "Words", mono(words.to_string())))
                    .child(row(
                        &theme,
                        "Characters",
                        mono(content.chars().count().to_string()),
                    )),
            )
            .child(
                section(&theme, "Text").child(
                    div()
                        .text_size(px(12.0))
                        .text_color(theme.muted)
                        .child(SharedString::from(preview)),
                ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(6.0))
                    .child(
                        text_button(
                            "inspector-copy",
                            if self.copied { "Copied" } else { "Copy text" },
                            &theme,
                            Kind::Quiet,
                        )
                        .h(px(28.0))
                        .on_click(|_, window, cx| window.dispatch_action(Box::new(CopyText), cx)),
                    )
                    .child(
                        text_button("inspector-highlight", "Highlight", &theme, Kind::Quiet)
                            .h(px(28.0))
                            .on_click(
                                cx.listener(|this, _, _, cx| this.unavailable("Highlighting", cx)),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn inspector_document(&self) -> AnyElement {
        let theme = self.chrome.theme;
        let Some(tab) = self.tabs.current() else {
            return div().into_any_element();
        };
        let size = std::fs::metadata(&tab.path)
            .map(|metadata| size_label(metadata.len()))
            .unwrap_or_else(|_| "Unknown".into());
        let pending = |text: &str| {
            div()
                .text_size(px(12.0))
                .text_color(theme.faint)
                .child(text.to_string())
        };
        div()
            .flex()
            .flex_col()
            .gap(px(18.0))
            .child(
                section(&theme, "File")
                    .child(row(&theme, "Name", div().child(tab.title())))
                    .child(row(&theme, "Size", mono(size)))
                    .child(row(&theme, "Pages", mono(tab.count.to_string())))
                    .child(row(
                        &theme,
                        "Location",
                        mono(folder_label(&tab.path)).text_size(px(11.0)),
                    )),
            )
            .child(section(&theme, "Properties").child(pending(
                "Title, author, dates and producer are not available yet.",
            )))
            .child(
                section(&theme, "Security")
                    .child(
                        div()
                            .flex()
                            .child(status(&theme, Status::Info, "Not checked yet")),
                    )
                    .child(pending(
                        "Encryption and permission details are not available yet.",
                    )),
            )
            .child(section(&theme, "Fonts").child(pending("Embedded fonts are not available yet.")))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_sizes_use_readable_units() {
        assert_eq!(size_label(512), "512 B");
        assert_eq!(size_label(2048), "2.0 KB");
        assert_eq!(size_label(5 * 1024 * 1024 + 1024 * 512), "5.5 MB");
    }
}
