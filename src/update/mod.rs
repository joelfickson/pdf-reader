mod fetch;
mod install;
mod platform;
mod release;
mod settings;
mod version;

use anyhow::{Context, Result, anyhow};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
pub(crate) use release::Asset;
pub(crate) use release::Release;
pub(crate) use settings::Settings;
pub(crate) use version::Version;

pub(crate) const REPOSITORY: &str = "joelfickson/agenticpdf";
const RELEASE_TAG: Option<&str> = option_env!("AGENTICPDF_RELEASE_TAG");
const UNPACKED: &str = "unpacked";
const VERIFIED: &str = "verified";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Channel {
    Stable,
    Beta,
}

#[derive(Clone, Debug)]
pub(crate) struct Build {
    pub version: Version,
    archive: &'static str,
    store: PathBuf,
}

pub(crate) fn current_label() -> String {
    RELEASE_TAG
        .and_then(Version::parse_tag)
        .map(|version| version.to_string())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

pub(crate) fn build(store: Option<PathBuf>) -> Result<Build, &'static str> {
    if cfg!(test) {
        return Err("Updates are turned off in tests");
    }
    let version = RELEASE_TAG
        .and_then(Version::parse_tag)
        .ok_or("Updates are available in release builds only")?;
    let archive = platform::archive_name().ok_or("Updates are not available on this platform")?;
    let store = store.ok_or("There is no folder to keep downloaded updates in")?;
    Ok(Build {
        version,
        archive,
        store: store.join("updates"),
    })
}

#[derive(Clone, Debug)]
pub(crate) struct Progress {
    pub version: Version,
    pub received: u64,
    pub total: u64,
}

#[derive(Debug)]
pub(crate) enum Outcome {
    Current,
    Ready {
        release: Box<Release>,
        staged: PathBuf,
    },
}

fn remove_folder(path: &Path) -> Result<()> {
    match fs::remove_dir_all(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(error).with_context(|| format!("Could not remove {}", path.display()))
        }
        _ => Ok(()),
    }
}

fn prune(build: &Build, keep: Option<&Path>) -> Result<()> {
    let entries = match fs::read_dir(&build.store) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("Could not list {}", build.store.display()));
        }
    };
    for entry in entries {
        let path = entry?.path();
        if Some(path.as_path()) != keep && path.is_dir() {
            remove_folder(&path)?;
        }
    }
    Ok(())
}

pub(crate) fn check(
    build: &Build,
    channel: Channel,
    mut progress: impl FnMut(Progress),
) -> Result<Outcome> {
    let installation = platform::locate()?;
    let client = fetch::Client::new();
    let Some(release) = release::select(client.releases()?, &build.version, channel, build.archive)
    else {
        prune(build, None)?;
        return Ok(Outcome::Current);
    };
    let folder = build.store.join(&release.tag);
    let unpacked = folder.join(UNPACKED);
    let staged = platform::staged_in(&unpacked);
    prune(build, Some(&folder))?;
    if folder.join(VERIFIED).is_file()
        && staged.exists()
        && platform::verify(&installation, &staged).is_ok()
    {
        return Ok(Outcome::Ready {
            release: Box::new(release),
            staged,
        });
    }
    remove_folder(&folder)?;
    fs::create_dir_all(&unpacked)
        .with_context(|| format!("Could not create {}", unpacked.display()))?;
    let total = release.archive.size;
    progress(Progress {
        version: release.version.clone(),
        received: 0,
        total,
    });
    let discard = |error: anyhow::Error| match remove_folder(&folder) {
        Ok(()) => error,
        Err(cleanup) => anyhow!("{error:#}. {cleanup:#}"),
    };
    let sums = client.text(&release.sums.url).map_err(discard)?;
    let expected = release::checksum(&sums, &release.archive.name)
        .map(str::to_ascii_lowercase)
        .with_context(|| {
            format!(
                "{} has no entry for {}",
                release::SUMS_NAME,
                release.archive.name
            )
        })
        .map_err(discard)?;
    let archive = folder.join(&release.archive.name);
    let hash = client
        .download(&release.archive, &archive, |received| {
            progress(Progress {
                version: release.version.clone(),
                received,
                total,
            })
        })
        .map_err(discard)?;
    if hash != expected {
        return Err(discard(anyhow!(
            "The {} download did not match its checksum and was removed",
            release.version
        )));
    }
    platform::extract(&archive, &unpacked).map_err(discard)?;
    fs::remove_file(&archive)
        .with_context(|| format!("Could not remove {}", archive.display()))
        .map_err(discard)?;
    if let Err(error) = platform::verify(&installation, &staged) {
        return Err(discard(anyhow!(
            "The {} download failed verification and was removed. {error:#}",
            release.version
        )));
    }
    fs::write(folder.join(VERIFIED), &hash)
        .with_context(|| format!("Could not write {}", folder.display()))
        .map_err(discard)?;
    Ok(Outcome::Ready {
        release: Box::new(release),
        staged,
    })
}

pub(crate) fn install(build: &Build, staged: &Path, reopen: Option<&[PathBuf]>) -> Result<()> {
    let installation = platform::locate()?;
    platform::verify(&installation, staged)?;
    platform::install(&installation, staged)?;
    prune(build, None)?;
    match reopen {
        Some(documents) => platform::relaunch(&installation, documents),
        None => Ok(()),
    }
}

pub(crate) fn clean_leftovers() -> Result<()> {
    if cfg!(test) || RELEASE_TAG.is_none() {
        return Ok(());
    }
    platform::clean_leftovers()
}
