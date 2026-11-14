#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod agent;
mod app;
mod bench;
mod cli;
mod mcp;
mod pdf;
mod preferences;
mod reader;
mod recent;
mod ui;
mod update;

use cli::Command;

fn main() -> anyhow::Result<()> {
    bench::start();
    match cli::parse(std::env::args_os().skip(1))? {
        Command::Open(paths) => app::run(paths),
        Command::Check(path) => pdf::inspect(&path),
        Command::Mcp => mcp::serve(),
        Command::Version => {
            println!("{}", cli::VERSION_LINE);
            Ok(())
        }
    }
}
