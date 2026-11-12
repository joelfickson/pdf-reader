use crate::pdf::{
    bind,
    search::Query,
    tools::{self, DocumentInfo, Outline, SearchResults, TextRange},
};
use anyhow::{Context, Result};
use pdfium_render::prelude::*;
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::SystemTime,
};
use tokio::sync::oneshot;

const OPEN_DOCUMENTS: usize = 4;

type Reply<T> = oneshot::Sender<Result<T, String>>;

enum Request {
    Info(PathBuf, Reply<DocumentInfo>),
    Outline(PathBuf, Reply<Outline>),
    Text {
        path: PathBuf,
        from_page: usize,
        max_chars: usize,
        reply: Reply<TextRange>,
    },
    Search {
        path: PathBuf,
        query: Query,
        max_results: usize,
        reply: Reply<SearchResults>,
    },
}

#[derive(Clone)]
pub(crate) struct Engine(mpsc::Sender<Request>);

type CacheKey = (PathBuf, Option<SystemTime>);

pub(crate) fn resolve(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("path is required".into());
    }
    let candidate = PathBuf::from(trimmed);
    let absolute = if candidate.is_absolute() {
        candidate
    } else {
        std::env::current_dir()
            .map_err(|error| format!("could not resolve a relative path: {error}"))?
            .join(candidate)
    };
    if !absolute
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
    {
        return Err(format!("{} is not a .pdf file", absolute.display()));
    }
    let canonical = absolute
        .canonicalize()
        .map_err(|error| format!("{} could not be opened: {error}", absolute.display()))?;
    if !canonical.is_file() {
        return Err(format!("{} is not a file", canonical.display()));
    }
    Ok(canonical)
}

fn describe(error: &PdfiumError, path: &Path) -> String {
    match error {
        PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError) => {
            format!(
                "{} is password-protected, which is not supported yet",
                path.display()
            )
        }
        _ => format!("{} could not be opened as a PDF: {error}", path.display()),
    }
}

fn document<'a, 'b>(
    engine: &'a Pdfium,
    cache: &'b mut VecDeque<(CacheKey, PdfDocument<'a>)>,
    path: &Path,
) -> Result<&'b PdfDocument<'a>, String> {
    let modified = std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok();
    let key = (path.to_path_buf(), modified);
    if let Some(index) = cache.iter().position(|(cached, _)| *cached == key) {
        if let Some(entry) = cache.remove(index) {
            cache.push_back(entry);
        }
    } else {
        cache.retain(|((cached, _), _)| cached != path);
        let loaded = engine
            .load_pdf_from_file(path, None)
            .map_err(|error| describe(&error, path))?;
        cache.push_back((key, loaded));
        while cache.len() > OPEN_DOCUMENTS {
            cache.pop_front();
        }
    }
    cache
        .back()
        .map(|(_, loaded)| loaded)
        .ok_or_else(|| "no document is open".to_string())
}

fn run(engine: Pdfium, requests: mpsc::Receiver<Request>) {
    let mut cache = VecDeque::new();
    for request in requests {
        match request {
            Request::Info(path, reply) => {
                let result = document(&engine, &mut cache, &path).and_then(|doc| {
                    tools::document_info(doc).map_err(|error| format!("{error:#}"))
                });
                let _ = reply.send(result);
            }
            Request::Outline(path, reply) => {
                let result = document(&engine, &mut cache, &path).map(tools::outline);
                let _ = reply.send(result);
            }
            Request::Text {
                path,
                from_page,
                max_chars,
                reply,
            } => {
                let result = document(&engine, &mut cache, &path).and_then(|doc| {
                    tools::text_range(doc, from_page, max_chars)
                        .map_err(|error| format!("{error:#}"))
                });
                let _ = reply.send(result);
            }
            Request::Search {
                path,
                query,
                max_results,
                reply,
            } => {
                let result = document(&engine, &mut cache, &path).and_then(|doc| {
                    tools::search_document(doc, &query, max_results)
                        .map_err(|error| format!("{error:#}"))
                });
                let _ = reply.send(result);
            }
        }
    }
}

impl Engine {
    pub fn start() -> Result<Self> {
        let engine = bind()?;
        let (sender, receiver) = mpsc::channel();
        thread::Builder::new()
            .name("pdf-tools".into())
            .spawn(move || run(engine, receiver))
            .context("Could not start the PDF tools thread")?;
        Ok(Self(sender))
    }

    async fn call<T>(&self, build: impl FnOnce(Reply<T>) -> Request) -> Result<T, String> {
        let (reply, response) = oneshot::channel();
        self.0
            .send(build(reply))
            .map_err(|_| "the PDF tools thread has stopped".to_string())?;
        response
            .await
            .map_err(|_| "the PDF tools thread stopped before replying".to_string())?
    }

    pub async fn info(&self, path: PathBuf) -> Result<DocumentInfo, String> {
        self.call(|reply| Request::Info(path, reply)).await
    }

    pub async fn outline(&self, path: PathBuf) -> Result<Outline, String> {
        self.call(|reply| Request::Outline(path, reply)).await
    }

    pub async fn text(
        &self,
        path: PathBuf,
        from_page: usize,
        max_chars: usize,
    ) -> Result<TextRange, String> {
        self.call(|reply| Request::Text {
            path,
            from_page,
            max_chars,
            reply,
        })
        .await
    }

    pub async fn search(
        &self,
        path: PathBuf,
        query: Query,
        max_results: usize,
    ) -> Result<SearchResults, String> {
        self.call(|reply| Request::Search {
            path,
            query,
            max_results,
            reply,
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_accepts_only_existing_pdf_files() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reading.pdf");
        assert_eq!(
            resolve(fixture.to_str().unwrap()).unwrap(),
            fixture.canonicalize().unwrap()
        );
        assert!(resolve("").unwrap_err().contains("required"));
        assert!(resolve("/etc/hosts").unwrap_err().contains("not a .pdf"));
        assert!(
            resolve("/definitely/missing.pdf")
                .unwrap_err()
                .contains("could not be opened")
        );
        let directory =
            std::env::temp_dir().join(format!("agenticpdf-dir-{}.pdf", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        assert!(
            resolve(directory.to_str().unwrap())
                .unwrap_err()
                .contains("not a file")
        );
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
