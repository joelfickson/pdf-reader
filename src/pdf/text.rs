use anyhow::{Result, bail};
use pdfium_render::prelude::*;
use std::ops::Range;

#[derive(Clone, Copy, Debug)]
pub(crate) struct TextRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Debug)]
pub(crate) struct Glyph {
    pub value: char,
    pub bounds: Option<TextRect>,
    pub direction: [f32; 2],
}

#[derive(Debug)]
pub(crate) struct PageText {
    pub glyphs: Vec<Glyph>,
}

impl PageText {
    pub fn byte_len(&self) -> usize {
        self.glyphs.capacity() * std::mem::size_of::<Glyph>()
    }

    pub fn content(&self, range: Range<usize>) -> String {
        self.glyphs
            .get(range.start.min(self.glyphs.len())..range.end.min(self.glyphs.len()))
            .unwrap_or_default()
            .iter()
            .map(|glyph| glyph.value)
            .filter(|value| *value != '\r' && *value != '\0')
            .collect()
    }

    pub fn caret(&self, x: f32, y: f32, strict: bool) -> Option<usize> {
        let (index, bounds, distance) = self
            .glyphs
            .iter()
            .enumerate()
            .filter_map(|(index, glyph)| {
                let bounds = glyph.bounds?;
                let dx = (bounds.left - x).max(0.0).max(x - bounds.right);
                let dy = (bounds.top - y).max(0.0).max(y - bounds.bottom);
                Some((index, bounds, dx * dx + dy * dy * 4.0))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))?;
        if strict && distance > 0.0001 {
            return None;
        }
        let direction = self.glyphs[index].direction;
        let advance = (x - (bounds.left + bounds.right) / 2.0) * direction[0]
            + (y - (bounds.top + bounds.bottom) / 2.0) * direction[1];
        Some(index + usize::from(advance > 0.0))
    }
}

pub(super) fn extract(page: &PdfPage<'_>, current: impl Fn() -> bool) -> Result<PageText> {
    let text = page.text()?;
    let chars = text.chars();
    if chars.len() > 100_000 {
        bail!("This page has too much text to select safely.");
    }
    let width = page.width().value;
    let height = page.height().value;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        bail!("This page has invalid text coordinates.");
    }
    let config = PdfRenderConfig::new().set_target_size(10_000, 10_000);
    let convert = |x, y| page.points_to_pixels(PdfPoints::new(x), PdfPoints::new(y), &config);
    let origin = convert(0.0, 0.0)?;
    let x_axis = convert(width, 0.0)?;
    let y_axis = convert(0.0, height)?;
    let point = |x: f32, y: f32| {
        (
            (origin.0 as f32
                + (x_axis.0 - origin.0) as f32 * x / width
                + (y_axis.0 - origin.0) as f32 * y / height)
                / 10_000.0,
            (origin.1 as f32
                + (x_axis.1 - origin.1) as f32 * x / width
                + (y_axis.1 - origin.1) as f32 * y / height)
                / 10_000.0,
        )
    };
    let mut glyphs = Vec::with_capacity(chars.len());
    for index in 0..chars.len() {
        if index % 256 == 0 && !current() {
            bail!("Text request superseded");
        }
        let character = chars.get(index)?;
        let value = character.unicode_char().unwrap_or('\u{fffd}');
        let bounds = character
            .loose_bounds()
            .or_else(|_| character.tight_bounds())
            .ok()
            .and_then(|bounds| {
                let corners = [
                    point(bounds.left().value, bounds.top().value),
                    point(bounds.right().value, bounds.top().value),
                    point(bounds.left().value, bounds.bottom().value),
                    point(bounds.right().value, bounds.bottom().value),
                ];
                if corners
                    .iter()
                    .any(|(x, y)| !x.is_finite() || !y.is_finite())
                {
                    return None;
                }
                let rect = TextRect {
                    left: corners
                        .iter()
                        .map(|p| p.0)
                        .fold(f32::INFINITY, f32::min)
                        .clamp(0.0, 1.0),
                    top: corners
                        .iter()
                        .map(|p| p.1)
                        .fold(f32::INFINITY, f32::min)
                        .clamp(0.0, 1.0),
                    right: corners
                        .iter()
                        .map(|p| p.0)
                        .fold(f32::NEG_INFINITY, f32::max)
                        .clamp(0.0, 1.0),
                    bottom: corners
                        .iter()
                        .map(|p| p.1)
                        .fold(f32::NEG_INFINITY, f32::max)
                        .clamp(0.0, 1.0),
                };
                (rect.right > rect.left && rect.bottom > rect.top && !value.is_control())
                    .then_some(rect)
            });
        let angle = character.angle_radians().unwrap_or(0.0);
        let origin = point(0.0, 0.0);
        let advance = point(angle.cos(), angle.sin());
        let direction = [advance.0 - origin.0, advance.1 - origin.1];
        glyphs.push(Glyph {
            value,
            bounds,
            direction,
        });
    }
    Ok(PageText { glyphs })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caret_follows_rotated_character_direction() {
        for direction in [[0.0, 1.0], [0.0, -1.0], [-1.0, 0.0]] {
            let text = PageText {
                glyphs: vec![Glyph {
                    value: 'a',
                    direction,
                    bounds: Some(TextRect {
                        left: 0.1,
                        top: 0.1,
                        right: 0.2,
                        bottom: 0.2,
                    }),
                }],
            };
            assert_eq!(
                text.caret(0.15 - direction[0] * 0.04, 0.15 - direction[1] * 0.04, true),
                Some(0)
            );
            assert_eq!(
                text.caret(0.15 + direction[0] * 0.04, 0.15 + direction[1] * 0.04, true),
                Some(1)
            );
        }
    }

    #[test]
    fn selection_preserves_unicode_and_line_breaks() {
        let text = PageText {
            glyphs: "Café\r\n你好"
                .chars()
                .map(|value| Glyph {
                    value,
                    bounds: None,
                    direction: [1.0, 0.0],
                })
                .collect(),
        };
        assert_eq!(text.content(0..text.glyphs.len()), "Café\n你好");
        assert_eq!(text.content(6..8), "你好");
    }

    #[test]
    fn caret_respects_character_edges_and_distant_whitespace() {
        let text = PageText {
            glyphs: vec![Glyph {
                value: 'a',
                direction: [1.0, 0.0],
                bounds: Some(TextRect {
                    left: 0.1,
                    top: 0.1,
                    right: 0.2,
                    bottom: 0.2,
                }),
            }],
        };
        assert_eq!(text.caret(0.11, 0.15, true), Some(0));
        assert_eq!(text.caret(0.19, 0.15, true), Some(1));
        assert_eq!(text.caret(0.9, 0.9, true), None);
        assert_eq!(text.caret(0.9, 0.9, false), Some(1));
    }
}
