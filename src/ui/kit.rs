use super::{icon::Icon, theme::Theme};
use gpui::{
    Div, ElementId, FontWeight, Hsla, IntoElement, SharedString, Stateful, Svg, div, prelude::*,
    px, svg,
};

pub(crate) const RADIUS: f32 = 8.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    Plain,
    Active,
    Danger,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Quiet,
    Primary,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Status {
    Warn,
    Info,
}

pub(crate) fn icon(glyph: Icon, color: Hsla) -> Svg {
    svg()
        .path(glyph.path())
        .size(px(16.0))
        .flex_none()
        .text_color(color)
}

pub(crate) fn icon_button(
    id: impl Into<ElementId>,
    glyph: Icon,
    theme: &Theme,
    tone: Tone,
    enabled: bool,
) -> Stateful<Div> {
    let (color, background) = match tone {
        Tone::Plain => (theme.muted, None),
        Tone::Active => (theme.accent_strong, Some(theme.accent_soft)),
        Tone::Danger => (theme.fail, None),
    };
    let hover = if tone == Tone::Active {
        theme.accent_soft
    } else {
        theme.hover
    };
    div()
        .id(id)
        .h(px(28.0))
        .min_w(px(28.0))
        .px(px(6.0))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .rounded(px(RADIUS))
        .text_size(px(12.0))
        .text_color(if enabled { color } else { theme.faint })
        .when_some(background, |button, background| button.bg(background))
        .child(icon(glyph, if enabled { color } else { theme.faint }))
        .when(enabled, |button| {
            button.cursor_pointer().hover(move |style| style.bg(hover))
        })
}

pub(crate) fn text_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    theme: &Theme,
    kind: Kind,
) -> Stateful<Div> {
    let (color, background, hover, weight) = match kind {
        Kind::Quiet => (theme.muted, None, theme.hover, FontWeight::NORMAL),
        Kind::Primary => (
            theme.on_accent,
            Some(theme.accent),
            theme.accent_strong,
            FontWeight::SEMIBOLD,
        ),
    };
    div()
        .id(id)
        .h(px(32.0))
        .px(px(12.0))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .rounded(px(RADIUS))
        .text_size(px(13.0))
        .font_weight(weight)
        .text_color(color)
        .when_some(background, |button, background| button.bg(background))
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(label.into())
}

pub(crate) fn pill(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    theme: &Theme,
) -> Stateful<Div> {
    let hover = theme.hover;
    div()
        .id(id)
        .h(px(28.0))
        .px(px(10.0))
        .flex()
        .flex_none()
        .items_center()
        .rounded(px(RADIUS))
        .text_size(px(12.0))
        .text_color(if selected { theme.ink } else { theme.muted })
        .when(selected, |pill| pill.bg(theme.hover))
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(label.into())
}

pub(crate) fn floating(theme: &Theme) -> Div {
    div()
        .bg(theme.raised)
        .rounded(px(RADIUS))
        .shadow(theme.floating_shadow())
}

pub(crate) fn kbd(theme: &Theme, keys: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .px(px(6.0))
        .rounded(px(RADIUS))
        .bg(theme.hover)
        .font_family(Theme::mono_font())
        .text_size(px(11.0))
        .text_color(theme.muted)
        .child(keys.into())
}

pub(crate) fn mono(text: impl Into<SharedString>) -> Div {
    div()
        .font_family(Theme::mono_font())
        .text_size(px(12.0))
        .child(text.into())
}

pub(crate) fn label(theme: &Theme, text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(11.0))
        .text_color(theme.faint)
        .child(text.into().to_uppercase())
}

pub(crate) fn dot(color: Hsla, size: f32) -> Div {
    div().flex_none().size(px(size)).rounded_full().bg(color)
}

pub(crate) fn status(theme: &Theme, kind: Status, text: impl Into<SharedString>) -> Div {
    let (color, background) = match kind {
        Status::Warn => (theme.warn, theme.warn_soft),
        Status::Info => (theme.accent_strong, theme.accent_soft),
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.0))
        .h(px(22.0))
        .px(px(8.0))
        .rounded(px(RADIUS))
        .bg(background)
        .text_color(color)
        .text_size(px(12.0))
        .child(dot(color, 8.0))
        .child(text.into())
}

pub(crate) fn separator(theme: &Theme) -> Div {
    div()
        .flex_none()
        .w(px(1.0))
        .h(px(16.0))
        .mx(px(4.0))
        .bg(theme.hairline)
}

pub(crate) fn toggle(id: impl Into<ElementId>, on: bool, theme: &Theme) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .w(px(30.0))
        .h(px(18.0))
        .rounded_full()
        .p(px(2.0))
        .flex()
        .when(on, |track| track.justify_end())
        .bg(if on { theme.accent } else { theme.pressed })
        .cursor_pointer()
        .child(div().size(px(14.0)).rounded_full().bg(if on {
            theme.on_accent
        } else {
            theme.muted
        }))
}

pub(crate) fn empty_state(theme: &Theme, glyph: Icon, title: &str, body: &str) -> impl IntoElement {
    div()
        .p(px(16.0))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(icon(glyph, theme.faint))
        .child(
            div()
                .text_size(px(13.0))
                .text_color(theme.ink)
                .child(title.to_string()),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme.muted)
                .child(body.to_string()),
        )
}
