use anyhow::{Context, Result, bail};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};

pub(crate) fn bind() -> Result<Pdfium> {
    let name = Pdfium::pdfium_platform_library_name();
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("PDFIUM_LIBRARY_PATH") {
        let path = PathBuf::from(path);
        candidates.push(if path.is_dir() {
            path.join(&name)
        } else {
            path
        });
    }
    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
    {
        candidates.push(parent.join(&name));
        candidates.push(parent.join("../Frameworks").join(&name));
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/pdfium");
    candidates.push(root.join("lib").join(&name));
    candidates.push(root.join("bin").join(&name));
    for candidate in candidates {
        if candidate.is_file() {
            return match Pdfium::bind_to_library(&candidate) {
                Ok(bindings) => Ok(Pdfium::new(bindings)),
                Err(PdfiumError::PdfiumLibraryBindingsAlreadyInitialized) => Ok(Pdfium::default()),
                Err(error) => Err(error).context(
                    "The installed PDF engine could not be loaded. Run the PDFium setup again.",
                ),
            };
        }
    }
    bail!("The PDF engine is not installed. Run: uv run scripts/setup_pdfium.py")
}
