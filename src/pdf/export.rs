use super::markup::{self, Edit};
use anyhow::{Context, Result, bail};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};

pub(crate) const MAX_IMAGE_SIDE: i32 = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Format {
    Pdf,
    Flattened,
    Png,
    Jpeg,
    Text,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Pdf | Self::Flattened => "pdf",
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Text => "txt",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Export {
    pub source: PathBuf,
    pub format: Format,
    pub page: Option<usize>,
    pub dpi: u32,
    pub edits: Vec<Edit>,
    pub marks: bool,
    pub to: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FileTask {
    Export(Export),
    Combine { files: Vec<PathBuf>, to: PathBuf },
}

pub(crate) struct FileJob {
    pub id: u64,
    pub task: FileTask,
}

pub(super) struct Active<'a> {
    pub id: u64,
    export: Export,
    document: PdfDocument<'a>,
    pages: Vec<usize>,
    next: usize,
    written: Vec<PathBuf>,
    text: String,
}

fn vacant(path: &Path) -> Result<()> {
    if path.exists() {
        bail!("{} already exists.", path.display());
    }
    Ok(())
}

fn numbered(to: &Path, number: usize) -> PathBuf {
    let stem = to.file_stem().unwrap_or_default().to_string_lossy();
    let extension = to.extension().unwrap_or_default().to_string_lossy();
    to.with_file_name(format!("{stem} {number}.{extension}"))
}

fn is_mark(kind: PdfPageAnnotationType) -> bool {
    !matches!(
        kind,
        PdfPageAnnotationType::Link
            | PdfPageAnnotationType::Widget
            | PdfPageAnnotationType::XfaWidget
            | PdfPageAnnotationType::Popup
    )
}

fn strip_marks(document: &PdfDocument<'_>) -> Result<()> {
    for mut page in document.pages().iter() {
        let annotations = page.annotations_mut();
        while let Some(index) = annotations
            .iter()
            .position(|annotation| is_mark(annotation.annotation_type()))
        {
            let annotation = annotations.get(index as PdfPageAnnotationIndex)?;
            annotations.delete_annotation(annotation)?;
        }
    }
    Ok(())
}

pub(super) fn combine(engine: &Pdfium, files: &[PathBuf], to: &Path) -> Result<Vec<PathBuf>> {
    if files.len() < 2 {
        bail!("Choose at least two PDFs to combine.");
    }
    vacant(to)?;
    let mut combined = engine
        .create_new_pdf()
        .context("A new PDF could not be created.")?;
    for file in files {
        let part = engine
            .load_pdf_from_file(file, None)
            .with_context(|| format!("{} could not be opened.", file.display()))?;
        combined
            .pages_mut()
            .append(&part)
            .with_context(|| format!("{} could not be added.", file.display()))?;
    }
    combined
        .save_to_file(to)
        .with_context(|| format!("The combined PDF could not be written to {}.", to.display()))?;
    Ok(vec![to.to_path_buf()])
}

impl<'a> Active<'a> {
    pub(super) fn start(engine: &'a Pdfium, id: u64, export: Export) -> Result<Self> {
        let mut document = engine
            .load_pdf_from_file(&export.source, None)
            .context("The PDF could not be opened for export.")?;
        let count = document.pages().len() as usize;
        let pages: Vec<usize> = match export.page {
            Some(page) if page < count => vec![page],
            Some(page) => bail!("Page {} is no longer in this PDF.", page + 1),
            None => (0..count).collect(),
        };
        if pages.is_empty() {
            bail!("This PDF has no pages to export.");
        }
        if export.marks {
            markup::apply(&mut document, &export.edits)?;
        } else if matches!(export.format, Format::Pdf | Format::Flattened) {
            strip_marks(&document)?;
        }
        if pages.len() == 1 || !matches!(export.format, Format::Png | Format::Jpeg) {
            vacant(&export.to)?;
        }
        Ok(Self {
            id,
            export,
            document,
            pages,
            next: 0,
            written: Vec::new(),
            text: String::new(),
        })
    }

    fn document_copy(&mut self) -> Result<()> {
        let flatten = self.export.format == Format::Flattened;
        if self.export.page.is_none() {
            if flatten {
                for mut page in self.document.pages().iter() {
                    page.flatten().context("A page could not be flattened.")?;
                }
            }
            self.document.save_to_file(&self.export.to)?;
            return Ok(());
        }
        let keep = self.pages[0];
        for index in (0..self.document.pages().len() as usize).rev() {
            if index != keep {
                self.document.pages().get(index as i32)?.delete()?;
            }
        }
        if flatten {
            self.document
                .pages()
                .get(0)?
                .flatten()
                .context("The page could not be flattened.")?;
        }
        self.document.save_to_file(&self.export.to)?;
        Ok(())
    }

    fn image(&mut self, index: usize) -> Result<PathBuf> {
        let page = self.document.pages().get(index as i32)?;
        let width = (page.width().value * self.export.dpi as f32 / 72.0).round() as i32;
        let bitmap = page.render_with_config(
            &PdfRenderConfig::new()
                .set_target_width(width.clamp(1, MAX_IMAGE_SIDE))
                .set_maximum_width(MAX_IMAGE_SIDE)
                .set_maximum_height(MAX_IMAGE_SIDE)
                .render_annotations(self.export.marks),
        )?;
        let (width, height) = (bitmap.width() as u32, bitmap.height() as u32);
        let to = if self.pages.len() == 1 {
            self.export.to.clone()
        } else {
            numbered(&self.export.to, index + 1)
        };
        vacant(&to)?;
        let rgba = bitmap.as_rgba_bytes();
        let saved = if self.export.format == Format::Jpeg {
            let rgb: Vec<u8> = rgba
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
                .collect();
            image::save_buffer_with_format(
                &to,
                &rgb,
                width,
                height,
                image::ColorType::Rgb8,
                image::ImageFormat::Jpeg,
            )
        } else {
            image::save_buffer_with_format(
                &to,
                &rgba,
                width,
                height,
                image::ColorType::Rgba8,
                image::ImageFormat::Png,
            )
        };
        saved.with_context(|| format!("The image could not be written to {}.", to.display()))?;
        Ok(to)
    }

    pub(super) fn step(&mut self) -> Result<Option<Vec<PathBuf>>> {
        match self.export.format {
            Format::Pdf | Format::Flattened => {
                self.document_copy().with_context(|| {
                    format!(
                        "The PDF could not be written to {}.",
                        self.export.to.display()
                    )
                })?;
                return Ok(Some(vec![self.export.to.clone()]));
            }
            Format::Png | Format::Jpeg => {
                let written = self.image(self.pages[self.next])?;
                self.written.push(written);
            }
            Format::Text => {
                let page = self.document.pages().get(self.pages[self.next] as i32)?;
                if self.next > 0 {
                    self.text.push_str("\n\n");
                }
                self.text
                    .push_str(page.text()?.all().replace(['\r', '\0'], "").trim_end());
            }
        }
        self.next += 1;
        if self.next < self.pages.len() {
            return Ok(None);
        }
        if self.export.format == Format::Text {
            self.text.push('\n');
            std::fs::write(&self.export.to, &self.text).with_context(|| {
                format!(
                    "The text could not be written to {}.",
                    self.export.to.display()
                )
            })?;
            self.written.push(self.export.to.clone());
        }
        Ok(Some(std::mem::take(&mut self.written)))
    }

    pub(super) fn abandon(self) {
        for path in self.written {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_image_names_keep_the_stem_and_extension() {
        assert_eq!(
            numbered(Path::new("/out/Report export.png"), 3),
            PathBuf::from("/out/Report export 3.png")
        );
        assert_eq!(Format::Jpeg.extension(), "jpg");
        assert_eq!(Format::Flattened.extension(), "pdf");
    }
}
