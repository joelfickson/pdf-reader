use super::super::{
    Reader,
    actions::*,
    chrome::{Input, Layout, Mode, Overlay, Panel, Tool},
    commands::modifier,
};
use crate::ui::{
    field::TextField,
    icon::Icon,
    kit::clickable,
    kit::{RADIUS, Status, Tone, dot, floating, icon, icon_button, kbd, label, separator, status},
    theme::{Marker, Theme},
};
use gpui::{
    AnyElement, Context, Corner, Div, FontWeight, Hsla, IntoElement, SharedString, Window,
    anchored, deferred, div, point, prelude::*, px,
};

pub(super) fn field(theme: &Theme, value: &TextField, placeholder: &str, focused: bool) -> Div {
    let empty = value.text.is_empty();
    let text = |content: &str| {
        div()
            .flex_none()
            .text_color(theme.ink)
            .child(content.to_string())
    };
    div()
        .flex()
        .items_center()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .when(empty, |field| {
            field
                .when(focused, |field| field.child(caret(theme)))
                .child(div().text_color(theme.faint).child(placeholder.to_string()))
        })
        .when(!empty, |field| {
            field
                .child(text(value.before_cursor()))
                .when(focused, |field| field.child(caret(theme)))
                .child(text(value.after_cursor()))
        })
}

fn caret(theme: &Theme) -> Div {
    div().w(px(1.5)).h(px(16.0)).mx(px(1.0)).bg(theme.accent)
}

fn layer_top_center(child: impl IntoElement) -> Div {
    div()
        .absolute()
        .top(px(14.0))
        .left_0()
        .right_0()
        .flex()
        .justify_center()
        .child(child)
}

fn layer_bottom_center(bottom: f32, child: impl IntoElement) -> Div {
    div()
        .absolute()
        .bottom(px(bottom))
        .left_0()
        .right_0()
        .flex()
        .justify_center()
        .child(child)
}

fn menu_item(
    id: impl Into<gpui::ElementId>,
    text: impl Into<SharedString>,
    checked: bool,
    hint: Option<String>,
    theme: &Theme,
) -> gpui::Stateful<Div> {
    let hover = theme.hover;
    clickable(id)
        .h(px(30.0))
        .px(px(8.0))
        .flex()
        .items_center()
        .gap(px(8.0))
        .rounded(px(RADIUS))
        .text_color(if checked {
            theme.accent_strong
        } else {
            theme.ink
        })
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(dot(
            if checked {
                theme.accent_strong
            } else {
                gpui::transparent_black()
            },
            6.0,
        ))
        .child(div().flex_1().child(text.into()))
        .children(hint.map(|hint| kbd(theme, hint)))
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::Select => "Selecting annotations",
            Self::Highlight => "Highlighting",
            Self::Underline => "Underlining",
            Self::Strike => "Strikethrough",
            Self::Note => "Adding notes",
            Self::Draw => "Drawing",
            Self::Shape => "Drawing shapes",
            Self::Text => "Adding text boxes",
            Self::Stamp => "Adding stamps",
        }
    }
}

impl Reader {
    pub(super) fn floating_layers(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut layers = Vec::new();
        let has_document = self.has_document();
        let reading = has_document && self.chrome.mode != Mode::Organize;
        if has_document && self.chrome.mode == Mode::Annotate {
            let stamps = (self.chrome.overlay == Overlay::Stamps).then(|| {
                div()
                    .absolute()
                    .top(px(44.0))
                    .right(px(110.0))
                    .child(self.stamps(cx))
            });
            layers.push(
                layer_top_center(div().relative().child(self.tools(cx)).children(stamps))
                    .into_any_element(),
            );
        }
        if reading && self.chrome.find_open {
            layers.push(
                div()
                    .absolute()
                    .top(px(if self.chrome.mode == Mode::Annotate {
                        62.0
                    } else {
                        14.0
                    }))
                    .right(px(20.0))
                    .child(self.find_bar(cx))
                    .into_any_element(),
            );
        }
        if reading {
            let menu = match self.chrome.overlay {
                Overlay::ZoomMenu => Some(
                    div()
                        .absolute()
                        .bottom(px(44.0))
                        .left(px(101.0))
                        .child(self.zoom_menu(cx))
                        .into_any_element(),
                ),
                Overlay::ViewMenu => Some(
                    div()
                        .absolute()
                        .bottom(px(44.0))
                        .right_0()
                        .child(self.view_menu(cx))
                        .into_any_element(),
                ),
                _ => None,
            };
            layers.push(
                layer_bottom_center(20.0, div().relative().child(self.hud(cx)).children(menu))
                    .into_any_element(),
            );
        }
        if let Some(popover) = self.selection_popover(window, cx) {
            layers.push(popover);
        }
        if let Some(toast) = self.toast_element(cx) {
            layers.push(
                div()
                    .absolute()
                    .right(px(20.0))
                    .bottom(px(if reading { 72.0 } else { 20.0 }))
                    .child(toast)
                    .into_any_element(),
            );
        }
        layers
    }

    fn hud(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        let tab = self.tabs.current();
        let page = tab.map_or(0, |tab| tab.page);
        let count = tab.map_or(0, |tab| tab.count);
        let zoom = tab.and_then(|tab| tab.zoom);
        let zoom_label = zoom.map_or_else(
            || {
                self.image.as_ref().map_or("Fit".to_string(), |image| {
                    format!("{:.0}%", image.zoom_percent)
                })
            },
            |zoom| format!("{zoom}%"),
        );
        let editing = self.chrome.input == Input::Page;
        let hover = theme.hover;
        floating(&theme)
            .id("hud")
            .occlude()
            .p(px(4.0))
            .flex()
            .items_center()
            .gap(px(2.0))
            .child(
                icon_button("previous", Icon::ChevronLeft, &theme, Tone::Plain, page > 0)
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(Previous), cx)),
            )
            .child(
                clickable("page-input")
                    .h(px(28.0))
                    .w(px(76.0))
                    .px(px(6.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(4.0))
                    .rounded(px(RADIUS))
                    .font_family(Theme::mono_font())
                    .text_size(px(12.0))
                    .when(editing, |input| {
                        input.bg(theme.canvas).border_1().border_color(theme.accent)
                    })
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .on_click(cx.listener(|this, _, _, cx| this.start_page_input(cx)))
                    .child(if editing {
                        field(
                            &theme,
                            &self.chrome.page_input,
                            &(page + 1).to_string(),
                            true,
                        )
                        .into_any_element()
                    } else {
                        div()
                            .text_color(theme.ink)
                            .child((page + 1).to_string())
                            .into_any_element()
                    })
                    .child(div().text_color(theme.faint).child(format!("/ {count}"))),
            )
            .child(
                icon_button(
                    "next",
                    Icon::ChevronRight,
                    &theme,
                    Tone::Plain,
                    page + 1 < count,
                )
                .on_click(|_, window, cx| window.dispatch_action(Box::new(Next), cx)),
            )
            .child(separator(&theme))
            .child(
                icon_button(
                    "zoom-out",
                    Icon::Minus,
                    &theme,
                    Tone::Plain,
                    zoom != Some(25),
                )
                .on_click(|_, window, cx| window.dispatch_action(Box::new(ZoomOut), cx)),
            )
            .child(
                clickable("zoom-presets")
                    .h(px(28.0))
                    .w(px(56.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(RADIUS))
                    .font_family(Theme::mono_font())
                    .text_size(px(12.0))
                    .text_color(if self.chrome.overlay == Overlay::ZoomMenu {
                        theme.accent_strong
                    } else {
                        theme.ink
                    })
                    .when(self.chrome.overlay == Overlay::ZoomMenu, |button| {
                        button.bg(theme.accent_soft)
                    })
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .on_click(cx.listener(|this, _, _, cx| this.set_overlay(Overlay::ZoomMenu, cx)))
                    .child(zoom_label),
            )
            .child(
                icon_button(
                    "zoom-in",
                    Icon::Plus,
                    &theme,
                    Tone::Plain,
                    zoom != Some(400),
                )
                .on_click(|_, window, cx| window.dispatch_action(Box::new(ZoomIn), cx)),
            )
            .child(separator(&theme))
            .child(
                icon_button("bookmark", Icon::Bookmark, &theme, Tone::Plain, true)
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(Bookmark), cx)),
            )
            .child(
                icon_button(
                    "view-options",
                    Icon::Layout,
                    &theme,
                    if self.chrome.overlay == Overlay::ViewMenu {
                        Tone::Active
                    } else {
                        Tone::Plain
                    },
                    true,
                )
                .on_click(cx.listener(|this, _, _, cx| this.set_overlay(Overlay::ViewMenu, cx))),
            )
    }

    fn zoom_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        let zoom = self.tabs.current().and_then(|tab| tab.zoom);
        let m = modifier();
        floating(&theme)
            .id("zoom-menu")
            .occlude()
            .w(px(220.0))
            .p(px(6.0))
            .flex()
            .flex_col()
            .child(
                menu_item(
                    "fit-page",
                    "Fit page",
                    false,
                    Some(format!("{m} 9")),
                    &theme,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.chrome.overlay = Overlay::None;
                    this.unavailable("Fit page", cx)
                })),
            )
            .child(
                menu_item(
                    "fit-width",
                    "Fit width",
                    zoom.is_none(),
                    Some(format!("{m} 0")),
                    &theme,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.chrome.overlay = Overlay::None;
                    this.set_zoom(None, window, cx)
                })),
            )
            .child(
                menu_item(
                    "actual-size",
                    "Actual size",
                    zoom == Some(100),
                    Some(format!("{m} 1")),
                    &theme,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.chrome.overlay = Overlay::None;
                    this.set_zoom(Some(100), window, cx)
                })),
            )
            .child(div().h(px(1.0)).my(px(6.0)).mx(px(4.0)).bg(theme.hairline))
            .children([50u32, 100, 200, 400].into_iter().map(|level| {
                menu_item(
                    ("zoom", level as usize),
                    format!("{level}%"),
                    zoom == Some(level),
                    None,
                    &theme,
                )
                .font_family(Theme::mono_font())
                .text_size(px(12.0))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.chrome.overlay = Overlay::None;
                    this.set_zoom(Some(level), window, cx)
                }))
            }))
    }

    fn view_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        let m = modifier();
        let item = |id: &'static str,
                    text: &'static str,
                    checked: bool,
                    hint: Option<String>,
                    feature: &'static str| {
            menu_item(id, text, checked, hint, &theme).on_click(cx.listener(
                move |this, _, _, cx| {
                    this.chrome.overlay = Overlay::None;
                    if !checked {
                        this.unavailable(feature, cx);
                    }
                    cx.notify();
                },
            ))
        };
        let current = self.chrome.layout;
        let layout_item = |id: &'static str, text: &'static str, layout: Layout| {
            menu_item(id, text, current == layout, None, &theme).on_click(cx.listener(
                move |this, _, window, cx| {
                    this.chrome.overlay = Overlay::None;
                    this.set_layout(layout, window, cx);
                },
            ))
        };
        floating(&theme)
            .id("view-menu")
            .occlude()
            .w(px(240.0))
            .p(px(6.0))
            .flex()
            .flex_col()
            .child(label(&theme, "Layout").px(px(8.0)).pt(px(6.0)).pb(px(4.0)))
            .child(layout_item("single", "Single page", Layout::Single))
            .child(layout_item("continuous", "Continuous", Layout::Continuous))
            .child(item("two-pages", "Two pages", false, None, "Two-page view"))
            .child(item(
                "cover",
                "Two pages, cover first",
                false,
                None,
                "Two-page view",
            ))
            .child(item(
                "split",
                "Split view",
                false,
                Some(format!("{m} \\")),
                "Split view",
            ))
            .child(div().h(px(1.0)).my(px(6.0)).mx(px(4.0)).bg(theme.hairline))
            .child(
                label(&theme, "Page colour")
                    .px(px(8.0))
                    .pt(px(6.0))
                    .pb(px(4.0)),
            )
            .child(item("original", "Original", true, None, ""))
            .child(item("night", "Night", false, None, "Night pages"))
            .child(item("sepia", "Sepia", false, None, "Sepia pages"))
            .child(div().h(px(1.0)).my(px(6.0)).mx(px(4.0)).bg(theme.hairline))
            .child(item(
                "magnifier",
                "Magnifier",
                false,
                Some(format!("{m} Shift M")),
                "The magnifier",
            ))
            .child(item(
                "present",
                "Presentation",
                false,
                Some(format!("{m} Shift P")),
                "Presentation mode",
            ))
    }

    fn find_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        let focused = self.chrome.input == Input::Find;
        let option = |id: &'static str, text: &'static str, whole_word: bool, pressed: bool| {
            let hover = theme.hover;
            clickable(id)
                .h(px(28.0))
                .min_w(px(26.0))
                .px(px(6.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(RADIUS))
                .font_family(Theme::mono_font())
                .text_size(px(11.0))
                .text_color(if pressed {
                    theme.accent_strong
                } else {
                    theme.muted
                })
                .when(pressed, |button| button.bg(theme.accent_soft))
                .cursor_pointer()
                .when(!pressed, |button| {
                    button.hover(move |style| style.bg(hover))
                })
                .on_click(
                    cx.listener(move |this, _, _, cx| this.toggle_find_option(whole_word, cx)),
                )
                .child(text)
        };
        let summary = self
            .search
            .as_ref()
            .map(|search| search.summary())
            .unwrap_or_default();
        floating(&theme)
            .id("find-bar")
            .occlude()
            .w(px(420.0))
            .p(px(4.0))
            .flex()
            .items_center()
            .gap(px(4.0))
            .child(div().pl(px(6.0)).child(icon(Icon::Search, theme.muted)))
            .child(
                clickable("find-field")
                    .flex_1()
                    .min_w_0()
                    .h(px(28.0))
                    .px(px(4.0))
                    .flex()
                    .items_center()
                    .cursor_text()
                    .on_click(cx.listener(|this, _, _, cx| this.open_find(cx)))
                    .child(field(
                        &theme,
                        &self.chrome.find,
                        "Find in document",
                        focused,
                    )),
            )
            .child(
                div()
                    .px(px(6.0))
                    .font_family(Theme::mono_font())
                    .text_size(px(12.0))
                    .text_color(theme.muted)
                    .whitespace_nowrap()
                    .child(summary),
            )
            .child(option("match-case", "Aa", false, self.chrome.find_case))
            .child(option("whole-word", "W", true, self.chrome.find_word))
            .child(
                icon_button("find-results", Icon::List, &theme, Tone::Plain, true).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.chrome.panel = Panel::Results;
                        this.sidebar = true;
                        cx.notify();
                    }),
                ),
            )
            .child(
                icon_button("find-previous", Icon::ChevronUp, &theme, Tone::Plain, true).on_click(
                    cx.listener(|this, _, window, cx| this.step_match(false, window, cx)),
                ),
            )
            .child(
                icon_button("find-next", Icon::ChevronDown, &theme, Tone::Plain, true)
                    .on_click(cx.listener(|this, _, window, cx| this.step_match(true, window, cx))),
            )
            .child(
                icon_button("find-close", Icon::Close, &theme, Tone::Plain, true)
                    .on_click(cx.listener(|this, _, _, cx| this.close_find(cx))),
            )
    }

    fn tools(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        let current = self.chrome.tool;
        let tool = |id: &'static str, glyph: Icon, value: Tool| {
            icon_button(
                id,
                glyph,
                &theme,
                if current == value {
                    Tone::Active
                } else {
                    Tone::Plain
                },
                true,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.chrome.tool = value;
                if value == Tool::Stamp {
                    this.set_overlay(Overlay::Stamps, cx);
                } else {
                    this.chrome.overlay = Overlay::None;
                    this.unavailable(value.name(), cx);
                }
            }))
        };
        let swatches = (0..Marker::ALL.len()).map(|index| {
            let selected = self.chrome.marker == index;
            let hover = theme.hover;
            clickable(("marker", index))
                .size(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(RADIUS))
                .when(selected, |swatch| swatch.bg(theme.hover))
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.chrome.marker = index;
                    cx.notify();
                }))
                .child(
                    div()
                        .size(px(16.0))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(selected, |ring| {
                            ring.border_1().border_color(Marker::color(index))
                        })
                        .child(dot(Marker::color(index), 12.0)),
                )
        });
        floating(&theme)
            .id("annotation-tools")
            .occlude()
            .p(px(4.0))
            .flex()
            .items_center()
            .gap(px(2.0))
            .child(tool("tool-select", Icon::Cursor, Tool::Select))
            .child(tool("tool-highlight", Icon::Highlight, Tool::Highlight))
            .children(swatches)
            .child(separator(&theme))
            .child(tool("tool-underline", Icon::Underline, Tool::Underline))
            .child(tool("tool-strike", Icon::Strike, Tool::Strike))
            .child(tool("tool-note", Icon::Note, Tool::Note))
            .child(tool("tool-draw", Icon::Draw, Tool::Draw))
            .child(tool("tool-shape", Icon::Rect, Tool::Shape))
            .child(tool("tool-text", Icon::Text, Tool::Text))
            .child(tool("tool-stamp", Icon::Stamp, Tool::Stamp))
            .child(
                icon_button("tool-sign", Icon::Sign, &theme, Tone::Plain, true)
                    .child("Sign")
                    .on_click(cx.listener(|this, _, _, cx| this.set_overlay(Overlay::Sign, cx))),
            )
            .child(separator(&theme))
            .child(
                icon_button("undo", Icon::Undo, &theme, Tone::Plain, true)
                    .on_click(cx.listener(|this, _, _, cx| this.unavailable("Undo", cx))),
            )
            .child(
                icon_button("redo", Icon::Redo, &theme, Tone::Plain, true)
                    .on_click(cx.listener(|this, _, _, cx| this.unavailable("Redo", cx))),
            )
            .child(
                icon_button("annotate-done", Icon::Check, &theme, Tone::Plain, true)
                    .child("Done")
                    .on_click(
                        cx.listener(|this, _, window, cx| {
                            this.set_mode(Mode::Annotate, window, cx)
                        }),
                    ),
            )
    }

    fn stamps(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        let stamps: [(&str, u32); 6] = [
            ("APPROVED", 0x1f883d),
            ("REJECTED", 0xcf222e),
            ("DRAFT", 0x5b6168),
            ("CONFIDENTIAL", 0xcf222e),
            ("REVIEWED", 0x1c3d8f),
            ("CUSTOM", 0x5b6168),
        ];
        floating(&theme)
            .id("stamps-menu")
            .occlude()
            .w(px(260.0))
            .p(px(8.0))
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(label(&theme, "Stamps").px(px(4.0)))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(4.0))
                    .children(stamps.into_iter().map(|(text, color)| {
                        let color: Hsla = gpui::rgb(color).into();
                        let accent = theme.accent;
                        clickable(text)
                            .w(px(118.0))
                            .h(px(44.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(RADIUS))
                            .bg(gpui::white())
                            .border_2()
                            .border_color(gpui::white())
                            .cursor_pointer()
                            .hover(move |style| style.border_color(accent))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.chrome.overlay = Overlay::None;
                                this.unavailable("Adding stamps", cx)
                            }))
                            .child(
                                div()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .border_2()
                                    .border_color(color)
                                    .rounded(px(3.0))
                                    .text_color(color)
                                    .text_size(px(10.0))
                                    .font_weight(FontWeight::EXTRA_BOLD)
                                    .child(text),
                            )
                    })),
            )
    }

    fn selection_popover(&self, _: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.has_selection()
            || self
                .selection
                .as_ref()
                .is_some_and(|selection| selection.dragging)
            || self.chrome.mode == Mode::Organize
            || self.chrome.overlay.is_modal()
        {
            return None;
        }
        let Some(Ok(text)) = &self.text else {
            return None;
        };
        let selection = self.selection.as_ref()?;
        let first = text
            .glyphs
            .iter()
            .skip(selection.range().start)
            .take(selection.range().len())
            .find_map(|glyph| glyph.bounds)?;
        let bounds = self.page_bounds.get();
        if bounds.size.width <= px(0.0) {
            return None;
        }
        let position = point(
            bounds.origin.x + bounds.size.width * first.left,
            bounds.origin.y + bounds.size.height * first.top - px(8.0),
        );
        let theme = self.chrome.theme;
        let copied = self.copied;
        let swatches = (0..Marker::ALL.len()).map(|index| {
            let hover = theme.hover;
            clickable(("quick-highlight", index))
                .size(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(RADIUS))
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .on_click(cx.listener(|this, _, _, cx| this.unavailable("Highlighting", cx)))
                .child(dot(Marker::color(index), 12.0))
        });
        let bar = floating(&theme)
            .id("selection-bar")
            .occlude()
            .p(px(4.0))
            .flex()
            .items_center()
            .gap(px(2.0))
            .children(swatches)
            .child(separator(&theme))
            .child(
                icon_button(
                    "quick-underline",
                    Icon::Underline,
                    &theme,
                    Tone::Plain,
                    true,
                )
                .on_click(cx.listener(|this, _, _, cx| this.unavailable("Underlining", cx))),
            )
            .child(
                icon_button("quick-note", Icon::Note, &theme, Tone::Plain, true)
                    .on_click(cx.listener(|this, _, _, cx| this.unavailable("Adding notes", cx))),
            )
            .child(
                icon_button("quick-lookup", Icon::Lookup, &theme, Tone::Plain, true)
                    .on_click(cx.listener(|this, _, _, cx| this.unavailable("Look up", cx))),
            )
            .child(separator(&theme))
            .child(
                icon_button(
                    "quick-copy",
                    if copied { Icon::Check } else { Icon::Copy },
                    &theme,
                    Tone::Plain,
                    true,
                )
                .child(if copied { "Copied" } else { "Copy" })
                .on_click(|_, window, cx| window.dispatch_action(Box::new(CopyText), cx)),
            );
        Some(
            deferred(
                anchored()
                    .position(position)
                    .anchor(Corner::BottomLeft)
                    .snap_to_window()
                    .child(bar),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    fn toast_element(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = self.chrome.theme;
        if self.chrome.toast.is_none()
            && let Some(notice) = &self.notice
        {
            return Some(
                floating(&theme)
                    .id("notice")
                    .occlude()
                    .max_w(px(420.0))
                    .py(px(8.0))
                    .pl(px(12.0))
                    .pr(px(6.0))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(status(&theme, Status::Warn, "Notice"))
                    .child(div().flex_1().text_color(theme.ink).child(notice.clone()))
                    .child(
                        icon_button("dismiss-notice", Icon::Close, &theme, Tone::Plain, true)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.notice = None;
                                cx.notify();
                            })),
                    )
                    .into_any_element(),
            );
        }
        let toast = self.chrome.toast.as_ref()?;
        Some(
            floating(&theme)
                .id("toast")
                .occlude()
                .max_w(px(420.0))
                .py(px(10.0))
                .px(px(12.0))
                .flex()
                .items_start()
                .gap(px(10.0))
                .child(status(&theme, toast.status, "Note"))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(div().text_color(theme.ink).child(toast.title.clone()))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(theme.muted)
                                .child(toast.message.clone()),
                        ),
                )
                .into_any_element(),
        )
    }

    pub(super) fn set_zoom(
        &mut self,
        zoom: Option<u32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.current_mut() {
            tab.zoom = zoom.map(|zoom| zoom.clamp(25, 400));
        }
        self.request(window);
        cx.notify();
    }
}
