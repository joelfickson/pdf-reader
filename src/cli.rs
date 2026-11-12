use anyhow::{Result, bail};
use std::ffi::OsString;
use std::path::PathBuf;

pub const VERSION_LINE: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Open(Vec<PathBuf>),
    Check(PathBuf),
    Mcp,
    Version,
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command> {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Ok(Command::Open(Vec::new()));
    };
    match first.to_str() {
        Some("mcp") => Ok(Command::Mcp),
        Some("--version") => Ok(Command::Version),
        Some("--check") => match (args.next(), args.next()) {
            (Some(path), None) => Ok(Command::Check(PathBuf::from(path))),
            _ => bail!("Usage: agenticpdf --check <file.pdf>"),
        },
        _ => Ok(Command::Open(
            std::iter::once(first)
                .chain(args)
                .map(PathBuf::from)
                .collect(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<Command> {
        parse(args.iter().map(OsString::from))
    }

    #[test]
    fn no_arguments_open_an_empty_window() {
        assert_eq!(run(&[]).unwrap(), Command::Open(Vec::new()));
    }

    #[test]
    fn paths_open_in_order() {
        assert_eq!(
            run(&["a.pdf", "b.pdf"]).unwrap(),
            Command::Open(vec![PathBuf::from("a.pdf"), PathBuf::from("b.pdf")])
        );
    }

    #[test]
    fn version_and_mcp_are_recognized() {
        assert_eq!(run(&["--version"]).unwrap(), Command::Version);
        assert_eq!(run(&["mcp"]).unwrap(), Command::Mcp);
    }

    #[test]
    fn check_requires_exactly_one_path() {
        assert_eq!(
            run(&["--check", "a.pdf"]).unwrap(),
            Command::Check(PathBuf::from("a.pdf"))
        );
        assert!(run(&["--check"]).is_err());
        assert!(run(&["--check", "a.pdf", "b.pdf"]).is_err());
    }

    #[test]
    fn version_line_names_the_package() {
        assert_eq!(
            VERSION_LINE,
            format!("agenticpdf {}", env!("CARGO_PKG_VERSION"))
        );
    }
}
