use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

const INCOMING: &str = "incoming";
const PREVIOUS: &str = "previous";

fn sibling(path: &Path, suffix: &str) -> Result<PathBuf> {
    let name = path
        .file_name()
        .with_context(|| format!("{} has no file name", path.display()))?;
    let parent = path
        .parent()
        .with_context(|| format!("{} has no parent folder", path.display()))?;
    Ok(parent.join(format!(".{}.{suffix}", name.to_string_lossy())))
}

fn remove(path: &Path) -> Result<()> {
    let result = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => Err(error),
    };
    result.with_context(|| format!("Could not remove {}", path.display()))
}

#[cfg(any(windows, test))]
pub(crate) fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)
        .with_context(|| format!("Could not read {}", source.display()))?;
    if metadata.is_dir() {
        fs::create_dir(destination)
            .with_context(|| format!("Could not create {}", destination.display()))?;
        for entry in
            fs::read_dir(source).with_context(|| format!("Could not list {}", source.display()))?
        {
            let entry = entry?;
            copy_tree(&entry.path(), &destination.join(entry.file_name()))?;
        }
        fs::set_permissions(destination, metadata.permissions())?;
    } else if metadata.is_file() {
        fs::copy(source, destination).with_context(|| {
            format!(
                "Could not copy {} to {}",
                source.display(),
                destination.display()
            )
        })?;
    } else {
        anyhow::bail!("{} is not a regular file or folder", source.display());
    }
    Ok(())
}

pub(crate) fn swap(
    target: &Path,
    staged: &Path,
    copy: impl Fn(&Path, &Path) -> Result<()>,
) -> Result<()> {
    let incoming = sibling(target, INCOMING)?;
    let previous = sibling(target, PREVIOUS)?;
    remove(&incoming)?;
    remove(&previous)?;
    copy(staged, &incoming)?;
    let existed = fs::symlink_metadata(target).is_ok();
    if existed {
        fs::rename(target, &previous)
            .with_context(|| format!("Could not move the current {} aside", target.display()))?;
    }
    if let Err(error) = fs::rename(&incoming, target) {
        if existed {
            fs::rename(&previous, target).with_context(|| {
                format!(
                    "Could not put {} back after a failed update",
                    target.display()
                )
            })?;
        }
        return Err(error).with_context(|| format!("Could not install {}", target.display()));
    }
    Ok(())
}

#[cfg(any(windows, test))]
pub(crate) fn swap_entries(
    folder: &Path,
    staged: &Path,
    copy: impl Fn(&Path, &Path) -> Result<()>,
) -> Result<()> {
    let mut entries: Vec<PathBuf> = fs::read_dir(staged)
        .with_context(|| format!("Could not list {}", staged.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<_>>()?;
    entries.sort();
    let mut done: Vec<PathBuf> = Vec::new();
    for source in entries {
        let target = folder.join(source.file_name().context("Staged entry has no name")?);
        if let Err(error) = swap(&target, &source, &copy) {
            for installed in done.iter().rev() {
                let previous = sibling(installed, PREVIOUS)?;
                if fs::symlink_metadata(&previous).is_ok() {
                    remove(installed)?;
                    fs::rename(&previous, installed)?;
                }
            }
            return Err(error);
        }
        done.push(target);
    }
    Ok(())
}

pub(crate) fn clean(folder: &Path) -> Result<()> {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("Could not list {}", folder.display()));
        }
    };
    for entry in entries {
        let path = entry?.path();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let leftover = name.starts_with('.')
            && (name.ends_with(&format!(".{INCOMING}")) || name.ends_with(&format!(".{PREVIOUS}")));
        if leftover {
            remove(&path)?;
        }
    }
    Ok(())
}

pub(crate) fn writable(folder: &Path) -> Result<()> {
    let probe = folder.join(format!(".agenticpdf-write-test-{}", std::process::id()));
    fs::write(&probe, b"")
        .and_then(|_| fs::remove_file(&probe))
        .with_context(|| {
            format!(
                "Agentic PDF cannot write to {}. Move it to a folder you own to install updates.",
                folder.display()
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::bail;

    fn scratch(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("agenticpdf-install-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn read(path: &Path) -> String {
        fs::read_to_string(path).unwrap()
    }

    #[test]
    fn bundle_swap_replaces_the_target_and_keeps_the_previous_copy_aside() {
        let root = scratch("bundle");
        let target = root.join("apps/Agentic PDF.app");
        let staged = root.join("cache/Agentic PDF.app");
        write(&target.join("Contents/MacOS/agenticpdf"), "old");
        write(&target.join("Contents/Resources/gone"), "old only");
        write(&staged.join("Contents/MacOS/agenticpdf"), "new");
        swap(&target, &staged, copy_tree).unwrap();
        assert_eq!(read(&target.join("Contents/MacOS/agenticpdf")), "new");
        assert!(!target.join("Contents/Resources/gone").exists());
        let previous = root.join("apps/.Agentic PDF.app.previous");
        assert_eq!(read(&previous.join("Contents/MacOS/agenticpdf")), "old");
        assert_eq!(read(&staged.join("Contents/MacOS/agenticpdf")), "new");
        clean(&root.join("apps")).unwrap();
        assert!(!previous.exists());
        assert!(target.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_copies_leave_the_installed_app_untouched() {
        let root = scratch("failed-copy");
        let target = root.join("Agentic PDF.app");
        write(&target.join("binary"), "old");
        let failed = swap(&target, &root.join("missing.app"), copy_tree);
        assert!(failed.is_err());
        assert_eq!(read(&target.join("binary")), "old");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn folder_swap_replaces_each_staged_entry_and_rolls_back_on_failure() {
        let root = scratch("folder");
        let folder = root.join("Agentic PDF");
        let staged = root.join("staged");
        write(&folder.join("agenticpdf.exe"), "old exe");
        write(&folder.join("pdfium.dll"), "old dll");
        write(&folder.join("licenses/PDFium-LICENSE"), "old license");
        write(&folder.join("notes.txt"), "user file");
        write(&staged.join("agenticpdf.exe"), "new exe");
        write(&staged.join("pdfium.dll"), "new dll");
        write(&staged.join("licenses/PDFium-LICENSE"), "new license");
        swap_entries(&folder, &staged, copy_tree).unwrap();
        assert_eq!(read(&folder.join("agenticpdf.exe")), "new exe");
        assert_eq!(read(&folder.join("pdfium.dll")), "new dll");
        assert_eq!(read(&folder.join("licenses/PDFium-LICENSE")), "new license");
        assert_eq!(read(&folder.join("notes.txt")), "user file");
        assert_eq!(read(&folder.join(".agenticpdf.exe.previous")), "old exe");
        clean(&folder).unwrap();
        let mut names: Vec<_> = fs::read_dir(&folder)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(
            names,
            ["agenticpdf.exe", "licenses", "notes.txt", "pdfium.dll"]
        );

        write(&staged.join("agenticpdf.exe"), "newer exe");
        let failing = |source: &Path, destination: &Path| {
            if source.ends_with("pdfium.dll") {
                bail!("disk full")
            }
            copy_tree(source, destination)
        };
        assert!(swap_entries(&folder, &staged, failing).is_err());
        assert_eq!(read(&folder.join("agenticpdf.exe")), "new exe");
        assert_eq!(read(&folder.join("licenses/PDFium-LICENSE")), "new license");
        assert_eq!(read(&folder.join("pdfium.dll")), "new dll");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unwritable_folders_are_reported() {
        assert!(writable(Path::new("/definitely/not/here")).is_err());
        let root = scratch("writable");
        writable(&root).unwrap();
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        let _ = fs::remove_dir_all(root);
    }
}
