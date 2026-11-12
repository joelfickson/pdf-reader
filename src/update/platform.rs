use super::install as files;
use anyhow::{Context, Result, bail};
use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

pub(crate) fn archive_name() -> Option<&'static str> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("agenticpdf-macos-arm64.zip")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("agenticpdf-macos-x64.zip")
    } else if cfg!(all(windows, target_arch = "x86_64")) {
        Some("agenticpdf-windows-x64.zip")
    } else {
        None
    }
}

fn run(program: &str, args: &[&std::ffi::OsStr]) -> Result<Output> {
    let mut command = Command::new(program);
    command.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command
        .output()
        .with_context(|| format!("Could not run {program}"))
}

fn checked(program: &str, args: &[&std::ffi::OsStr], failure: &str) -> Result<Output> {
    let output = run(program, args)?;
    if !output.status.success() {
        bail!(
            "{failure}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output)
}

#[cfg(any(target_os = "macos", test))]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Signature {
    pub identifier: String,
    pub team: Option<String>,
}

#[cfg(any(target_os = "macos", test))]
pub(crate) fn parse_signature(details: &str) -> Option<Signature> {
    let field = |name: &str| {
        details
            .lines()
            .find_map(|line| line.strip_prefix(name)?.strip_prefix('='))
            .map(str::trim)
    };
    let identifier = field("Identifier")?.to_string();
    let team = field("TeamIdentifier")
        .filter(|team| !team.is_empty() && *team != "not set")
        .map(str::to_string);
    Some(Signature { identifier, team })
}

pub(crate) struct Installation {
    pub target: PathBuf,
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;

    const CODESIGN: &str = "/usr/bin/codesign";
    const STAGED_APP: &str = "Agentic PDF.app";

    fn signature(bundle: &Path) -> Result<Signature> {
        checked(
            CODESIGN,
            &[
                "--verify".as_ref(),
                "--deep".as_ref(),
                "--strict".as_ref(),
                bundle.as_os_str(),
            ],
            &format!("{} failed its code signature check", bundle.display()),
        )?;
        let output = checked(
            CODESIGN,
            &["-d".as_ref(), "--verbose=2".as_ref(), bundle.as_os_str()],
            "Could not read the code signature",
        )?;
        parse_signature(&String::from_utf8_lossy(&output.stderr))
            .context("The code signature has no identifier")
    }

    pub fn locate() -> Result<Installation> {
        let executable = std::env::current_exe()?.canonicalize()?;
        let bundle = executable
            .ancestors()
            .nth(3)
            .filter(|bundle| {
                bundle
                    .extension()
                    .is_some_and(|extension| extension == "app")
            })
            .context("Agentic PDF is not running from an app bundle")?
            .to_path_buf();
        if bundle
            .components()
            .any(|part| part.as_os_str() == "AppTranslocation")
        {
            bail!("Move Agentic PDF to Applications to install updates");
        }
        files::writable(bundle.parent().context("The app bundle has no folder")?)?;
        Ok(Installation { target: bundle })
    }

    pub fn staged_in(folder: &Path) -> PathBuf {
        folder.join(STAGED_APP)
    }

    pub fn extract(archive: &Path, folder: &Path) -> Result<()> {
        checked(
            "/usr/bin/ditto",
            &[
                "-x".as_ref(),
                "-k".as_ref(),
                archive.as_os_str(),
                folder.as_os_str(),
            ],
            "The update could not be unpacked",
        )
        .map(drop)
    }

    pub fn verify(installation: &Installation, staged: &Path) -> Result<()> {
        let current = signature(&installation.target)?;
        let Some(team) = current.team.as_deref() else {
            bail!("This copy of Agentic PDF is not signed, so updates cannot be verified");
        };
        let incoming = signature(staged)?;
        if incoming.identifier != current.identifier || incoming.team.as_deref() != Some(team) {
            bail!("The update is signed by a different developer");
        }
        Ok(())
    }

    pub fn install(installation: &Installation, staged: &Path) -> Result<()> {
        files::swap(&installation.target, staged, |source, destination| {
            checked(
                "/usr/bin/ditto",
                &[source.as_os_str(), destination.as_os_str()],
                "The update could not be copied",
            )
            .map(drop)
        })
    }

    pub fn relaunch(installation: &Installation, documents: &[PathBuf]) -> Result<()> {
        let mut command = Command::new("/usr/bin/open");
        command.arg("-n").arg(&installation.target);
        if !documents.is_empty() {
            command.arg("--args").args(documents);
        }
        command.spawn().context("Could not reopen Agentic PDF")?;
        Ok(())
    }

    pub fn leftovers(installation: &Installation) -> Option<PathBuf> {
        installation.target.parent().map(Path::to_path_buf)
    }
}

#[cfg(windows)]
mod native {
    use super::*;

    const EXECUTABLE: &str = "agenticpdf.exe";
    const REQUIRED: [&str; 2] = [EXECUTABLE, "pdfium.dll"];

    pub fn locate() -> Result<Installation> {
        let executable = std::env::current_exe()?;
        let folder = executable
            .parent()
            .context("Agentic PDF has no install folder")?
            .to_path_buf();
        files::writable(&folder)?;
        Ok(Installation { target: folder })
    }

    pub fn staged_in(folder: &Path) -> PathBuf {
        folder.to_path_buf()
    }

    pub fn extract(archive: &Path, folder: &Path) -> Result<()> {
        let system = std::env::var_os("SystemRoot").context("SystemRoot is not set")?;
        let tar = PathBuf::from(system).join("System32").join("tar.exe");
        std::fs::create_dir_all(folder)?;
        checked(
            &tar.to_string_lossy(),
            &[
                "-xf".as_ref(),
                archive.as_os_str(),
                "-C".as_ref(),
                folder.as_os_str(),
            ],
            "The update could not be unpacked",
        )
        .map(drop)
    }

    pub fn verify(_: &Installation, staged: &Path) -> Result<()> {
        for name in REQUIRED {
            if !staged.join(name).is_file() {
                bail!("The update is missing {name}");
            }
        }
        Ok(())
    }

    pub fn install(installation: &Installation, staged: &Path) -> Result<()> {
        files::swap_entries(&installation.target, staged, files::copy_tree)
    }

    pub fn relaunch(installation: &Installation, documents: &[PathBuf]) -> Result<()> {
        Command::new(installation.target.join(EXECUTABLE))
            .args(documents)
            .spawn()
            .context("Could not reopen Agentic PDF")?;
        Ok(())
    }

    pub fn leftovers(installation: &Installation) -> Option<PathBuf> {
        Some(installation.target.clone())
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod native {
    use super::*;

    pub fn locate() -> Result<Installation> {
        bail!("Updates are not available on this platform")
    }

    pub fn staged_in(folder: &Path) -> PathBuf {
        folder.to_path_buf()
    }

    pub fn extract(_: &Path, _: &Path) -> Result<()> {
        bail!("Updates are not available on this platform")
    }

    pub fn verify(_: &Installation, _: &Path) -> Result<()> {
        bail!("Updates are not available on this platform")
    }

    pub fn install(_: &Installation, _: &Path) -> Result<()> {
        bail!("Updates are not available on this platform")
    }

    pub fn relaunch(_: &Installation, _: &[PathBuf]) -> Result<()> {
        bail!("Updates are not available on this platform")
    }

    pub fn leftovers(_: &Installation) -> Option<PathBuf> {
        None
    }
}

pub(crate) use native::{extract, install, locate, relaunch, staged_in, verify};

pub(crate) fn clean_leftovers() -> Result<()> {
    let installation = native::locate()?;
    match native::leftovers(&installation) {
        Some(folder) => files::clean(&folder),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_details_yield_identifier_and_team() {
        let signed = "Executable=/Applications/Agentic PDF.app/Contents/MacOS/agenticpdf\nIdentifier=app.agenticpdf.desktop\nFormat=app bundle with Mach-O thin (arm64)\nTeamIdentifier=ABCDE12345\nSealed Resources version=2 rules=13 files=4\n";
        assert_eq!(
            parse_signature(signed),
            Some(Signature {
                identifier: "app.agenticpdf.desktop".into(),
                team: Some("ABCDE12345".into()),
            })
        );
        let ad_hoc = "Identifier=app.agenticpdf.desktop\nSignature=adhoc\nTeamIdentifier=not set\n";
        assert_eq!(parse_signature(ad_hoc).unwrap().team, None);
        assert_eq!(parse_signature("Format=app bundle\n"), None);
    }
}
