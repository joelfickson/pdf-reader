use super::super::{Reader, TextResult, images::PageImage, signing::OUTLINE_OFFSET};
use super::ink::paint_strokes;
use crate::pdf::{
    markup::{Edit, MarkupKind, STROKE_WEIGHT},
    text::TextRect,
};
use crate::ui::{
    kit::{RADIUS, clickable},
    theme::Paper,
};

const LOUPE_WIDTH: f32 = 300.0;
const LOUPE_HEIGHT: f32 = 150.0;
const LOUPE_SCALE: f32 = 2.0;
use gpui::{
    Bounds, Context, CursorStyle, HitboxBehavior, IntoElement, MouseButton, Pixels, canvas, div,
    img, point, prelude::*, px, size,
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
        let live = self.live_sketch(page);
        let placing = self
            .placing_signature()
            .filter(|signature| signature.page == page)
            .map(|signature| signature.bounds);
        let mut signatures = Vec::new();
        let mut stamps = Vec::new();
        let mut pictures = Vec::new();
        for edit in self.tabs.current().map_or(&[][..], |tab| tab.unsaved()) {
            let markup = match edit {
                Edit::Markup(markup) => markup,
                Edit::Signature(signature) => {
                    if signature.page == page {
                        signatures.push((
                            signature.bounds,
                            gpui::Hsla::from(gpui::rgb(signature.color)),
                            signature.strokes.clone(),
                            signature.weight,
                        ));
                    }
                    continue;
                }
                Edit::Stamp(stamp) => {
                    if stamp.page == page {
                        stamps.push(stamp.clone());
                    }
                    continue;
                }
                Edit::Picture(picture) => {
                    if picture.page == page
                        && let Some(preview) = self.preview_of(&picture.pixels)
                    {
                        pictures.push((picture.bounds, preview));
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
        if let Some(live) = live {
            signatures.push((
                live.bounds,
                gpui::Hsla::from(gpui::rgb(live.color)),
                live.strokes.clone(),
                live.weight,
            ));
        }
        let loupe = self
            .pointer
            .filter(|_| self.chrome.magnifier)
            .zip(self.page_frames.borrow().get(&page).copied())
            .filter(|(pointer, frame)| frame.contains(pointer))
            .map(|(pointer, frame)| {
                let x = f32::from(pointer.x - frame.origin.x);
                let y = f32::from(pointer.y - frame.origin.y);
                gpui::deferred(
                    div()
                        .absolute()
                        .left(px(x - LOUPE_WIDTH / 2.0))
                        .top(px(y - LOUPE_HEIGHT / 2.0))
                        .w(px(LOUPE_WIDTH))
                        .h(px(LOUPE_HEIGHT))
                        .rounded(px(RADIUS))
                        .overflow_hidden()
                        .bg(Paper::base())
                        .border_2()
                        .border_color(theme.accent)
                        .shadow(theme.floating_shadow())
                        .child(
                            img(image.image.clone())
                                .absolute()
                                .left(px(LOUPE_WIDTH / 2.0 - x * LOUPE_SCALE))
                                .top(px(LOUPE_HEIGHT / 2.0 - y * LOUPE_SCALE))
                                .w(px(image.width * LOUPE_SCALE))
                                .h(px(image.height * LOUPE_SCALE)),
                        )
                        .child(
                            div()
                                .absolute()
                                .right(px(8.0))
                                .bottom(px(8.0))
                                .px(px(6.0))
                                .rounded(px(RADIUS))
                                .bg(Paper::rule())
                                .font_family(crate::ui::theme::Theme::mono_font())
                                .text_size(px(11.0))
                                .text_color(Paper::muted())
                                .child("2x"),
                        ),
                )
                .with_priority(2)
            });
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
                    if this.placement_press(page, event.position, pressed_bounds.get(), cx)
                        || this.sketch_press(page, event.position, pressed_bounds.get(), cx)
                    {
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
            .children(pictures.into_iter().map(|(bounds, preview)| {
                img(preview)
                    .absolute()
                    .left(px(image.width * bounds.left))
                    .top(px(image.height * bounds.top))
                    .w(px(image.width * (bounds.right - bounds.left)))
                    .h(px(image.height * (bounds.bottom - bounds.top)))
            }))
            .children(stamps.into_iter().map(|stamp| {
                let color: gpui::Hsla = gpui::rgb(stamp.color).into();
                let bounds = stamp.bounds;
                let height = image.height * (bounds.bottom - bounds.top);
                div()
                    .absolute()
                    .left(px(image.width * bounds.left))
                    .top(px(image.height * bounds.top))
                    .w(px(image.width * (bounds.right - bounds.left)))
                    .h(px(height))
                    .border_color(color)
                    .border(px((height * 0.06).max(1.0)))
                    .rounded(px(3.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .text_color(color)
                    .text_size(px(height * 0.46))
                    .font_weight(gpui::FontWeight::EXTRA_BOLD)
                    .child(stamp.label)
            }))
            .children(loupe)
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
                        for (rect, color, strokes, weight) in &signatures {
                            let frame = place(bounds, rect);
                            let weight = match weight {
                                Some(weight) => bounds.size.width * *weight,
                                None => frame.size.width * STROKE_WEIGHT,
                            }
                            .max(px(0.75));
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
