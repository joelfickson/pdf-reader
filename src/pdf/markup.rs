use super::text::{Geometry, Glyph, TextRect};
use anyhow::{Context, Result, bail};
use pdfium_render::prelude::*;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

pub(crate) const STROKE_WEIGHT: f32 = 0.009;
const MIN_STROKE_POINTS: f32 = 0.4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MarkupKind {
    Highlight,
    Underline,
    Strikeout,
}

impl MarkupKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Highlight => "Highlight",
            Self::Underline => "Underline",
            Self::Strikeout => "Strikethrough",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PageMarks {
    pub page: usize,
    pub rects: Vec<TextRect>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Markup {
    pub kind: MarkupKind,
    pub color: u32,
    pub quote: String,
    pub pages: Vec<PageMarks>,
}

impl Markup {
    pub fn first_page(&self) -> usize {
        self.pages.first().map_or(0, |marks| marks.page)
    }

    pub fn on_page(&self, page: usize) -> impl Iterator<Item = &TextRect> {
        self.pages
            .iter()
            .filter(move |marks| marks.page == page)
            .flat_map(|marks| marks.rects.iter())
    }
}

pub(crate) type Strokes = Arc<Vec<Vec<[f32; 2]>>>;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Signature {
    pub page: usize,
    pub bounds: TextRect,
    pub color: u32,
    pub strokes: Strokes,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Edit {
    Markup(Markup),
    Signature(Signature),
}

impl Edit {
    pub fn first_page(&self) -> usize {
        match self {
            Self::Markup(markup) => markup.first_page(),
            Self::Signature(signature) => signature.page,
        }
    }

    pub fn color(&self) -> u32 {
        match self {
            Self::Markup(markup) => markup.color,
            Self::Signature(signature) => signature.color,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Markup(markup) => markup.kind.label(),
            Self::Signature(_) => "Signature",
        }
    }

    pub fn summary(&self) -> &str {
        match self {
            Self::Markup(markup) => &markup.quote,
            Self::Signature(_) => "Drawn signature, image only",
        }
    }
}

pub(crate) fn lines(glyphs: &[Glyph]) -> Vec<TextRect> {
    let mut lines: Vec<(TextRect, bool)> = Vec::new();
    for glyph in glyphs {
        let Some(bounds) = glyph.bounds else {
            continue;
        };
        let horizontal = glyph.direction[0].abs() >= glyph.direction[1].abs();
        match lines.last_mut() {
            Some((line, flow)) if *flow == horizontal && same_line(line, &bounds, horizontal) => {
                line.left = line.left.min(bounds.left);
                line.top = line.top.min(bounds.top);
                line.right = line.right.max(bounds.right);
                line.bottom = line.bottom.max(bounds.bottom);
            }
            _ => lines.push((bounds, horizontal)),
        }
    }
    lines.into_iter().map(|(line, _)| line).collect()
}

fn same_line(line: &TextRect, glyph: &TextRect, horizontal: bool) -> bool {
    let (line_span, glyph_span) = if horizontal {
        ((line.top, line.bottom), (glyph.top, glyph.bottom))
    } else {
        ((line.left, line.right), (glyph.left, glyph.right))
    };
    let overlap = line_span.1.min(glyph_span.1) - line_span.0.max(glyph_span.0);
    let smaller = (line_span.1 - line_span.0).min(glyph_span.1 - glyph_span.0);
    smaller > 0.0 && overlap >= smaller * 0.5
}

struct Quad {
    left: f32,
    bottom: f32,
    right: f32,
    top: f32,
}

fn quad(geometry: &Geometry, rect: &TextRect) -> Option<Quad> {
    let first = geometry.page_point(rect.left, rect.top)?;
    let second = geometry.page_point(rect.right, rect.bottom)?;
    let quad = Quad {
        left: first.0.min(second.0),
        bottom: first.1.min(second.1),
        right: first.0.max(second.0),
        top: first.1.max(second.1),
    };
    (quad.right > quad.left && quad.top > quad.bottom).then_some(quad)
}

fn describe(
    annotation: &mut impl PdfPageAnnotationCommon,
    quads: &[Quad],
    color: u32,
) -> Result<()> {
    let bounds = quads.iter().fold(
        (
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ),
        |bounds, quad| {
            (
                bounds.0.min(quad.left),
                bounds.1.min(quad.bottom),
                bounds.2.max(quad.right),
                bounds.3.max(quad.top),
            )
        },
    );
    annotation.set_bounds(PdfRect::new_from_values(
        bounds.1, bounds.0, bounds.3, bounds.2,
    ))?;
    annotation.set_stroke_color(PdfColor::new(
        (color >> 16) as u8,
        (color >> 8) as u8,
        color as u8,
        255,
    ))?;
    annotation.set_is_printed(true)?;
    Ok(())
}

fn attach(points: &mut PdfPageAnnotationAttachmentPoints<'_>, quads: &[Quad]) -> Result<()> {
    for quad in quads {
        points.create_attachment_point_at_end(PdfQuadPoints::new_from_values(
            quad.left,
            quad.top,
            quad.right,
            quad.top,
            quad.left,
            quad.bottom,
            quad.right,
            quad.bottom,
        ))?;
    }
    Ok(())
}

fn sign(document: &PdfDocument<'_>, signature: &Signature) -> Result<()> {
    let mut page = document
        .pages()
        .get(signature.page as i32)
        .with_context(|| format!("Page {} is no longer in this PDF.", signature.page + 1))?;
    let geometry = Geometry::of(&page)?;
    let bounds = signature.bounds;
    let (width, height) = (bounds.right - bounds.left, bounds.bottom - bounds.top);
    let frame = quad(&geometry, &bounds).with_context(|| {
        format!(
            "The signature on page {} has no usable position.",
            signature.page + 1
        )
    })?;
    let weight = (page.width().value * width * STROKE_WEIGHT).max(MIN_STROKE_POINTS);
    let color = PdfColor::new(
        (signature.color >> 16) as u8,
        (signature.color >> 8) as u8,
        signature.color as u8,
        255,
    );
    let place = |point: &[f32; 2]| {
        geometry.page_point(
            bounds.left + point[0] * width,
            bounds.top + point[1] * height,
        )
    };
    let mut annotation = page.annotations_mut().create_stamp_annotation()?;
    annotation.set_bounds(PdfRect::new_from_values(
        frame.bottom - weight,
        frame.left - weight,
        frame.top + weight,
        frame.right + weight,
    ))?;
    annotation.set_is_printed(true)?;
    let mut drawn = 0;
    for stroke in signature.strokes.iter() {
        let mut points = stroke.iter().filter_map(place);
        let Some(start) = points.next() else {
            continue;
        };
        let mut path = PdfPagePathObject::new(
            document,
            PdfPoints::new(start.0),
            PdfPoints::new(start.1),
            Some(color),
            Some(PdfPoints::new(weight)),
            None,
        )?;
        let mut segments = 0;
        for point in points {
            path.line_to(PdfPoints::new(point.0), PdfPoints::new(point.1))?;
            segments += 1;
        }
        if segments == 0 {
            path.line_to(
                PdfPoints::new(start.0 + weight / 4.0),
                PdfPoints::new(start.1),
            )?;
        }
        path.set_line_cap(PdfPageObjectLineCap::Round)?;
        path.set_line_join(PdfPageObjectLineJoin::Round)?;
        annotation.objects_mut().add_path_object(path)?;
        drawn += 1;
    }
    if drawn == 0 {
        bail!(
            "The signature on page {} has no strokes.",
            signature.page + 1
        );
    }
    Ok(())
}

pub(super) fn apply(document: &PdfDocument<'_>, edits: &[Edit]) -> Result<()> {
    for edit in edits {
        let markup = match edit {
            Edit::Markup(markup) => markup,
            Edit::Signature(signature) => {
                sign(document, signature)?;
                continue;
            }
        };
        for marks in &markup.pages {
            let mut page = document
                .pages()
                .get(marks.page as i32)
                .with_context(|| format!("Page {} is no longer in this PDF.", marks.page + 1))?;
            let geometry = Geometry::of(&page)?;
            let quads: Vec<Quad> = marks
                .rects
                .iter()
                .filter_map(|rect| quad(&geometry, rect))
                .collect();
            if quads.is_empty() {
                bail!(
                    "The marked text on page {} has no usable position.",
                    marks.page + 1
                );
            }
            let annotations = page.annotations_mut();
            match markup.kind {
                MarkupKind::Highlight => {
                    let mut annotation = annotations.create_highlight_annotation()?;
                    describe(&mut annotation, &quads, markup.color)?;
                    attach(annotation.attachment_points_mut(), &quads)?;
                }
                MarkupKind::Underline => {
                    let mut annotation = annotations.create_underline_annotation()?;
                    describe(&mut annotation, &quads, markup.color)?;
                    attach(annotation.attachment_points_mut(), &quads)?;
                }
                MarkupKind::Strikeout => {
                    let mut annotation = annotations.create_strikeout_annotation()?;
                    describe(&mut annotation, &quads, markup.color)?;
                    attach(annotation.attachment_points_mut(), &quads)?;
                }
            }
        }
    }
    Ok(())
}

fn sibling(path: &Path) -> Result<PathBuf> {
    let name = path
        .file_name()
        .context("This PDF has no file name to save to.")?
        .to_string_lossy();
    Ok(path.with_file_name(format!(".{name}.{}.saving", std::process::id())))
}

pub(super) struct Staged {
    temporary: PathBuf,
    permissions: fs::Permissions,
}

pub(super) fn stage(path: &Path) -> Result<Staged> {
    let metadata = fs::metadata(path).context("This PDF is no longer where it was opened from.")?;
    if metadata.permissions().readonly() {
        bail!("This PDF is read-only, so changes cannot be saved to it.");
    }
    Ok(Staged {
        temporary: sibling(path)?,
        permissions: metadata.permissions(),
    })
}

impl Staged {
    pub(super) fn temporary(&self) -> &Path {
        &self.temporary
    }

    pub(super) fn discard(self) {
        let _ = fs::remove_file(&self.temporary);
    }

    pub(super) fn commit(self, path: &Path) -> Result<()> {
        let replaced = fs::set_permissions(&self.temporary, self.permissions.clone())
            .context("The saved copy could not take the PDF's permissions.")
            .and_then(|()| {
                fs::rename(&self.temporary, path)
                    .context("The saved copy could not replace the PDF.")
            });
        if replaced.is_err() {
            self.discard();
        }
        replaced
    }
}

pub(super) fn save(engine: &Pdfium, path: &Path, edits: &[Edit]) -> Result<()> {
    let staged = stage(path)?;
    let written = (|| -> Result<()> {
        let document = engine
            .load_pdf_from_file(path, None)
            .context("The PDF could not be reopened for saving.")?;
        if !document.signatures().is_empty() {
            bail!(
                "This PDF is digitally signed. Saving marks would invalidate the signature, so it is left unchanged."
            );
        }
        apply(&document, edits)?;
        document
            .save_to_file(staged.temporary())
            .context("The changes could not be written next to the PDF.")?;
        Ok(())
    })();
    match written {
        Ok(()) => staged.commit(path),
        Err(error) => {
            staged.discard();
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyph(left: f32, top: f32, right: f32, bottom: f32, direction: [f32; 2]) -> Glyph {
        Glyph {
            value: 'a',
            bounds: Some(TextRect {
                left,
                top,
                right,
                bottom,
            }),
            direction,
        }
    }

    #[test]
    fn glyphs_merge_into_one_rect_per_line() {
        let across = [1.0, 0.0];
        let mut glyphs = vec![
            glyph(0.10, 0.10, 0.12, 0.12, across),
            glyph(0.12, 0.101, 0.14, 0.121, across),
            glyph(0.14, 0.10, 0.16, 0.12, across),
            glyph(0.10, 0.13, 0.12, 0.15, across),
            glyph(0.12, 0.13, 0.15, 0.15, across),
        ];
        glyphs.insert(
            2,
            Glyph {
                value: '\n',
                bounds: None,
                direction: across,
            },
        );
        let merged = lines(&glyphs);
        assert_eq!(merged.len(), 2);
        assert_eq!((merged[0].left, merged[0].right), (0.10, 0.16));
        assert_eq!((merged[0].top, merged[0].bottom), (0.10, 0.121));
        assert_eq!((merged[1].left, merged[1].right), (0.10, 0.15));
        assert!(lines(&[]).is_empty());
    }

    #[test]
    fn vertical_text_merges_along_its_own_direction() {
        let down = [0.0, 1.0];
        let merged = lines(&[
            glyph(0.50, 0.10, 0.52, 0.12, down),
            glyph(0.50, 0.12, 0.52, 0.14, down),
            glyph(0.46, 0.10, 0.48, 0.12, down),
        ]);
        assert_eq!(merged.len(), 2);
        assert_eq!((merged[0].top, merged[0].bottom), (0.10, 0.14));
    }

    fn tint(raster: &crate::pdf::Raster, rect: &TextRect) -> f32 {
        let left = (rect.left * raster.width as f32).ceil() as u32;
        let right = ((rect.right * raster.width as f32).floor() as u32).min(raster.width);
        let top = (rect.top * raster.height as f32).ceil() as u32;
        let bottom = ((rect.bottom * raster.height as f32).floor() as u32).min(raster.height);
        let mut tinted = 0;
        let mut total = 0;
        for y in top..bottom {
            for x in left..right {
                let offset = ((y * raster.width + x) * 4) as usize;
                let pixel = &raster.bytes[offset..offset + 3];
                let spread = pixel.iter().max().unwrap() - pixel.iter().min().unwrap();
                tinted += u32::from(spread > 60);
                total += 1;
            }
        }
        assert!(total > 0, "the marked rect covers no pixels");
        tinted as f32 / total as f32
    }

    #[test]
    #[ignore = "requires PDFium installed by scripts/setup_pdfium.py"]
    fn saved_marks_cover_the_marked_text_and_replace_the_file() {
        let engine = crate::pdf::bind().unwrap();
        let directory =
            std::env::temp_dir().join(format!("agenticpdf-markup-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("marked.pdf");
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/selection.pdf"),
            &path,
        )
        .unwrap();
        let mut markups = Vec::new();
        let mut baselines = Vec::new();
        {
            let document = engine.load_pdf_from_file(&path, None).unwrap();
            for (index, kind, color) in [
                (0, MarkupKind::Highlight, 0xf2c230),
                (1, MarkupKind::Highlight, 0x4fc26b),
                (0, MarkupKind::Underline, 0x5aaef5),
            ] {
                let page = document.pages().get(index).unwrap();
                let text = crate::pdf::text::extract(&page, || true).unwrap();
                let end = text.glyphs.len().min(12);
                let rects = lines(&text.glyphs[..end]);
                assert!(!rects.is_empty());
                let raster = crate::pdf::render(&page, 800.0, 1.0, 4096).unwrap();
                baselines.push(
                    rects
                        .iter()
                        .map(|rect| tint(&raster, rect))
                        .collect::<Vec<_>>(),
                );
                markups.push(Edit::Markup(Markup {
                    kind,
                    color,
                    quote: text.content(0..end),
                    pages: vec![PageMarks {
                        page: index as usize,
                        rects,
                    }],
                }));
            }
        }
        save(&engine, &path, &markups).unwrap();
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        {
            let document = engine.load_pdf_from_file(&path, None).unwrap();
            for (index, expected) in [
                (
                    0,
                    vec![
                        PdfPageAnnotationType::Highlight,
                        PdfPageAnnotationType::Underline,
                    ],
                ),
                (1, vec![PdfPageAnnotationType::Highlight]),
            ] {
                let page = document.pages().get(index).unwrap();
                let kinds: Vec<_> = page
                    .annotations()
                    .iter()
                    .map(|annotation| annotation.annotation_type())
                    .collect();
                assert_eq!(kinds, expected);
                let raster = crate::pdf::render(&page, 800.0, 1.0, 4096).unwrap();
                let Edit::Markup(highlight) = &markups[index as usize] else {
                    panic!("expected a text mark");
                };
                for (rect, baseline) in highlight
                    .on_page(index as usize)
                    .zip(&baselines[index as usize])
                {
                    let covered = tint(&raster, rect);
                    assert!(
                        covered - baseline > 0.5,
                        "highlight must cover the marked text on page {index}: {baseline} -> {covered}"
                    );
                }
                let clear = TextRect {
                    left: 0.02,
                    top: 0.9,
                    right: 0.2,
                    bottom: 0.98,
                };
                assert!(tint(&raster, &clear) < 0.05);
            }
        }
        let before = fs::read(&path).unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions).unwrap();
        assert!(save(&engine, &path, &markups).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    #[ignore = "requires PDFium installed by scripts/setup_pdfium.py"]
    fn saved_signatures_draw_their_strokes_inside_their_bounds() {
        let engine = crate::pdf::bind().unwrap();
        let directory =
            std::env::temp_dir().join(format!("agenticpdf-signature-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("signed.pdf");
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/selection.pdf"),
            &path,
        )
        .unwrap();
        let bounds = TextRect {
            left: 0.35,
            top: 0.78,
            right: 0.65,
            bottom: 0.88,
        };
        let around = [
            TextRect {
                left: 0.05,
                top: 0.78,
                right: 0.3,
                bottom: 0.88,
            },
            TextRect {
                left: 0.7,
                top: 0.78,
                right: 0.95,
                bottom: 0.88,
            },
            TextRect {
                left: 0.35,
                top: 0.6,
                right: 0.65,
                bottom: 0.74,
            },
            TextRect {
                left: 0.35,
                top: 0.9,
                right: 0.65,
                bottom: 0.98,
            },
        ];
        let strokes: Strokes = Arc::new(vec![
            vec![[0.0, 0.0], [0.5, 1.0], [1.0, 0.0]],
            vec![[0.0, 1.0], [1.0, 1.0]],
            vec![[0.5, 0.5]],
        ]);
        let edits: Vec<Edit> = (0..2)
            .map(|page| {
                Edit::Signature(Signature {
                    page,
                    bounds,
                    color: 0x1c3d8f,
                    strokes: strokes.clone(),
                })
            })
            .collect();
        let mut before = Vec::new();
        {
            let document = engine.load_pdf_from_file(&path, None).unwrap();
            for index in 0..2 {
                let page = document.pages().get(index).unwrap();
                let raster = crate::pdf::render(&page, 800.0, 1.0, 4096).unwrap();
                assert_eq!(tint(&raster, &bounds), 0.0);
                before.push(around.map(|rect| tint(&raster, &rect)));
            }
        }
        save(&engine, &path, &edits).unwrap();
        let document = engine.load_pdf_from_file(&path, None).unwrap();
        for index in 0..2 {
            let page = document.pages().get(index).unwrap();
            let kinds: Vec<_> = page
                .annotations()
                .iter()
                .map(|annotation| annotation.annotation_type())
                .collect();
            assert_eq!(kinds, [PdfPageAnnotationType::Stamp]);
            let raster = crate::pdf::render(&page, 800.0, 1.0, 4096).unwrap();
            let inked = tint(&raster, &bounds);
            assert!(
                (0.01..0.5).contains(&inked),
                "signature strokes must show inside their bounds on page {index}: {inked}"
            );
            assert_eq!(
                around.map(|rect| tint(&raster, &rect)),
                before[index as usize],
                "nothing may be drawn outside the signature on page {index}"
            );
        }
        drop(document);
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn page_points_invert_the_display_mapping() {
        let geometry = Geometry::UNIT;
        let (x, y) = geometry.page_point(0.25, 0.1).unwrap();
        assert!((x - 0.25).abs() < 1e-6 && (y - 0.9).abs() < 1e-6);
        let display = geometry.point(x, y);
        assert!((display.0 - 0.25).abs() < 1e-6 && (display.1 - 0.1).abs() < 1e-6);
        let quad = quad(
            &geometry,
            &TextRect {
                left: 0.1,
                top: 0.2,
                right: 0.3,
                bottom: 0.25,
            },
        )
        .unwrap();
        assert!((quad.left - 0.1).abs() < 1e-6 && (quad.right - 0.3).abs() < 1e-6);
        assert!((quad.bottom - 0.75).abs() < 1e-6 && (quad.top - 0.8).abs() < 1e-6);
    }
}
