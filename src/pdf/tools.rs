use super::{search, text};
use anyhow::{Result, bail};
use pdfium_render::prelude::*;
use schemars::{JsonSchema, transform::RestrictFormats};
use serde::Serialize;

pub(crate) const DEFAULT_MAX_CHARS: usize = 20_000;
pub(crate) const MAX_CHARS_LIMIT: usize = 100_000;
pub(crate) const DEFAULT_MAX_RESULTS: usize = 50;
pub(crate) const MAX_RESULTS_LIMIT: usize = 500;
const MAX_OUTLINE_ENTRIES: usize = 2_000;
const MAX_OUTLINE_DEPTH: usize = 32;
const MAX_LISTED_PAGES: usize = 1_000;

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = RestrictFormats::default())]
pub(crate) struct DocumentInfo {
    pub page_count: usize,
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    #[schemars(description = "1-based pages with no text layer, such as scanned images")]
    pub pages_without_text: Vec<usize>,
    #[schemars(description = "1-based pages with more text than the reader extracts")]
    pub pages_too_large: Vec<usize>,
    #[schemars(description = "True when the page lists were cut short")]
    pub page_lists_truncated: bool,
}

#[derive(Debug, PartialEq, Serialize, JsonSchema)]
#[schemars(transform = RestrictFormats::default())]
pub(crate) struct OutlineEntry {
    pub title: String,
    #[schemars(description = "Nesting depth, starting at 1")]
    pub level: usize,
    #[schemars(description = "1-based page the entry points to, if any")]
    pub page: Option<usize>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = RestrictFormats::default())]
pub(crate) struct Outline {
    pub entries: Vec<OutlineEntry>,
    pub truncated: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = RestrictFormats::default())]
pub(crate) struct PageText {
    pub page: usize,
    pub text: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = RestrictFormats::default())]
pub(crate) struct TextRange {
    pub page_count: usize,
    pub pages: Vec<PageText>,
    #[schemars(description = "Page to pass as from_page to continue reading, or null when done")]
    pub next_page: Option<usize>,
    pub pages_without_text: Vec<usize>,
    pub pages_too_large: Vec<usize>,
    #[schemars(description = "True when the only returned page was cut to max_chars")]
    pub truncated: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = RestrictFormats::default())]
pub(crate) struct SearchMatch {
    pub page: usize,
    pub before: String,
    pub matched: String,
    pub after: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = RestrictFormats::default())]
pub(crate) struct SearchResults {
    pub matches: Vec<SearchMatch>,
    #[schemars(description = "True when more matches exist than max_results")]
    pub truncated: bool,
    pub pages_too_large: Vec<usize>,
}

enum Page {
    Text(String),
    Empty,
    TooLarge,
}

fn read_page(document: &PdfDocument<'_>, index: usize) -> Result<Page> {
    let page = document.pages().get(i32::try_from(index)?)?;
    Ok(match text::characters(&page) {
        None => Page::TooLarge,
        Some(characters) => {
            let content: String = characters
                .into_iter()
                .filter(|value| !matches!(value, '\r' | '\0' | '\u{2}'))
                .collect();
            if content.trim().is_empty() {
                Page::Empty
            } else {
                Page::Text(content)
            }
        }
    })
}

fn metadata(document: &PdfDocument<'_>, tag: PdfDocumentMetadataTagType) -> Option<String> {
    document
        .metadata()
        .get(tag)
        .map(|entry| entry.value().trim().to_string())
        .filter(|value| !value.is_empty())
}

fn push_limited(list: &mut Vec<usize>, page: usize, truncated: &mut bool) {
    if list.len() < MAX_LISTED_PAGES {
        list.push(page);
    } else {
        *truncated = true;
    }
}

pub(crate) fn document_info(document: &PdfDocument<'_>) -> Result<DocumentInfo> {
    let page_count = document.pages().len() as usize;
    let mut pages_without_text = Vec::new();
    let mut pages_too_large = Vec::new();
    let mut page_lists_truncated = false;
    for index in 0..page_count {
        match read_page(document, index)? {
            Page::Text(_) => {}
            Page::Empty => push_limited(
                &mut pages_without_text,
                index + 1,
                &mut page_lists_truncated,
            ),
            Page::TooLarge => {
                push_limited(&mut pages_too_large, index + 1, &mut page_lists_truncated)
            }
        }
    }
    Ok(DocumentInfo {
        page_count,
        title: metadata(document, PdfDocumentMetadataTagType::Title),
        author: metadata(document, PdfDocumentMetadataTagType::Author),
        subject: metadata(document, PdfDocumentMetadataTagType::Subject),
        creator: metadata(document, PdfDocumentMetadataTagType::Creator),
        producer: metadata(document, PdfDocumentMetadataTagType::Producer),
        pages_without_text,
        pages_too_large,
        page_lists_truncated,
    })
}

fn collect_outline(first: Option<PdfBookmark<'_>>, level: usize, outline: &mut Outline) {
    let mut current = first;
    while let Some(bookmark) = current {
        if outline.entries.len() >= MAX_OUTLINE_ENTRIES {
            outline.truncated = true;
            return;
        }
        outline.entries.push(OutlineEntry {
            title: bookmark.title().unwrap_or_default().trim().to_string(),
            level,
            page: bookmark
                .destination()
                .and_then(|destination| destination.page_index().ok())
                .map(|index| index as usize + 1),
        });
        if level < MAX_OUTLINE_DEPTH {
            collect_outline(bookmark.first_child(), level + 1, outline);
        } else if bookmark.first_child().is_some() {
            outline.truncated = true;
        }
        current = bookmark.next_sibling();
    }
}

pub(crate) fn outline(document: &PdfDocument<'_>) -> Outline {
    let mut outline = Outline {
        entries: Vec::new(),
        truncated: false,
    };
    collect_outline(document.bookmarks().root(), 1, &mut outline);
    outline
}

pub(crate) fn text_range(
    document: &PdfDocument<'_>,
    from_page: usize,
    max_chars: usize,
) -> Result<TextRange> {
    let page_count = document.pages().len() as usize;
    if from_page == 0 || from_page > page_count {
        bail!("from_page must be between 1 and {page_count}");
    }
    let budget = max_chars.clamp(1, MAX_CHARS_LIMIT);
    let mut range = TextRange {
        page_count,
        pages: Vec::new(),
        next_page: None,
        pages_without_text: Vec::new(),
        pages_too_large: Vec::new(),
        truncated: false,
    };
    let mut used = 0;
    let mut page = from_page;
    while page <= page_count {
        match read_page(document, page - 1)? {
            Page::Empty => range.pages_without_text.push(page),
            Page::TooLarge => range.pages_too_large.push(page),
            Page::Text(content) => {
                let length = content.chars().count();
                if used > 0 && used + length > budget {
                    break;
                }
                if length > budget {
                    range.pages.push(PageText {
                        page,
                        text: content.chars().take(budget).collect(),
                    });
                    range.truncated = true;
                    page += 1;
                    break;
                }
                used += length;
                range.pages.push(PageText {
                    page,
                    text: content,
                });
            }
        }
        page += 1;
    }
    range.next_page = (page <= page_count).then_some(page);
    Ok(range)
}

pub(crate) fn search_document(
    document: &PdfDocument<'_>,
    query: &search::Query,
    max_results: usize,
) -> Result<SearchResults> {
    let limit = max_results.clamp(1, MAX_RESULTS_LIMIT);
    let mut results = SearchResults {
        matches: Vec::new(),
        truncated: false,
        pages_too_large: Vec::new(),
    };
    let page_count = document.pages().len() as usize;
    for index in 0..page_count {
        let page = document.pages().get(i32::try_from(index)?)?;
        let Some(characters) = text::characters(&page) else {
            results.pages_too_large.push(index + 1);
            continue;
        };
        let remaining = limit + 1 - results.matches.len();
        for hit in search::find(&characters, query, remaining) {
            if results.matches.len() == limit {
                results.truncated = true;
                return Ok(results);
            }
            results.matches.push(SearchMatch {
                page: index + 1,
                before: hit.before,
                matched: hit.matched,
                after: hit.after,
            });
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn with_document<T>(name: &str, check: impl FnOnce(&PdfDocument<'_>) -> T) -> T {
        let engine = super::super::library::bind().unwrap();
        let document = engine.load_pdf_from_file(&fixture(name), None).unwrap();
        check(&document)
    }

    #[test]
    #[ignore = "requires PDFium installed by scripts/setup_pdfium.py"]
    fn document_info_reports_pages_and_blank_pages() {
        let info = with_document("selection.pdf", |document| document_info(document).unwrap());
        assert_eq!(info.page_count, 3);
        assert_eq!(info.pages_without_text, [3]);
        assert!(info.pages_too_large.is_empty());
    }

    #[test]
    #[ignore = "requires PDFium installed by scripts/setup_pdfium.py"]
    fn outline_is_flattened_in_reading_order_with_levels_and_pages() {
        let outline = with_document("outline.pdf", outline);
        let entries: Vec<_> = outline
            .entries
            .iter()
            .map(|entry| (entry.title.as_str(), entry.level, entry.page))
            .collect();
        assert_eq!(
            entries,
            [
                ("Part one: attention", 1, Some(1)),
                ("Begin with the page", 2, Some(1)),
                ("Part two: margins", 1, Some(2)),
                ("Leave room in the margins", 2, Some(2)),
                ("Find your own pace", 2, Some(2)),
                ("Keep what helps you read", 1, Some(3)),
            ]
        );
        assert!(!outline.truncated);
        assert!(
            with_document("reading.pdf", super::outline)
                .entries
                .is_empty()
        );
    }

    #[test]
    #[ignore = "requires PDFium installed by scripts/setup_pdfium.py"]
    fn text_range_pages_through_the_document_within_the_budget() {
        let whole = with_document("reading.pdf", |document| {
            text_range(document, 1, MAX_CHARS_LIMIT).unwrap()
        });
        assert_eq!(whole.pages.len(), 3);
        assert_eq!(whole.next_page, None);
        assert!(whole.pages[0].text.contains("step\naside"));
        assert!(!whole.pages[0].text.contains('\r'));

        let first_length = whole.pages[0].text.chars().count();
        let paged = with_document("reading.pdf", |document| {
            text_range(document, 1, first_length).unwrap()
        });
        assert_eq!(paged.pages.len(), 1);
        assert_eq!(paged.next_page, Some(2));
        assert!(!paged.truncated);

        let cut = with_document("reading.pdf", |document| {
            text_range(document, 2, 10).unwrap()
        });
        assert_eq!(cut.pages.len(), 1);
        assert_eq!(cut.pages[0].text.chars().count(), 10);
        assert!(cut.truncated);
        assert_eq!(cut.next_page, Some(3));

        let blank = with_document("selection.pdf", |document| {
            text_range(document, 3, 100).unwrap()
        });
        assert!(blank.pages.is_empty());
        assert_eq!(blank.pages_without_text, [3]);
        assert_eq!(blank.next_page, None);

        assert!(with_document("reading.pdf", |document| text_range(document, 0, 10)).is_err());
        assert!(with_document("reading.pdf", |document| text_range(document, 4, 10)).is_err());
    }

    #[test]
    #[ignore = "requires PDFium installed by scripts/setup_pdfium.py"]
    fn search_document_finds_cross_line_matches_and_caps_results() {
        let query = |text: &str| search::Query {
            text: text.into(),
            match_case: false,
            whole_word: false,
        };
        let phrase = with_document("reading.pdf", |document| {
            search_document(document, &query("step aside"), 10).unwrap()
        });
        assert_eq!(phrase.matches.len(), 1);
        assert_eq!(phrase.matches[0].page, 1);
        assert!(!phrase.truncated);

        let capped = with_document("reading.pdf", |document| {
            search_document(document, &query("e"), 3).unwrap()
        });
        assert_eq!(capped.matches.len(), 3);
        assert!(capped.truncated);
    }
}
