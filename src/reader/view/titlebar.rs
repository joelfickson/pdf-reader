use super::super::{
    Reader,
    actions::*,
    chrome::{InspectorTab, Mode, Overlay},
    commands::modifier,
};
use crate::ui::{
    icon::Icon,
    kit::clickable,
    kit::{RADIUS, Tone, dot, icon, icon_button, kbd},
};
use gpui::{Context, IntoElement, MouseButton, Window, div, prelude::*, px};

impl Reader {
    pub(in crate::reader) fn titlebar(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let traffic_lights = cfg!(target_os = "macos") && !window.is_fullscreen();
        let theme = self.chrome.theme;
        let has_document = self.has_document();
        let has_tab = self.tabs.current().is_some();
        let tone = |active: bool| if active { Tone::Active } else { Tone::Plain };
        let hover = theme.hover;
        let tabs = self.tabs.entries.iter().enumerate().map(|(index, tab)| {
            let active = index == self.tabs.active;
            clickable(("tab", tab.session))
                .group("tab")
                .h(px(30.0))
                .flex_basis(px(220.0))
                .flex_shrink()
                .min_w(px(110.0))
                .max_w(px(240.0))
                .pl(px(10.0))
                .pr(px(4.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .rounded(px(RADIUS))
                .text_size(px(13.0))
                .text_color(if active { theme.ink } else { theme.muted })
                .when(active, |item| item.bg(theme.raised))
                .cursor_pointer()
                .hover(move |style| if active { style } else { style.bg(hover) })
                .on_click(
                    cx.listener(move |this, _, window, cx| this.activate_tab(index, window, cx)),
                )
                .child(div().flex_1().min_w_0().truncate().child(tab.title()))
                .child(
                    if tab.modified() {
                        clickable(("close", tab.session))
                            .relative()
                            .w(px(22.0))
                            .min_w(px(22.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(RADIUS))
                            .cursor_pointer()
                            .child(
                                dot(theme.accent, 8.0)
                                    .group_hover("tab", |style| style.invisible()),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .invisible()
                                    .group_hover("tab", |style| style.visible())
                                    .child(icon(Icon::Close, theme.faint)),
                            )
                    } else {
                        icon_button(
                            ("close", tab.session),
                            Icon::Close,
                            &theme,
                            Tone::Plain,
                            true,
                        )
                        .w(px(22.0))
                        .min_w(px(22.0))
                        .h(px(22.0))
                        .px(px(3.0))
                        .when(!active, |button| {
                            button
                                .invisible()
                                .group_hover("tab", |style| style.visible())
                        })
                    }
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.close_tab(index, window, cx)
                    })),
                )
        });
        clickable("titlebar")
            .h(px(40.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(4.0))
            .pr(px(8.0))
            .pl(px(if traffic_lights { 80.0 } else { 8.0 }))
            .bg(theme.chrome)
            .on_click(|event, window, _| {
                if event.click_count() == 2 {
                    window.titlebar_double_click();
                }
            })
            .child(
                clickable("tabs")
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .overflow_x_scroll()
                    .children(tabs)
                    .child(
                        icon_button(
                            "new-tab",
                            Icon::Plus,
                            &theme,
                            Tone::Plain,
                            !self.picker_open,
                        )
                        .on_click(|_, window, cx| window.dispatch_action(Box::new(Open), cx)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(2.0))
                    .child(
                        icon_button(
                            "toggle-sidebar",
                            Icon::Sidebar,
                            &theme,
                            tone(self.sidebar_visible()),
                            has_document,
                        )
                        .on_click(|_, window, cx| {
                            window.dispatch_action(Box::new(ToggleSidebar), cx)
                        }),
                    )
                    .child(
                        icon_button(
                            "toggle-find",
                            Icon::Search,
                            &theme,
                            tone(self.chrome.find_open),
                            has_document,
                        )
                        .on_click(|_, window, cx| window.dispatch_action(Box::new(ToggleFind), cx)),
                    )
                    .child(
                        icon_button(
                            "toggle-annotate",
                            Icon::Pen,
                            &theme,
                            tone(self.chrome.mode == Mode::Annotate),
                            has_document,
                        )
                        .on_click(|_, window, cx| {
                            window.dispatch_action(Box::new(ToggleAnnotate), cx)
                        }),
                    )
                    .child(
                        icon_button(
                            "toggle-organize",
                            Icon::Grid,
                            &theme,
                            tone(self.chrome.mode == Mode::Organize),
                            has_document,
                        )
                        .on_click(|_, window, cx| {
                            window.dispatch_action(Box::new(ToggleOrganize), cx)
                        }),
                    )
                    .child(
                        icon_button(
                            "show-export",
                            Icon::Export,
                            &theme,
                            tone(self.chrome.overlay == Overlay::Export),
                            has_document,
                        )
                        .on_click(|_, window, cx| window.dispatch_action(Box::new(ShowExport), cx)),
                    )
                    .child(
                        icon_button(
                            "toggle-inspector",
                            Icon::Inspector,
                            &theme,
                            tone(self.chrome.inspector.is_some()),
                            has_tab,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            let tab = this.chrome.inspector.unwrap_or(InspectorTab::Document);
                            this.set_inspector(tab, window, cx)
                        })),
                    )
                    .children(self.update_control(cx))
                    .child({
                        let open = self.agent.open;
                        let soft = theme.accent_soft;
                        clickable("toggle-agent")
                            .h(px(28.0))
                            .px(px(10.0))
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .rounded(px(RADIUS))
                            .text_size(px(13.0))
                            .text_color(theme.accent_strong)
                            .when(open, |button| button.bg(soft))
                            .cursor_pointer()
                            .hover(move |style| style.bg(if open { soft } else { hover }))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(ToggleAgent), cx)
                            })
                            .child(icon(Icon::Note, theme.accent_strong))
                            .child("Agent")
                    })
                    .child(
                        clickable("palette-trigger")
                            .ml(px(4.0))
                            .h(px(28.0))
                            .w(px(180.0))
                            .pl(px(10.0))
                            .pr(px(6.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(8.0))
                            .rounded(px(RADIUS))
                            .bg(theme.raised)
                            .text_color(theme.muted)
                            .cursor_pointer()
                            .hover(move |style| style.bg(hover))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(TogglePalette), cx)
                            })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(icon(Icon::Search, theme.faint))
                                    .child("Go to anything"),
                            )
                            .child(kbd(&theme, format!("{} K", modifier()))),
                    ),
            )
    }
}
