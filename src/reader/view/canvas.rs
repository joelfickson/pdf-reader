use super::sidebar::sheet_placeholder;
use super::{super::Reader, CANVAS_PADDING};
use crate::pdf::ViewKey;
use crate::reader::actions::Open;
use crate::reader::column::Column;
use crate::ui::{
    icon::Icon,
    kit::clickable,
    kit::{Kind, icon, text_button},
};
use gpui::{AnyElement, Context, IntoElement, Window, div, prelude::*, px};

impl Reader {
    pub(in crate::reader) fn canvas(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.chrome.theme;
        let content = if let Some(error) = self.tabs.current().and_then(|tab| tab.error.as_ref()) {
            div()
                .mt(px(120.0))
                .w(px(420.0))
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(icon(Icon::File, theme.faint))
                .child(
                    div()
                        .text_size(px(17.0))
                        .text_color(theme.ink)
                        .child("This PDF could not be displayed"),
                )
                .child(div().text_color(theme.muted).child(error.clone()))
                .child(
                    div().flex().child(
                        text_button("error-open", "Open another PDF", &theme, Kind::Primary)
                            .on_click(|_, window, cx| window.dispatch_action(Box::new(Open), cx)),
                    ),
                )
                .into_any_element()
        } else if let Some(column) = self.column(window) {
            self.column_element(&column, window, cx)
        } else if let Some(image) = &self.image {
            let page = self.tabs.current().map_or(0, |tab| tab.page);
            self.page_element(page, image, self.text.as_ref(), cx)
                .into_any_element()
        } else {
            div()
                .mt(px(120.0))
                .text_color(theme.muted)
                .child("Opening page")
                .into_any_element()
        };
        clickable("document-canvas")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_scroll()
            .when_some(self.tabs.current(), |canvas, tab| {
                canvas.track_scroll(&tab.scroll)
            })
            .on_scroll_wheel(cx.listener(|this, _, _, cx| {
                if let Some(tab) = this.tabs.current_mut() {
                    tab.follow_scroll = true;
                }
                cx.notify();
            }))
            .child(
                div()
                    .min_w_full()
                    .min_h_full()
                    .pt(px(28.0))
                    .px(px(CANVAS_PADDING))
                    .pb(px(96.0))
                    .flex()
                    .items_start()
                    .child(div().mx_auto().flex_shrink_0().child(content)),
            )
    }

    fn column_element(
        &mut self,
        column: &Column,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(tab) = self.tabs.current() else {
            return div().into_any_element();
        };
        let (session, current, zoom) = (tab.session, tab.page, tab.zoom);
        let scale = (window.scale_factor() * 100.0).round() as u32;
        let (top, height) = self.viewport(window);
        let pages: Vec<AnyElement> = column
            .around(top, height)
            .map(|page| {
                let view = ViewKey {
                    session,
                    page,
                    available_width: column.view_width(page),
                    zoom,
                    scale,
                };
                let (width, height) = (column.widths[page], column.heights[page]);
                let content = if page == current && self.image_key.as_ref() == Some(&view) {
                    self.image.as_ref().map(|image| {
                        self.page_element(page, image, self.text.as_ref(), cx)
                            .into_any_element()
                    })
                } else {
                    self.pages.peek(&view).map(|image| {
                        self.page_element(page, image, self.texts.peek(&(session, page)), cx)
                            .into_any_element()
                    })
                };
                div()
                    .absolute()
                    .top(px(column.tops[page]))
                    .left(px((column.width - width) / 2.0))
                    .w(px(width))
                    .h(px(height))
                    .child(content.unwrap_or_else(|| {
                        sheet_placeholder(width, height, page).into_any_element()
                    }))
                    .into_any_element()
            })
            .collect();
        div()
            .relative()
            .w(px(column.width))
            .h(px(column.height))
            .children(pages)
            .into_any_element()
    }
}
