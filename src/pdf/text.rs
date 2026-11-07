use anyhow::{Result, bail};
use pdfium_render::prelude::*;
use std::ops::Range;

const MAX_PAGE_CHARACTERS: usize = 100_000;
const MAX_PAGE_LINKS: usize = 2_000;
const GEOMETRY_PIXELS: i32 = 10_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TextRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl TextRect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        (self.left..=self.right).contains(&x) && (self.top..=self.bottom).contains(&y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Geometry {
    origin: [f32; 2],
    x_axis: [f32; 2],
    y_axis: [f32; 2],
    width: f32,
    height: f32,
}

impl Geometry {
    #[cfg(test)]
    pub const UNIT: Self = Self {
        origin: [0.0, 1.0],
        x_axis: [1.0, 0.0],
        y_axis: [0.0, -1.0],
        width: 1.0,
        height: 1.0,
    };

    fn of(page: &PdfPage<'_>) -> Result<Self> {
        let width = page.width().value;
        let height = page.height().value;
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            bail!("This page has invalid text coordinates.");
        }
        let config = PdfRenderConfig::new().set_target_size(GEOMETRY_PIXELS, GEOMETRY_PIXELS);
        let scale = GEOMETRY_PIXELS as f32;
        let convert = |x, y| -> Result<[f32; 2]> {
            let (px, py) = page.points_to_pixels(PdfPoints::new(x), PdfPoints::new(y), &config)?;
            Ok([px as f32 / scale, py as f32 / scale])
        };
        let origin = convert(0.0, 0.0)?;
        let x_end = convert(width, 0.0)?;
        let y_end = convert(0.0, height)?;
        Ok(Self {
            origin,
            x_axis: [x_end[0] - origin[0], x_end[1] - origin[1]],
            y_axis: [y_end[0] - origin[0], y_end[1] - origin[1]],
            width,
            height,
        })
    }

    pub fn point(&self, x: f32, y: f32) -> (f32, f32) {
        let (u, v) = (x / self.width, y / self.height);
        (
            self.origin[0] + self.x_axis[0] * u + self.y_axis[0] * v,
            self.origin[1] + self.x_axis[1] * u + self.y_axis[1] * v,
        )
    }

    fn rect(&self, left: f32, top: f32, right: f32, bottom: f32) -> Option<TextRect> {
        let corners = [
            self.point(left, top),
            self.point(right, top),
            self.point(left, bottom),
            self.point(right, bottom),
        ];
        if corners
            .iter()
            .any(|(x, y)| !x.is_finite() || !y.is_finite())
        {
            return None;
        }
        let xs = corners.map(|corner| corner.0);
        let ys = corners.map(|corner| corner.1);
        let rect = TextRect {
            left: xs.into_iter().fold(f32::INFINITY, f32::min).clamp(0.0, 1.0),
            top: ys.into_iter().fold(f32::INFINITY, f32::min).clamp(0.0, 1.0),
            right: xs
                .into_iter()
                .fold(f32::NEG_INFINITY, f32::max)
                .clamp(0.0, 1.0),
            bottom: ys
                .into_iter()
                .fold(f32::NEG_INFINITY, f32::max)
                .clamp(0.0, 1.0),
        };
        (rect.right > rect.left && rect.bottom > rect.top).then_some(rect)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum LinkTarget {
    Page {
        index: usize,
        anchor: Option<(f32, f32)>,
    },
    Uri(String),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Link {
    pub bounds: TextRect,
    pub target: LinkTarget,
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
    pub links: Vec<Link>,
    pub geometry: Geometry,
}

impl PageText {
    pub fn byte_len(&self) -> usize {
        self.glyphs.capacity() * std::mem::size_of::<Glyph>()
            + self.links.capacity() * std::mem::size_of::<Link>()
            + self
                .links
                .iter()
                .map(|link| match &link.target {
                    LinkTarget::Uri(uri) => uri.capacity(),
                    LinkTarget::Page { .. } => 0,
                })
                .sum::<usize>()
    }

    pub fn link_at(&self, x: f32, y: f32) -> Option<&Link> {
        self.links
            .iter()
            .rev()
            .find(|link| link.bounds.contains(x, y))
    }

    pub fn anchor_top(&self, anchor: (f32, f32)) -> Option<f32> {
        let (_, top) = self.geometry.point(anchor.0, anchor.1);
        top.is_finite().then(|| top.clamp(0.0, 1.0))
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

pub(super) fn characters(page: &PdfPage<'_>) -> Option<Vec<char>> {
    let text = page.text().ok()?;
    let chars = text.chars();
    if chars.len() > MAX_PAGE_CHARACTERS {
        return None;
    }
    Some(
        chars
            .iter()
            .map(|character| character.unicode_char().unwrap_or('\u{fffd}'))
            .collect(),
    )
}

pub(super) fn extract(page: &PdfPage<'_>, current: impl Fn() -> bool) -> Result<PageText> {
    let geometry = Geometry::of(page)?;
    let links = links(page, &geometry);
    let text = page.text()?;
    let chars = text.chars();
    if chars.len() > MAX_PAGE_CHARACTERS {
        bail!("This page has too much text to select safely.");
    }
    let origin = geometry.point(0.0, 0.0);
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
                geometry.rect(
                    bounds.left().value,
                    bounds.top().value,
                    bounds.right().value,
                    bounds.bottom().value,
                )
            })
            .filter(|_| !value.is_control());
        let angle = character.angle_radians().unwrap_or(0.0);
        let advance = geometry.point(angle.cos(), angle.sin());
        glyphs.push(Glyph {
            value,
            bounds,
            direction: [advance.0 - origin.0, advance.1 - origin.1],
        });
    }
    Ok(PageText {
        glyphs,
        links,
        geometry,
    })
}

fn links(page: &PdfPage<'_>, geometry: &Geometry) -> Vec<Link> {
    page.links()
        .iter()
        .filter_map(|link| {
            let target = link_target(&link)?;
            let rect = link.rect().ok()?;
            let bounds = geometry.rect(
                rect.left().value,
                rect.top().value,
                rect.right().value,
                rect.bottom().value,
            )?;
            Some(Link { bounds, target })
        })
        .take(MAX_PAGE_LINKS)
        .collect()
}

fn link_target(link: &PdfLink<'_>) -> Option<LinkTarget> {
    match link.action() {
        Some(PdfAction::Uri(action)) => action.uri().ok().map(LinkTarget::Uri),
        Some(PdfAction::LocalDestination(action)) => page_target(&action.destination().ok()?),
        Some(_) => None,
        None => page_target(&link.destination()?),
    }
}

fn page_target(destination: &PdfDestination<'_>) -> Option<LinkTarget> {
    let index = usize::try_from(destination.page_index().ok()?).ok()?;
    let anchor = destination
        .view_settings()
        .ok()
        .and_then(|settings| match settings {
            PdfDestinationViewSettings::SpecificCoordinatesAndZoom(left, top, _) => {
                top.map(|top| (left.map_or(0.0, |left| left.value), top.value))
            }
            PdfDestinationViewSettings::FitPageHorizontallyToWindow(top)
            | PdfDestinationViewSettings::FitBoundsHorizontallyToWindow(top) => {
                top.map(|top| (0.0, top.value))
            }
            PdfDestinationViewSettings::FitPageToRectangle(rect) => {
                Some((rect.left().value, rect.top().value))
            }
            _ => None,
        });
    Some(LinkTarget::Page { index, anchor })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caret_follows_rotated_character_direction() {
        for direction in [[0.0, 1.0], [0.0, -1.0], [-1.0, 0.0]] {
            let text = PageText {
                links: Vec::new(),
                geometry: Geometry::UNIT,
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
            links: Vec::new(),
            geometry: Geometry::UNIT,
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
            links: Vec::new(),
            geometry: Geometry::UNIT,
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

    fn link_covers(text: &PageText, phrase: &str) -> Option<LinkTarget> {
        let content: String = text.glyphs.iter().map(|glyph| glyph.value).collect();
        let start = content.find(phrase)?;
        let start = content[..start].chars().count();
        let mut targets = text.glyphs[start..start + phrase.chars().count()]
            .iter()
            .filter(|glyph| !glyph.value.is_whitespace())
            .map(|glyph| {
                let bounds = glyph.bounds.expect("glyph bounds");
                text.link_at(
                    (bounds.left + bounds.right) / 2.0,
                    (bounds.top + bounds.bottom) / 2.0,
                )
                .map(|link| link.target.clone())
            });
        let first = targets.next()??;
        targets
            .all(|target| target.as_ref() == Some(&first))
            .then_some(first)
    }

    #[test]
    #[ignore = "requires PDFium installed by scripts/setup_pdfium.py"]
    fn links_cover_their_text_on_upright_and_rotated_pages() {
        let engine = crate::pdf::bind().unwrap();
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/links.pdf");
        let document = engine.load_pdf_from_file(&path, None).unwrap();
        let page = |index| extract(&document.pages().get(index).unwrap(), || true).unwrap();

        let first = page(0);
        assert_eq!(first.links.len(), 2);
        assert_eq!(
            link_covers(&first, "Go to the last page"),
            Some(LinkTarget::Page {
                index: 2,
                anchor: Some((0.0, 340.0)),
            })
        );
        assert_eq!(
            link_covers(&first, "Visit example.com"),
            Some(LinkTarget::Uri("https://example.com/".into()))
        );
        assert!(link_covers(&first, "Internal and web links").is_none());

        let rotated = page(1);
        assert_eq!(rotated.links.len(), 1);
        let bounds = rotated.links[0].bounds;
        assert!(bounds.bottom - bounds.top > bounds.right - bounds.left);
        assert_eq!(
            link_covers(&rotated, "Back to the first page"),
            Some(LinkTarget::Page {
                index: 0,
                anchor: None,
            })
        );

        let last = page(2);
        let top = last.anchor_top((0.0, 340.0)).unwrap();
        assert!((top - (792.0 - 340.0) / 792.0).abs() < 0.01);
        let content: String = last.glyphs.iter().map(|glyph| glyph.value).collect();
        let arrival = content[..content.find("You arrived").unwrap()]
            .chars()
            .count();
        let arrival = last.glyphs[arrival].bounds.unwrap();
        assert!(arrival.top > top - 0.03 && arrival.top < top + 0.03);
    }
}
