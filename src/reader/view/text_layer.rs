use super::super::Reader;
use crate::ui::kit::clickable;
use gpui::{Bounds, Context, IntoElement, MouseButton, canvas, img, point, prelude::*, px, size};

impl Reader {
    pub(in crate::reader) fn page_element(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut page = clickable("rendered-page").relative().flex_shrink_0();
        if let Some(image) = &self.image {
            let rects: Vec<_> = match (&self.text, &self.selection) {
                (Some(Ok(text)), Some(selection)) => text
                    .glyphs
                    .iter()
                    .skip(selection.range().start)
                    .take(selection.range().len())
                    .filter_map(|glyph| glyph.bounds)
                    .collect(),
                _ => Vec::new(),
            };
            let page_bounds = self.page_bounds.clone();
            let select = self.chrome.theme.select;
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
                            for rect in rects {
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
                                window.paint_quad(gpui::fill(highlight, select));
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
