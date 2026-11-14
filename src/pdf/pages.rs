use super::markup::{Staged, stage};
use anyhow::{Context, Result, bail};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PageOp {
    Rotate { pages: Vec<usize>, turns: u8 },
    Delete { pages: Vec<usize> },
    Insert { file: PathBuf },
    Extract { pages: Vec<usize>, to: PathBuf },
    Apply,
    Cancel,
}

fn turned(rotation: PdfPageRenderRotation, turns: u8) -> PdfPageRenderRotation {
    let current = match rotation {
        PdfPageRenderRotation::None => 0,
        PdfPageRenderRotation::Degrees90 => 1,
        PdfPageRenderRotation::Degrees180 => 2,
        PdfPageRenderRotation::Degrees270 => 3,
    };
    match (current + turns) % 4 {
        0 => PdfPageRenderRotation::None,
        1 => PdfPageRenderRotation::Degrees90,
        2 => PdfPageRenderRotation::Degrees180,
        _ => PdfPageRenderRotation::Degrees270,
    }
}

fn checked(document: &PdfDocument<'_>, pages: &[usize]) -> Result<Vec<usize>> {
    let count = document.pages().len() as usize;
    let mut pages = pages.to_vec();
    pages.sort_unstable();
    pages.dedup();
    if pages.is_empty() {
        bail!("No pages were chosen.");
    }
    if let Some(page) = pages.iter().find(|page| **page >= count) {
        bail!("Page {} is no longer in this PDF.", page + 1);
    }
    Ok(pages)
}

pub(super) fn rotate(document: &PdfDocument<'_>, pages: &[usize], turns: u8) -> Result<()> {
    for index in checked(document, pages)? {
        let mut page = document.pages().get(index as i32)?;
        let rotation = page
            .rotation()
            .with_context(|| format!("Page {} has an unreadable rotation.", index + 1))?;
        page.set_rotation(turned(rotation, turns));
    }
    Ok(())
}

pub(super) fn delete(document: &PdfDocument<'_>, pages: &[usize]) -> Result<()> {
    let pages = checked(document, pages)?;
    if pages.len() >= document.pages().len() as usize {
        bail!("A PDF needs at least one page, so they cannot all be deleted.");
    }
    for index in pages.into_iter().rev() {
        document
            .pages()
            .get(index as i32)?
            .delete()
            .with_context(|| format!("Page {} could not be deleted.", index + 1))?;
    }
    Ok(())
}

pub(super) fn insert<'a>(
    engine: &'a Pdfium,
    document: &mut PdfDocument<'a>,
    file: &Path,
) -> Result<()> {
    let other = engine.load_pdf_from_file(file, None).context(
        "The file to insert could not be opened. It may be damaged or password-protected.",
    )?;
    if other.pages().is_empty() {
        bail!("The file to insert has no pages.");
    }
    document
        .pages_mut()
        .append(&other)
        .context("The pages could not be added.")
}

pub(super) fn extract(
    engine: &Pdfium,
    document: &PdfDocument<'_>,
    pages: &[usize],
    to: &Path,
) -> Result<()> {
    let pages = checked(document, pages)?;
    let mut extracted = engine
        .create_new_pdf()
        .context("A new PDF could not be created.")?;
    for (position, index) in pages.into_iter().enumerate() {
        extracted
            .pages_mut()
            .copy_page_from_document(document, index as i32, position as i32)
            .with_context(|| format!("Page {} could not be copied.", index + 1))?;
    }
    extracted.save_to_file(to).with_context(|| {
        format!(
            "The extracted pages could not be written to {}.",
            to.display()
        )
    })
}

pub(super) fn write(document: &PdfDocument<'_>, path: &Path) -> Result<Staged> {
    if !document.signatures().is_empty() {
        bail!(
            "This PDF is digitally signed. Changing its pages would invalidate the signature, so it is left unchanged."
        );
    }
    let staged = stage(path)?;
    match document
        .save_to_file(staged.temporary())
        .context("The changes could not be written next to the PDF.")
    {
        Ok(()) => Ok(staged),
        Err(error) => {
            staged.discard();
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarter_turns_wrap_in_both_directions() {
        use PdfPageRenderRotation::*;
        assert!(matches!(turned(None, 1), Degrees90));
        assert!(matches!(turned(Degrees270, 1), None));
        assert!(matches!(turned(None, 3), Degrees270));
        assert!(matches!(turned(Degrees90, 3), None));
        assert!(matches!(turned(Degrees180, 4), Degrees180));
    }
}
