use super::engine::{Engine, resolve};
use crate::pdf::{
    search::Query,
    tools::{
        DEFAULT_MAX_CHARS, DEFAULT_MAX_RESULTS, DocumentInfo, Outline, SearchResults, TextRange,
    },
};
use rmcp::{
    ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{Implementation, ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router,
};
use schemars::{JsonSchema, transform::RestrictFormats};
use serde::Deserialize;

const INSTRUCTIONS: &str = "Read-only tools for local PDF files. Pass an absolute path to a .pdf file. Page numbers are 1-based. Start with document_info and get_outline, then read text in ranges with get_text, passing next_page as from_page until it is null. Pages listed in pages_without_text have no text layer, so say they could not be read instead of skipping them.";

#[derive(Deserialize, JsonSchema)]
#[schemars(transform = RestrictFormats::default())]
pub(crate) struct PathParams {
    #[schemars(description = "Absolute path to a .pdf file")]
    path: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(transform = RestrictFormats::default())]
pub(crate) struct TextParams {
    #[schemars(description = "Absolute path to a .pdf file")]
    path: String,
    #[schemars(description = "1-based page to start reading from (default 1)")]
    from_page: Option<usize>,
    #[schemars(description = "Maximum characters to return, at most 100000 (default 20000)")]
    max_chars: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(transform = RestrictFormats::default())]
pub(crate) struct SearchParams {
    #[schemars(description = "Absolute path to a .pdf file")]
    path: String,
    #[schemars(description = "Text to find. Matches across line breaks")]
    query: String,
    #[schemars(description = "Match upper and lower case exactly (default false)")]
    match_case: Option<bool>,
    #[schemars(description = "Only match whole words (default false)")]
    whole_word: Option<bool>,
    #[schemars(description = "Maximum matches to return, at most 500 (default 50)")]
    max_results: Option<usize>,
}

#[derive(Clone)]
pub(crate) struct PdfServer {
    engine: Engine,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl PdfServer {
    pub fn new(engine: Engine) -> Self {
        Self {
            engine,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Page count, metadata, and which pages have no text layer (for example scanned pages)",
        annotations(read_only_hint = true)
    )]
    async fn document_info(
        &self,
        Parameters(params): Parameters<PathParams>,
    ) -> Result<Json<DocumentInfo>, String> {
        self.engine.info(resolve(&params.path)?).await.map(Json)
    }

    #[tool(
        description = "The document's table of contents as a flat list with nesting levels and 1-based page numbers. Empty when the PDF has no outline",
        annotations(read_only_hint = true)
    )]
    async fn get_outline(
        &self,
        Parameters(params): Parameters<PathParams>,
    ) -> Result<Json<Outline>, String> {
        self.engine.outline(resolve(&params.path)?).await.map(Json)
    }

    #[tool(
        description = "Text of consecutive pages, starting at from_page and stopping before max_chars is exceeded. Continue with next_page until it is null",
        annotations(read_only_hint = true)
    )]
    async fn get_text(
        &self,
        Parameters(params): Parameters<TextParams>,
    ) -> Result<Json<TextRange>, String> {
        self.engine
            .text(
                resolve(&params.path)?,
                params.from_page.unwrap_or(1),
                params.max_chars.unwrap_or(DEFAULT_MAX_CHARS),
            )
            .await
            .map(Json)
    }

    #[tool(
        description = "Find text in the whole document, ignoring case and accents by default. Returns each match with its 1-based page and surrounding text",
        annotations(read_only_hint = true)
    )]
    async fn search(
        &self,
        Parameters(params): Parameters<SearchParams>,
    ) -> Result<Json<SearchResults>, String> {
        if params.query.trim().is_empty() {
            return Err("query must not be empty".into());
        }
        let query = Query {
            text: params.query,
            match_case: params.match_case.unwrap_or(false),
            whole_word: params.whole_word.unwrap_or(false),
        };
        self.engine
            .search(
                resolve(&params.path)?,
                query,
                params.max_results.unwrap_or(DEFAULT_MAX_RESULTS),
            )
            .await
            .map(Json)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for PdfServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "agentic-pdf",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(INSTRUCTIONS)
    }
}
