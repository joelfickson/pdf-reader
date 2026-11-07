use super::super::Reader;
use crate::pdf::text::TextRect;
use crate::ui::kit::clickable;
use gpui::{
    Bounds, Context, CursorStyle, HitboxBehavior, IntoElement, MouseButton, Pixels, canvas, img,
    point, prelude::*, px, size,
};
use std::ops::Range;

impl Reader {
    pub(in crate::reader) fn page_element(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut page = clickable("rendered-page").relative().flex_shrink_0();
        if let Some(image) = &self.image {
            let theme = self.chrome.theme;
            let mut rects = Vec::new();
            let mut links = Vec::new();
            if let Some(Ok(text)) = &self.text {
                links.extend(text.links.iter().map(|link| link.bounds));
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
                        this.pressed_link = this.link_at(event.position);
                        if this.pressed_link.is_some() {
                            this.focus.focus(window);
                        } else {
                            this.select_at(
                                event.position,
                                this.page_bounds.get(),
                                true,
                                window,
                                cx,
                            );
                        }
                        cx.stop_propagation();
                    }),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, event: &gpui::MouseUpEvent, window, cx| {
                        if let Some(pressed) = this.pressed_link.take()
                            && this.link_at(event.position).as_ref() == Some(&pressed)
                        {
                            this.follow_link(pressed, window, cx);
                        }
                    }),
                )
                .child(img(image.image.clone()).size_full().rounded(px(2.0)))
                .child(
                    canvas(
                        move |bounds, window, _| {
                            page_bounds.set(bounds);
                            links
                                .iter()
                                .map(|rect| {
                                    window
                                        .insert_hitbox(place(bounds, rect), HitboxBehavior::Normal)
                                })
                                .collect::<Vec<_>>()
                        },
                        move |bounds, hitboxes, window, cx| {
                            for (rect, color) in rects {
                                window.paint_quad(gpui::fill(place(bounds, &rect), color));
                            }
                            for hitbox in &hitboxes {
                                window.set_cursor_style(CursorStyle::PointingHand, hitbox);
                            }
                            crate::bench::page_painted(cx);
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

fn place(bounds: Bounds<Pixels>, rect: &TextRect) -> Bounds<Pixels> {
    Bounds::new(
        point(
            bounds.origin.x + bounds.size.width * rect.left,
            bounds.origin.y + bounds.size.height * rect.top,
        ),
        size(
            bounds.size.width * (rect.right - rect.left),
            bounds.size.height * (rect.bottom - rect.top),
        ),
    )
}
