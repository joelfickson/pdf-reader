mod engine;
mod server;

use anyhow::{Context, Result};
use engine::Engine;
use rmcp::{ServiceExt, transport::stdio};
use server::PdfServer;

pub(crate) fn serve() -> Result<()> {
    let engine = Engine::start()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .context("Could not start the MCP runtime")?;
    runtime.block_on(async move {
        let service = PdfServer::new(engine)
            .serve(stdio())
            .await
            .context("Could not start the MCP server")?;
        service
            .waiting()
            .await
            .context("The MCP server stopped with an error")?;
        Ok(())
    })
}
