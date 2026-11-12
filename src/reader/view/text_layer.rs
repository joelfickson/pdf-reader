use super::super::{Reader, TextResult, images::PageImage, signing::OUTLINE_OFFSET};
use super::ink::paint_strokes;
use crate::pdf::{
    markup::{Edit, MarkupKind, STROKE_WEIGHT},
    text::TextRect,
};
use crate::ui::{kit::clickable, theme::Paper};
use gpui::{
    Bounds, Context, CursorStyle, HitboxBehavior, IntoElement, MouseButton, Pixels, canvas, img,
    point, prelude::*, px, size,
};
use std::{cell::Cell, ops::Range, rc::Rc};

impl Reader {
    pub(in crate::reader) fn page_element(
        &self,
        page: usize,
        image: &PageImage,
        text: Option<&TextResult>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.chrome.theme;
        let current = self.tabs.current().is_some_and(|tab| tab.page == page);
        let mut rects = Vec::new();
        let mut links = Vec::new();
        let placing = self
            .placing_signature()
            .filter(|signature| signature.page == page)
            .map(|signature| signature.bounds);
        let mut signatures = Vec::new();
        for edit in self.tabs.current().map_or(&[][..], |tab| tab.unsaved()) {
            let markup = match edit {
                Edit::Markup(markup) => markup,
                Edit::Signature(signature) => {
                    if signature.page == page {
                        signatures.push((
                            signature.bounds,
                            gpui::Hsla::from(gpui::rgb(signature.color)),
                            signature.strokes.clone(),
                        ));
                    }
                    continue;
                }
            };
            let color: gpui::Hsla = gpui::rgb(markup.color).into();
            for rect in markup.on_page(page) {
                rects.push(match markup.kind {
                    MarkupKind::Highlight => (*rect, color.opacity(0.4)),
                    MarkupKind::Underline => (rule(rect, 0.94), color),
                    MarkupKind::Strikeout => (rule(rect, 0.55), color),
                });
            }
        }
        if let Some(Ok(text)) = text {
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
                for index in search.page_hits(page) {
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
                let range = selection.on_page(page, text.glyphs.len());
                rects.extend(glyphs(range).map(|rect| (rect, theme.select)));
            }
        }
        let page_bounds = if current {
            self.page_bounds.clone()
        } else {
            Rc::new(Cell::new(Bounds::default()))
        };
        let pressed_bounds = page_bounds.clone();
        let frames = self.page_frames.clone();
        clickable(("rendered-page", page))
            .relative()
            .flex_shrink_0()
            .w(px(image.width))
            .h(px(image.height))
            .when(
                matches!(text, Some(Ok(text)) if !text.glyphs.is_empty()),
                |element| element.cursor(gpui::CursorStyle::IBeam),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                    if this.placement_press(page, event.position, pressed_bounds.get(), cx) {
                        this.focus.focus(window);
                        cx.stop_propagation();
                        return;
                    }
                    if !current {
                        this.focus_page(page, window);
                        this.page_bounds.set(pressed_bounds.get());
                    }
                    this.pressed_link = this.link_at(event.position);
                    if this.pressed_link.is_some() {
                        this.focus.focus(window);
                    } else {
                        this.select_at(
                            page,
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
                        frames.borrow_mut().insert(page, bounds);
                        links
                            .iter()
                            .map(|rect| {
                                window.insert_hitbox(place(bounds, rect), HitboxBehavior::Normal)
                            })
                            .collect::<Vec<_>>()
                    },
                    move |bounds, hitboxes, window, cx| {
                        for (rect, color) in rects {
                            window.paint_quad(gpui::fill(place(bounds, &rect), color));
                        }
                        for (rect, color, strokes) in &signatures {
                            let frame = place(bounds, rect);
                            let weight = (frame.size.width * STROKE_WEIGHT).max(px(0.75));
                            paint_strokes(
                                window,
                                strokes,
                                |unit| {
                                    point(
                                        frame.origin.x + frame.size.width * unit[0],
                                        frame.origin.y + frame.size.height * unit[1],
                                    )
                                },
                                weight,
                                *color,
                            );
                        }
                        if let Some(rect) = &placing {
                            let outline = place(bounds, rect).dilate(px(OUTLINE_OFFSET));
                            window.paint_quad(gpui::quad(
                                outline,
                                px(2.0),
                                gpui::transparent_black(),
                                px(1.5),
                                theme.accent,
                                gpui::BorderStyle::Dashed,
                            ));
                            for corner in [
                                outline.origin,
                                outline.top_right(),
                                outline.bottom_left(),
                                outline.bottom_right(),
                            ] {
                                window.paint_quad(gpui::quad(
                                    Bounds::centered_at(corner, size(px(8.0), px(8.0))),
                                    px(2.0),
                                    Paper::base(),
                                    px(1.5),
                                    theme.accent,
                                    gpui::BorderStyle::Solid,
                                ));
                            }
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
            )
    }
}

fn rule(rect: &TextRect, position: f32) -> TextRect {
    let height = rect.bottom - rect.top;
    let center = rect.top + height * position;
    let half = height * 0.04;
    TextRect {
        left: rect.left,
        top: center - half,
        right: rect.right,
        bottom: center + half,
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
