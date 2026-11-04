#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod pdf;
mod reader;

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let first = args.next();
    if first.as_deref() == Some(std::ffi::OsStr::new("--check")) {
        let path = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("Usage: pdf-reader --check <file.pdf>"))?;
        return pdf::inspect(&PathBuf::from(path));
    }
    app::run(first.into_iter().chain(args).map(PathBuf::from).collect())
}
