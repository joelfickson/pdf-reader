use super::super::{
    Reader,
    chrome::Overlay,
    updates::{Phase, elapsed, megabytes},
};
use crate::ui::{
    icon::Icon,
    kit::{Kind, RADIUS, clickable, dot, floating, mono, text_button},
    theme::Theme,
};
use crate::update::{Channel, current_label};
use gpui::{
    AnyElement, Context, Div, FontWeight, IntoElement, MouseButton, deferred, div, prelude::*, px,
};

const METER_WIDTH: f32 = 260.0;

fn line(theme: &Theme, text: impl Into<gpui::SharedString>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .text_size(px(13.0))
        .text_color(theme.ink)
        .child(text.into())
}

fn marked(theme: &Theme, color: gpui::Hsla, text: impl Into<gpui::SharedString>) -> Div {
    div()
        .w_full()
        .flex()
        .items_start()
        .gap(px(8.0))
        .text_size(px(13.0))
        .text_color(theme.ink)
        .child(div().pt(px(6.0)).child(dot(color, 7.0)))
        .child(div().flex_1().min_w_0().child(text.into()))
}

fn row(theme: &Theme, key: &str, value: impl IntoElement) -> Div {
    div()
        .mt(px(14.0))
        .flex()
        .items_start()
        .gap(px(10.0))
        .child(
            div()
                .w(px(120.0))
                .flex_none()
                .pt(px(2.0))
                .text_size(px(12.0))
                .text_color(theme.muted)
                .child(key.to_string()),
        )
        .child(div().flex_1().min_w_0().child(value))
}

fn secondary(id: &'static str, text: &'static str, theme: &Theme) -> gpui::Stateful<Div> {
    text_button(id, text, theme, Kind::Quiet)
        .h(px(28.0))
        .bg(theme.hover)
        .text_color(theme.ink)
}

fn primary(id: &'static str, text: &'static str, theme: &Theme) -> gpui::Stateful<Div> {
    text_button(id, text, theme, Kind::Primary).h(px(28.0))
}

fn link(id: &'static str, text: &'static str, theme: &Theme) -> gpui::Stateful<Div> {
    clickable(id)
        .text_size(px(13.0))
        .text_color(theme.accent_strong)
        .cursor_pointer()
        .child(text)
}

impl Reader {
    pub(in crate::reader) fn update_control(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.updates.shows_control() {
            return None;
        }
        let theme = self.chrome.theme;
        let hover = theme.hover;
        let installing = matches!(self.updates.phase, Phase::Installing { .. });
        let popover = (self.chrome.overlay == Overlay::Update && !installing)
            .then(|| self.update_popover(cx))
            .flatten();
        Some(
            div()
                .relative()
                .child(
                    clickable("update-ready")
                        .h(px(28.0))
                        .px(px(10.0))
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .rounded(px(RADIUS))
                        .text_size(px(13.0))
                        .text_color(theme.accent_strong)
                        .cursor_pointer()
                        .hover(move |style| style.bg(hover))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !installing {
                                this.set_overlay(Overlay::Update, cx)
                            }
                        }))
                        .child(dot(theme.accent, 6.0))
                        .child(if installing {
                            "Installing…"
                        } else {
                            "Update ready"
                        }),
                )
                .children(popover)
                .into_any_element(),
        )
    }

    fn update_popover(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let release = self.updates.ready()?;
        let theme = self.chrome.theme;
        let notes = release.notes.iter().map(|note| {
            div()
                .flex()
                .items_start()
                .gap(px(8.0))
                .mb(px(4.0))
                .child(div().pt(px(7.0)).child(dot(theme.ink, 4.0)))
                .child(div().flex_1().min_w_0().child(note.clone()))
        });
        let card = floating(&theme)
            .debug_selector(|| "update-popover".into())
            .id("update-popover")
            .w(px(340.0))
            .p(px(16.0))
            .text_size(px(13.0))
            .text_color(theme.ink)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb(px(2.0))
                    .child(format!("PDF Reader {} is ready", release.version)),
            )
            .child(div().text_color(theme.muted).mb(px(12.0)).child(format!(
                "You have {}. Downloaded and verified.",
                current_label()
            )))
            .when(!release.notes.is_empty(), |card| {
                card.child(div().mb(px(10.0)).children(notes))
            })
            .child(
                div()
                    .h(px(1.0))
                    .mx(px(-16.0))
                    .mb(px(12.0))
                    .bg(theme.hairline),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        primary("update-restart", "Restart to update", &theme)
                            .on_click(cx.listener(|this, _, _, cx| this.restart_to_update(cx))),
                    )
                    .child(
                        secondary("update-later", "Later", &theme)
                            .on_click(cx.listener(|this, _, _, cx| this.dismiss_update(cx))),
                    )
                    .child(
                        link("update-notes", "Release notes", &theme)
                            .ml_auto()
                            .on_click(cx.listener(|this, _, _, cx| this.open_release_notes(cx))),
                    ),
            );
        Some(
            deferred(div().absolute().top(px(34.0)).right_0().child(card))
                .with_priority(1)
                .into_any_element(),
        )
    }

    fn update_status(&self, cx: &mut Context<Self>) -> Div {
        let theme = self.chrome.theme;
        let label = current_label();
        let check = |id, text| {
            secondary(id, text, &theme)
                .on_click(cx.listener(|this, _, _, cx| this.check_for_updates_now(cx)))
        };
        let column = div().flex().flex_col().items_start().gap(px(6.0));
        match &self.updates.phase {
            Phase::Unavailable(reason) => column
                .child(line(&theme, format!("PDF Reader {label}")))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(theme.faint)
                        .child(*reason),
                ),
            Phase::Idle => column
                .child(line(&theme, format!("You have PDF Reader {label}")))
                .child(check("update-check", "Check now")),
            Phase::Checking => column.child(line(&theme, "Checking for updates…")),
            Phase::Current { checked } => column
                .child(marked(
                    &theme,
                    theme.ok,
                    format!("PDF Reader {label} is the latest version"),
                ))
                .child(mono(elapsed(*checked)).text_color(theme.muted))
                .child(check("update-check", "Check now")),
            Phase::Downloading {
                version,
                received,
                total,
            } => {
                let fraction = if *total == 0 {
                    0.0
                } else {
                    (*received as f32 / *total as f32).clamp(0.0, 1.0)
                };
                column
                    .child(line(&theme, format!("Downloading {version}")))
                    .child(
                        div()
                            .w(px(METER_WIDTH))
                            .h(px(3.0))
                            .rounded(px(2.0))
                            .bg(theme.hover)
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    .w(px(METER_WIDTH * fraction))
                                    .bg(theme.accent),
                            ),
                    )
                    .child(
                        mono(format!(
                            "{} of {} MB",
                            megabytes(*received),
                            megabytes(*total)
                        ))
                        .text_color(theme.muted),
                    )
            }
            Phase::Ready { release, .. } => column
                .child(marked(
                    &theme,
                    theme.accent,
                    format!("{} is ready to install. You have {label}.", release.version),
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            primary("settings-update-restart", "Restart to update", &theme)
                                .on_click(cx.listener(|this, _, _, cx| this.restart_to_update(cx))),
                        )
                        .child(
                            link("settings-update-notes", "Release notes", &theme).on_click(
                                cx.listener(|this, _, _, cx| this.open_release_notes(cx)),
                            ),
                        ),
                ),
            Phase::Installing { version } => column.child(line(
                &theme,
                format!("Installing {version}. PDF Reader will reopen in a moment."),
            )),
            Phase::Failed { message, checked } => {
                let retry = if self.updates.settings.automatic {
                    ". Will retry in 6 hours."
                } else {
                    "."
                };
                column
                    .child(marked(&theme, theme.fail, message.clone()))
                    .child(mono(format!("{}{retry}", elapsed(*checked))).text_color(theme.muted))
                    .child(check("update-retry", "Try again"))
            }
        }
    }

    pub(super) fn update_settings(&self, cx: &mut Context<Self>) -> Div {
        let theme = self.chrome.theme;
        let settings = self.updates.settings;
        let automatic = settings.automatic;
        let checkbox = div()
            .flex_none()
            .size(px(16.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .bg(if automatic {
                theme.accent
            } else {
                theme.pressed
            })
            .when(automatic, |checkbox| {
                checkbox.child(
                    gpui::svg()
                        .path(Icon::Check.path())
                        .size(px(12.0))
                        .text_color(theme.on_accent),
                )
            });
        let channels = div()
            .flex()
            .p(px(2.0))
            .rounded(px(RADIUS))
            .bg(theme.chrome)
            .children(
                [("Stable", Channel::Stable), ("Beta", Channel::Beta)]
                    .into_iter()
                    .map(|(text, channel)| {
                        let selected = settings.channel == channel;
                        let ink = theme.ink;
                        clickable(("update-channel", channel as usize))
                            .h(px(24.0))
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .rounded(px(6.0))
                            .text_size(px(12.0))
                            .text_color(if selected { theme.ink } else { theme.muted })
                            .when(selected, |item| item.bg(theme.hover))
                            .cursor_pointer()
                            .hover(move |style| style.text_color(ink))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_update_channel(channel, cx)
                            }))
                            .child(text)
                    }),
            );
        div()
            .child(row(
                &theme,
                "Check",
                div()
                    .child(
                        clickable("update-automatic")
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .text_size(px(13.0))
                            .text_color(theme.ink)
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_automatic_updates(!automatic, cx)
                            }))
                            .child(checkbox)
                            .child("Download new versions automatically"),
                    )
                    .child(div().mt(px(4.0)).text_size(px(12.0)).text_color(theme.faint).child(
                        "Asks GitHub for the latest release on launch and every 6 hours. Sends no document data.",
                    )),
            ))
            .child(row(&theme, "Channel", channels))
            .child(row(&theme, "Version", self.update_status(cx)))
    }
}
