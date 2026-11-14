use super::super::{
    Reader,
    chrome::{Input, Overlay},
    commands::{Entry, back_forward, modifier},
    signing::INKS,
};
use super::{floating::field, ink::paint_strokes};
use crate::ui::{
    icon::Icon,
    kit::clickable,
    kit::{
        Kind, RADIUS, Status, Tone, floating, icon, icon_button, kbd, label, mono, pill, status,
        text_button, toggle,
    },
    theme::{Paper, Theme},
};
use gpui::{
    AnyElement, Context, CursorStyle, Div, FontWeight, Hsla, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, SharedString, Window, canvas, div, point, prelude::*, px,
};

pub(super) fn form_row(theme: &Theme, key: &str, value: impl IntoElement) -> Div {
    div()
        .mt(px(14.0))
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .w(px(120.0))
                .flex_none()
                .text_size(px(12.0))
                .text_color(theme.muted)
                .child(key.to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(2.0))
                .flex_wrap()
                .child(value),
        )
}

impl Reader {
    pub(super) fn modal(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let theme = self.chrome.theme;
        let (width, content) = match self.chrome.overlay {
            Overlay::Palette => (560.0, self.palette(cx)),
            Overlay::Sign => (560.0, self.sign_dialog(cx)),
            Overlay::Export => (620.0, self.export_dialog(cx)),
            Overlay::Combine => (560.0, self.combine_dialog(cx)),
            Overlay::Print => (600.0, self.print_dialog(cx)),
            Overlay::Settings => (720.0, self.settings_dialog(window, cx)),
            Overlay::Shortcuts => (820.0, self.shortcuts_dialog(cx)),
            _ => return None,
        };
        Some(
            clickable("scrim")
                .absolute()
                .inset_0()
                .bg(theme.scrim)
                .occlude()
                .flex()
                .justify_center()
                .items_start()
                .pt(px(72.0))
                .px(px(16.0))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.set_overlay(Overlay::None, cx)),
                )
                .child(
                    floating(&theme)
                        .id("dialog")
                        .w(px(width))
                        .max_w_full()
                        .overflow_hidden()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(content),
                )
                .into_any_element(),
        )
    }

    pub(super) fn dialog_header(&self, title: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(16.0))
            .pt(px(14.0))
            .pb(px(6.0))
            .child(
                div()
                    .text_size(px(15.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title.to_string()),
            )
            .child(
                icon_button("dialog-close", Icon::Close, &theme, Tone::Plain, true)
                    .on_click(cx.listener(|this, _, _, cx| this.set_overlay(Overlay::None, cx))),
            )
    }

    pub(super) fn choice_pills(
        &self,
        prefix: &'static str,
        options: Vec<String>,
        current: usize,
        set: fn(&mut Reader, usize),
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = self.chrome.theme;
        div()
            .flex()
            .gap(px(2.0))
            .children(options.into_iter().enumerate().map(|(index, text)| {
                pill((prefix, index), text, current == index, &theme).on_click(cx.listener(
                    move |this, _, _, cx| {
                        set(this, index);
                        cx.notify();
                    },
                ))
            }))
    }

    fn dialog_footer(
        &self,
        note: Option<AnyElement>,
        primary: &'static str,
        feature: &'static str,
        action: Option<fn(&mut Self, &mut Window, &mut Context<Self>)>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.chrome.theme;
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_t_1()
            .border_color(theme.hairline)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .children(note),
            )
            .child(
                text_button("dialog-cancel", "Cancel", &theme, Kind::Quiet)
                    .on_click(cx.listener(|this, _, _, cx| this.set_overlay(Overlay::None, cx))),
            )
            .child(
                text_button("dialog-primary", primary, &theme, Kind::Primary).on_click(
                    cx.listener(move |this, _, window, cx| match action {
                        Some(action) => action(this, window, cx),
                        None => {
                            this.chrome.overlay = Overlay::None;
                            this.unavailable(feature, cx)
                        }
                    }),
                ),
            )
    }

    fn palette(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.chrome.theme;
        let entries = self.palette_entries();
        let selected = self
            .chrome
            .palette_index
            .min(entries.len().saturating_sub(1));
        let mut list = clickable("palette-list")
            .max_h(px(420.0))
            .overflow_y_scroll()
            .p(px(6.0))
            .flex()
            .flex_col();
        let mut last_group = None;
        for (index, entry) in entries.iter().enumerate() {
            let (group, glyph, text, hint): (&str, Icon, SharedString, Option<String>) = match entry
            {
                Entry::Page(page) => (
                    "Go to",
                    Icon::Goto,
                    format!("Page {}", page + 1).into(),
                    None,
                ),
                Entry::Tab(tab) => (
                    "Open documents",
                    Icon::File,
                    self.tabs.entries[*tab].title().into(),
                    None,
                ),
                Entry::Command(command) => (
                    command.group().title(),
                    command.icon(),
                    command.label().into(),
                    command.shortcut(),
                ),
            };
            if last_group != Some(group) {
                list = list.child(label(&theme, group).px(px(10.0)).pt(px(10.0)).pb(px(6.0)));
                last_group = Some(group);
            }
            let entry = *entry;
            let hover = theme.hover;
            list = list.child(
                clickable(("palette-entry", index))
                    .h(px(34.0))
                    .px(px(10.0))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .rounded(px(RADIUS))
                    .when(index == selected, |row| row.bg(theme.hover))
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.run_entry(entry, window, cx)),
                    )
                    .child(icon(glyph, theme.muted))
                    .child(div().flex_1().truncate().child(text))
                    .children(hint.map(|hint| kbd(&theme, hint))),
            );
        }
        if entries.is_empty() {
            list = list.child(
                div()
                    .p(px(12.0))
                    .text_color(theme.muted)
                    .child("No matching commands"),
            );
        }
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(48.0))
                    .px(px(14.0))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .text_size(px(15.0))
                    .child(icon(Icon::Search, theme.muted))
                    .child(field(
                        &theme,
                        &self.chrome.palette,
                        "Type a command, page number, or file name",
                        self.chrome.input == Input::Palette,
                    )),
            )
            .child(div().h(px(1.0)).bg(theme.hairline))
            .child(list)
            .into_any_element()
    }

    fn draw_pad(&self, cx: &mut Context<Self>) -> AnyElement {
        let area = self.signing.pad.clone();
        let strokes = self.signing.strokes.clone();
        let ink: Hsla = gpui::rgb(INKS[self.chrome.sign_ink % INKS.len()]).into();
        let empty = strokes.is_empty();
        clickable("signature-pad")
            .relative()
            .h(px(170.0))
            .rounded(px(RADIUS))
            .bg(Paper::base())
            .cursor(CursorStyle::Crosshair)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.begin_stroke(event.position, cx);
                    cx.stop_propagation();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if event.pressed_button == Some(MouseButton::Left) && this.signing.drawing {
                    this.extend_stroke(event.position, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.release_pointer(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.release_pointer(cx)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(32.0))
                    .right(px(32.0))
                    .bottom(px(38.0))
                    .h(px(1.0))
                    .bg(Paper::rule()),
            )
            .when(empty, |pad| {
                pad.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(Paper::muted())
                        .child("Draw your signature here"),
                )
            })
            .child(
                canvas(
                    move |bounds, _, _| area.set(bounds),
                    move |bounds, _, window, _| {
                        paint_strokes(
                            window,
                            &strokes,
                            |position| bounds.origin + point(px(position[0]), px(position[1])),
                            px(2.6),
                            ink,
                        )
                    },
                )
                .absolute()
                .inset_0()
                .size_full(),
            )
            .when(!empty, |pad| {
                pad.child(
                    clickable("signature-clear")
                        .absolute()
                        .top(px(8.0))
                        .right(px(8.0))
                        .h(px(26.0))
                        .px(px(8.0))
                        .flex()
                        .items_center()
                        .rounded(px(RADIUS))
                        .text_size(px(12.0))
                        .text_color(Paper::muted())
                        .cursor_pointer()
                        .hover(|style| style.bg(Paper::rule()))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|this, _, _, cx| this.clear_pad(cx)))
                        .child("Clear"),
                )
            })
            .into_any_element()
    }

    fn sign_dialog(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.chrome.theme;
        let tabs = ["Draw", "Type", "Image"];
        let pad = match self.chrome.sign_tab {
            0 => div(),
            1 => div()
                .font_family(Theme::script_font())
                .text_size(px(44.0))
                .text_color(Paper::signature())
                .child("Your name"),
            _ => div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.0))
                .text_color(Paper::muted())
                .child(icon(Icon::Image, Paper::muted()).size(px(24.0)))
                .child("Drop a PNG or JPEG of your signature"),
        };
        div()
            .flex()
            .flex_col()
            .child(self.dialog_header("Add signature", cx))
            .child(
                div()
                    .px(px(16.0))
                    .pb(px(16.0))
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .child(
                        div()
                            .flex()
                            .gap(px(2.0))
                            .children(tabs.iter().enumerate().map(|(index, tab)| {
                                pill(
                                    ("sign-tab", index),
                                    *tab,
                                    self.chrome.sign_tab == index,
                                    &theme,
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.chrome.sign_tab = index;
                                        cx.notify();
                                    },
                                ))
                            })),
                    )
                    .child(if self.chrome.sign_tab == 0 {
                        self.draw_pad(cx)
                    } else {
                        clickable("signature-pad")
                            .h(px(170.0))
                            .rounded(px(RADIUS))
                            .bg(Paper::base())
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(12.0))
                            .child(pad)
                            .child(div().w(px(420.0)).max_w_full().h(px(1.0)).bg(Paper::rule()))
                            .into_any_element()
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .text_size(px(12.0))
                            .text_color(theme.muted)
                            .child(div().mr(px(4.0)).child("Ink"))
                            .children(INKS.into_iter().enumerate().map(|(index, color)| {
                                let selected = self.chrome.sign_ink == index;
                                let hover = theme.hover;
                                clickable(("sign-ink", index))
                                    .size(px(28.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(RADIUS))
                                    .when(selected, |swatch| swatch.bg(theme.hover))
                                    .cursor_pointer()
                                    .hover(move |style| style.bg(hover))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.chrome.sign_ink = index;
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .size(px(14.0))
                                            .rounded_full()
                                            .bg(gpui::rgb(color))
                                            .border_1()
                                            .border_color(theme.hairline),
                                    )
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(label(&theme, "Saved"))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme.faint)
                                    .child("Saved signatures are not available yet"),
                            ),
                    ),
            )
            .child(
                self.dialog_footer(
                    Some(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .text_size(px(12.0))
                            .text_color(theme.muted)
                            .child(status(&theme, Status::Warn, "Visual only"))
                            .child("Adds an image, not a certificate")
                            .into_any_element(),
                    ),
                    "Place on page",
                    ["Signatures", "Typed signatures", "Image signatures"]
                        [self.chrome.sign_tab.min(2)],
                    (self.chrome.sign_tab == 0).then_some(|this, _, cx| this.place_signature(cx)),
                    cx,
                ),
            )
            .into_any_element()
    }

    fn export_dialog(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.chrome.theme;
        let formats = [
            ("PDF", "Copy with your notes"),
            ("Flattened PDF", "Notes and signatures baked in"),
            ("PNG", "One image per page"),
            ("JPEG", "Smaller images"),
            ("Plain text", "Extracted text"),
            ("Notes summary", "Highlights and notes as Markdown"),
        ];
        let count = self.tabs.current().map_or(0, |tab| tab.count);
        let choices =
            div()
                .flex()
                .flex_wrap()
                .gap(px(4.0))
                .children(formats.iter().enumerate().map(|(index, (name, detail))| {
                    let selected = self.chrome.export_format == index;
                    let hover = theme.hover;
                    clickable(("export-format", index))
                        .w(px(192.0))
                        .p(px(10.0))
                        .rounded(px(RADIUS))
                        .border_1()
                        .border_color(if selected {
                            theme.accent
                        } else {
                            gpui::transparent_black()
                        })
                        .when(selected, |choice| choice.bg(theme.accent_soft))
                        .cursor_pointer()
                        .hover(move |style| if selected { style } else { style.bg(hover) })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.chrome.export_format = index;
                            cx.notify();
                        }))
                        .child(div().text_color(theme.ink).child(*name))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(if selected {
                                    theme.accent_strong
                                } else {
                                    theme.muted
                                })
                                .child(*detail),
                        )
                }));
        let downloads = dirs_label();
        div()
            .flex()
            .flex_col()
            .child(self.dialog_header("Export", cx))
            .child(
                div()
                    .px(px(16.0))
                    .pb(px(16.0))
                    .child(choices)
                    .child(form_row(
                        &theme,
                        "Pages",
                        self.choice_pills(
                            "export-pages",
                            vec![format!("All {count}"), "Current".into()],
                            self.chrome.export_pages,
                            |this, index| this.chrome.export_pages = index,
                            cx,
                        ),
                    ))
                    .child(form_row(
                        &theme,
                        "Resolution",
                        self.choice_pills(
                            "export-dpi",
                            vec!["72 dpi".into(), "150 dpi".into(), "300 dpi".into()],
                            self.chrome.export_dpi,
                            |this, index| this.chrome.export_dpi = index,
                            cx,
                        ),
                    ))
                    .child(form_row(
                        &theme,
                        "Include",
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .child(
                                toggle("export-notes", self.chrome.include_notes, &theme).on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.chrome.include_notes = !this.chrome.include_notes;
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child("Highlights, notes, and signatures"),
                    ))
                    .child(form_row(
                        &theme,
                        "Save to",
                        mono(downloads).text_color(theme.muted).truncate(),
                    )),
            )
            .child(self.dialog_footer(
                None,
                "Export",
                "Exporting",
                Some(|this, _, cx| this.export_document(cx)),
                cx,
            ))
            .into_any_element()
    }

    fn combine_dialog(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.chrome.theme;
        let open = self.tabs.entries.len();
        let added: Vec<_> = self
            .combine_extra
            .iter()
            .enumerate()
            .map(|(index, file)| {
                div()
                    .h(px(36.0))
                    .px(px(10.0))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        mono((open + index + 1).to_string())
                            .w(px(16.0))
                            .text_color(theme.faint),
                    )
                    .child(icon(Icon::File, theme.muted))
                    .child(
                        div()
                            .flex_1()
                            .truncate()
                            .child(crate::app::recent_title(file)),
                    )
            })
            .collect();
        let mut files: Vec<_> = self
            .tabs
            .entries
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                div()
                    .h(px(36.0))
                    .px(px(10.0))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        mono((index + 1).to_string())
                            .w(px(16.0))
                            .text_color(theme.faint),
                    )
                    .child(icon(Icon::File, theme.muted))
                    .child(div().flex_1().truncate().child(tab.title()))
                    .child(
                        mono(if tab.count > 0 {
                            format!("{} pages", tab.count)
                        } else {
                            String::new()
                        })
                        .text_color(theme.faint),
                    )
            })
            .collect();
        files.extend(added);
        let empty = files.is_empty();
        div()
            .flex()
            .flex_col()
            .child(self.dialog_header("Combine into one PDF", cx))
            .child(
                div()
                    .px(px(16.0))
                    .pb(px(16.0))
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .children(files)
                    .when(empty, |list| {
                        list.child(
                            div()
                                .py(px(8.0))
                                .text_size(px(12.0))
                                .text_color(theme.muted)
                                .child("Open tabs appear here. Add more files to combine them."),
                        )
                    })
                    .child(
                        div().flex().child(
                            icon_button("combine-add", Icon::Plus, &theme, Tone::Plain, true)
                                .child("Add files")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.choose_combine_files(window, cx)
                                })),
                        ),
                    )
                    .child(form_row(
                        &theme,
                        "Image pages",
                        self.choice_pills(
                            "combine-fit",
                            vec![
                                "Fit to A4".into(),
                                "Fit to Letter".into(),
                                "Original size".into(),
                            ],
                            self.chrome.combine_fit,
                            |this, index| this.chrome.combine_fit = index,
                            cx,
                        ),
                    )),
            )
            .child(
                self.dialog_footer(
                    Some(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme.muted)
                            .child(if self.tabs.entries.iter().all(|tab| tab.count > 0) {
                                format!(
                                    "Drag to reorder. {} pages",
                                    self.tabs.entries.iter().map(|tab| tab.count).sum::<usize>()
                                )
                            } else {
                                "Drag to reorder".to_string()
                            })
                            .into_any_element(),
                    ),
                    "Combine",
                    "Combining files",
                    Some(|this, window, cx| this.choose_combine_destination(window, cx)),
                    cx,
                ),
            )
            .into_any_element()
    }

    fn print_dialog(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.chrome.theme;
        let tab = self.tabs.current();
        let session = tab.map_or(0, |tab| tab.session);
        let page = tab.map_or(0, |tab| tab.page);
        let count = tab.map_or(0, |tab| tab.count);
        div()
            .flex()
            .flex_col()
            .child(self.dialog_header("Print", cx))
            .child(
                div()
                    .px(px(16.0))
                    .pb(px(16.0))
                    .flex()
                    .gap(px(20.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(8.0))
                            .child(self.thumbnail_element(session, page, 170.0, 220.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .child(
                                        icon_button(
                                            "print-previous",
                                            Icon::ChevronLeft,
                                            &theme,
                                            Tone::Plain,
                                            page > 0,
                                        )
                                        .on_click(
                                            cx.listener(|this, _, window, cx| {
                                                if let Some(page) =
                                                    this.tabs.current().map(|tab| tab.page)
                                                {
                                                    this.go(page.saturating_sub(1), window, cx)
                                                }
                                            }),
                                        ),
                                    )
                                    .child(
                                        mono(format!("{} of {count}", page + 1))
                                            .text_color(theme.muted),
                                    )
                                    .child(
                                        icon_button(
                                            "print-next",
                                            Icon::ChevronRight,
                                            &theme,
                                            Tone::Plain,
                                            page + 1 < count,
                                        )
                                        .on_click(
                                            cx.listener(|this, _, window, cx| {
                                                if let Some(page) =
                                                    this.tabs.current().map(|tab| tab.page)
                                                {
                                                    this.go(page + 1, window, cx)
                                                }
                                            }),
                                        ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(form_row(
                                &theme,
                                "Printer",
                                div()
                                    .text_color(theme.muted)
                                    .child("Chosen in the system dialog"),
                            ))
                            .child(form_row(
                                &theme,
                                "Pages",
                                self.choice_pills(
                                    "print-pages",
                                    vec!["All".into(), "Current".into()],
                                    self.chrome.print_pages,
                                    |this, index| this.chrome.print_pages = index,
                                    cx,
                                ),
                            ))
                            .child(form_row(&theme, "Copies", mono("1")))
                            .child(form_row(
                                &theme,
                                "Scale",
                                self.choice_pills(
                                    "print-scale",
                                    vec!["Fit".into(), "Actual size".into()],
                                    self.chrome.print_scale,
                                    |this, index| this.chrome.print_scale = index,
                                    cx,
                                ),
                            ))
                            .child(form_row(
                                &theme,
                                "Include",
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .child(
                                        toggle("print-notes", self.chrome.include_notes, &theme)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.chrome.include_notes =
                                                    !this.chrome.include_notes;
                                                cx.notify();
                                            })),
                                    )
                                    .child("Notes and stamps"),
                            ))
                            .child(form_row(
                                &theme,
                                "Sides",
                                self.choice_pills(
                                    "print-sides",
                                    vec!["One".into(), "Both".into()],
                                    self.chrome.print_sides,
                                    |this, index| this.chrome.print_sides = index,
                                    cx,
                                ),
                            )),
                    ),
            )
            .child(
                self.dialog_footer(
                    Some(
                        text_button("print-system", "System dialog", &theme, Kind::Quiet)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.chrome.overlay = Overlay::None;
                                this.unavailable("The system print dialog", cx)
                            }))
                            .into_any_element(),
                    ),
                    "Print",
                    "Printing",
                    Some(|this, _, cx| this.print_document(cx)),
                    cx,
                ),
            )
            .into_any_element()
    }

    fn shortcuts_dialog(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.chrome.theme;
        let m = modifier();
        let columns: [(&str, Vec<(&str, String)>); 3] = [
            (
                "Navigate",
                vec![
                    ("Command palette", format!("{m} K")),
                    ("Go to page", format!("{m} G")),
                    ("Next or previous page", "Right Left".into()),
                    ("First or last page", "Home End".into()),
                    ("Back or forward", back_forward()),
                    ("Next or previous tab", "Ctrl Tab".into()),
                    ("Open", format!("{m} O")),
                ],
            ),
            (
                "View",
                vec![
                    ("Zoom in or out", format!("{m} = -")),
                    ("Fit width", format!("{m} 0")),
                    ("Sidebar", format!("{m} B")),
                    ("Inspector", format!("{m} I")),
                    ("Agent", format!("{m} J")),
                    ("Close tab", format!("{m} W")),
                    ("Shortcuts", "?".into()),
                ],
            ),
            (
                "Edit",
                vec![
                    ("Find", format!("{m} F")),
                    ("Copy text", format!("{m} C")),
                    ("Select all on page", format!("{m} A")),
                    ("Annotate", format!("{m} Shift A")),
                    ("Organize pages", format!("{m} Shift O")),
                    ("Export", format!("{m} E")),
                ],
            ),
        ];
        div()
            .flex()
            .flex_col()
            .child(self.dialog_header("Keyboard shortcuts", cx))
            .child(
                div()
                    .px(px(16.0))
                    .pb(px(18.0))
                    .flex()
                    .gap(px(28.0))
                    .children(columns.into_iter().map(|(title, keys)| {
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .child(label(&theme, title).mt(px(8.0)).mb(px(6.0)))
                            .children(keys.into_iter().map(|(text, keys)| {
                                div()
                                    .h(px(26.0))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(8.0))
                                    .text_size(px(12.0))
                                    .text_color(theme.muted)
                                    .child(text)
                                    .child(kbd(&theme, keys))
                            }))
                    })),
            )
            .into_any_element()
    }
}

fn dirs_label() -> String {
    std::env::var_os("HOME").map_or_else(|| "Downloads".to_string(), |_| "~/Downloads".to_string())
}
