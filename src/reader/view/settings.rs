use super::super::{
    Reader,
    chrome::{Layout, Overlay},
};
use super::dialogs::form_row;
use crate::preferences::{DefaultZoom, NightPages, Preferences};
use crate::ui::{
    kit::clickable,
    kit::{RADIUS, Status, label, pill, status, toggle},
    theme::ThemeChoice,
};
use gpui::{AnyElement, Context, IntoElement, Window, div, prelude::*, px};

const SECTIONS: [&str; 5] = [
    "General",
    "Reading",
    "Notes and signing",
    "Shortcuts",
    "Updates",
];

impl Reader {
    fn preference_toggle(
        &self,
        id: &'static str,
        text: &'static str,
        on: bool,
        set: fn(&mut Preferences, bool),
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .child(toggle(id, on, &self.chrome.theme).on_click(cx.listener(
                move |this, _, _, cx| {
                    this.update_preferences(|preferences| set(preferences, !on), cx)
                },
            )))
            .child(text)
    }

    fn default_zoom_choices(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        let options = [
            ("Fit width", DefaultZoom::FitWidth),
            ("Fit page", DefaultZoom::FitPage),
            ("100%", DefaultZoom::Actual),
        ];
        div()
            .flex()
            .gap(px(2.0))
            .children(
                options
                    .into_iter()
                    .enumerate()
                    .map(|(index, (text, zoom))| {
                        let selected = self.preferences.default_zoom == zoom;
                        pill(("default-zoom", index), text, selected, &theme).on_click(cx.listener(
                            move |this, _, _, cx| {
                                this.update_preferences(
                                    |preferences| preferences.default_zoom = zoom,
                                    cx,
                                )
                            },
                        ))
                    }),
            )
    }

    fn night_choices(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        let options = [
            ("Never", NightPages::Never),
            ("Follow theme", NightPages::FollowTheme),
            ("Always", NightPages::Always),
        ];
        div()
            .flex()
            .gap(px(2.0))
            .children(
                options
                    .into_iter()
                    .enumerate()
                    .map(|(index, (text, night))| {
                        let selected = self.preferences.night == night;
                        pill(("night-pages", index), text, selected, &theme).on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.update_preferences(
                                    |preferences| preferences.night = night,
                                    cx,
                                );
                                this.apply_night_setting(window, cx);
                            },
                        ))
                    }),
            )
    }

    fn layout_choices(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        let options = [
            ("Single page", Layout::Single),
            ("Continuous", Layout::Continuous),
            ("Two pages", Layout::TwoPages),
        ];
        div()
            .flex()
            .gap(px(2.0))
            .children(
                options
                    .into_iter()
                    .enumerate()
                    .map(|(index, (text, layout))| {
                        let selected = layout == self.chrome.layout
                            || (layout == Layout::TwoPages
                                && self.chrome.layout == Layout::CoverFirst);
                        pill(("default-layout", index), text, selected, &theme).on_click(
                            cx.listener(move |this, _, window, cx| {
                                this.set_layout(layout, window, cx)
                            }),
                        )
                    }),
            )
    }

    pub(super) fn settings_dialog(&self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.chrome.theme;
        let section = self.chrome.settings_section;
        let nav = div()
            .w(px(170.0))
            .flex_none()
            .p(px(8.0))
            .bg(theme.chrome)
            .flex()
            .flex_col()
            .gap(px(2.0))
            .children(SECTIONS.iter().enumerate().map(|(index, name)| {
                let selected = index == section;
                let hover = theme.hover;
                clickable(("settings-section", index))
                    .h(px(30.0))
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .rounded(px(RADIUS))
                    .text_color(if selected { theme.ink } else { theme.muted })
                    .when(selected, |item| item.bg(theme.raised))
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if SECTIONS[index] == "Shortcuts" {
                            this.set_overlay(Overlay::Shortcuts, cx);
                        } else {
                            this.chrome.settings_section = index;
                            cx.notify();
                        }
                    }))
                    .child(*name)
            }));
        let heading = |text: &'static str| label(&theme, text).mt(px(20.0)).mb(px(2.0));
        let body = match section {
            0 => div()
                .child(heading("Appearance"))
                .child(form_row(
                    &theme,
                    "Theme",
                    div().flex().gap(px(2.0)).children(
                        [
                            ("System", ThemeChoice::System),
                            ("Light", ThemeChoice::Light),
                            ("Dark", ThemeChoice::Dark),
                        ]
                        .into_iter()
                        .map(|(text, choice)| {
                            pill(text, text, self.chrome.theme_choice == choice, &theme).on_click(
                                cx.listener(move |this, _, window, cx| {
                                    this.set_theme_choice(choice, window, cx)
                                }),
                            )
                        }),
                    ),
                ))
                .child(heading("Startup"))
                .child(form_row(
                    &theme,
                    "On launch",
                    self.preference_toggle(
                        "reopen-tabs",
                        "Reopen last session's tabs",
                        self.preferences.reopen_tabs,
                        |preferences, on| preferences.reopen_tabs = on,
                        cx,
                    ),
                ))
                .child(form_row(
                    &theme,
                    "Each file",
                    self.preference_toggle(
                        "last-page",
                        "Open at the last page read",
                        self.preferences.last_page,
                        |preferences, on| preferences.last_page = on,
                        cx,
                    ),
                ))
                .child(form_row(
                    &theme,
                    "Default app",
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(status(&theme, Status::Warn, "Not set"))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(theme.muted)
                                .child("Setting the default PDF app is not available yet"),
                        ),
                )),
            1 => div()
                .child(heading("Reading"))
                .child(form_row(
                    &theme,
                    "Default zoom",
                    self.default_zoom_choices(cx),
                ))
                .child(form_row(&theme, "Layout", self.layout_choices(cx)))
                .child(form_row(&theme, "Night pages", self.night_choices(cx))),
            2 => div()
                .child(heading("Notes"))
                .child(form_row(
                    &theme,
                    "Author name",
                    div().text_color(theme.faint).child("Not available yet"),
                ))
                .child(form_row(
                    &theme,
                    "Saving",
                    self.preference_toggle(
                        "autosave",
                        "Save notes automatically",
                        self.preferences.autosave,
                        |preferences, on| preferences.autosave = on,
                        cx,
                    ),
                ))
                .child(heading("Signing"))
                .child(form_row(
                    &theme,
                    "Signatures",
                    div()
                        .text_color(theme.faint)
                        .child("Saved signatures are not available yet"),
                )),
            _ => div()
                .child(heading("Updates"))
                .child(self.update_settings(cx)),
        };
        div()
            .flex()
            .flex_col()
            .child(self.dialog_header("Settings", cx))
            .child(
                div()
                    .flex()
                    .min_h(px(420.0))
                    .child(nav)
                    .child(div().flex_1().px(px(20.0)).pb(px(20.0)).child(body)),
            )
            .into_any_element()
    }
}
