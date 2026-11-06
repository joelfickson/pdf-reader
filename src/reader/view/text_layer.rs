use super::super::Reader;
use crate::ui::kit::clickable;
use gpui::{Bounds, Context, IntoElement, MouseButton, canvas, img, point, prelude::*, px, size};
use std::ops::Range;

impl Reader {
    pub(in crate::reader) fn page_element(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut page = clickable("rendered-page").relative().flex_shrink_0();
        if let Some(image) = &self.image {
            let theme = self.chrome.theme;
            let mut rects = Vec::new();
            if let Some(Ok(text)) = &self.text {
                let glyphs = |range: Range<usize>| {
                    text.glyphs
                        .get(range.start.min(text.glyphs.len())..range.end.min(text.glyphs.len()))
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|glyph| glyph.bounds)
                };
                if let (Some(search), Some(tab)) = (&self.search, self.tabs.current())
                    && search.session == tab.session
                {
                    for index in search.page_hits(tab.page) {
                        let color = if search.current == Some(index) {
                            theme.found_current
                        } else {
                            theme.found
                        };
                        rects.extend(
                            glyphs(search.hits[index].1.glyphs.clone()).map(|rect| (rect, color)),
                        );
                    }
                }
                if let Some(selection) = &self.selection {
                    rects.extend(glyphs(selection.range()).map(|rect| (rect, theme.select)));
                }
            }
            let page_bounds = self.page_bounds.clone();
            page = page
                .w(px(image.width))
                .h(px(image.height))
                .when(
                    matches!(&self.text, Some(Ok(text)) if !text.glyphs.is_empty()),
                    |page| page.cursor(gpui::CursorStyle::IBeam),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                        this.select_at(event.position, this.page_bounds.get(), true, window, cx);
                        cx.stop_propagation();
                    }),
                )
                .child(img(image.image.clone()).size_full().rounded(px(2.0)))
                .child(
                    canvas(
                        move |bounds, _, _| {
                            page_bounds.set(bounds);
                        },
                        move |bounds, _, window, _| {
                            for (rect, color) in rects {
                                let highlight = Bounds::new(
                                    point(
                                        bounds.origin.x + bounds.size.width * rect.left,
                                        bounds.origin.y + bounds.size.height * rect.top,
                                    ),
                                    size(
                                        bounds.size.width * (rect.right - rect.left),
                                        bounds.size.height * (rect.bottom - rect.top),
                                    ),
                                );
                                window.paint_quad(gpui::fill(highlight, color));
                            }
                        },
                    )
                    .absolute()
                    .inset_0()
                    .size_full(),
                );
        }
        page
    }
}
