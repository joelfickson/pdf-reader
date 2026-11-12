use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) const MAX_RECENT: usize = 20;
const FILE_NAME: &str = "recent.tsv";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RecentFile {
    pub path: PathBuf,
    pub page: usize,
    pub zoom: Option<u32>,
}

#[derive(Debug, Default)]
pub(crate) struct RecentFiles {
    entries: Vec<RecentFile>,
    store: Option<PathBuf>,
}

struct Home {
    name: &'static str,
    identifier: &'static str,
    folder: &'static str,
}

const HOME: Home = if cfg!(debug_assertions) {
    Home {
        name: "Agentic PDF Debug",
        identifier: "app.agenticpdf.desktop.debug",
        folder: "agenticpdf-debug",
    }
} else {
    Home {
        name: "Agentic PDF",
        identifier: "app.agenticpdf.desktop",
        folder: "agenticpdf",
    }
};

const FORMER_HOME: Home = if cfg!(debug_assertions) {
    Home {
        name: "PDF Reader Debug",
        identifier: "org.pdfreader.desktop.debug",
        folder: "pdf-reader-debug",
    }
} else {
    Home {
        name: "PDF Reader",
        identifier: "org.pdfreader.desktop",
        folder: "pdf-reader",
    }
};

const CARRIED_FILES: [&str; 2] = ["recent.tsv", "updates.conf"];

fn location(home: &Home) -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|user| {
            PathBuf::from(user)
                .join("Library/Application Support")
                .join(home.identifier)
        })
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(|data| PathBuf::from(data).join(home.name))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|user| PathBuf::from(user).join(".config")))
            .map(|config| config.join(home.folder))
    }
}

pub(crate) fn data_dir() -> Option<PathBuf> {
    location(&HOME)
}

pub(crate) fn carry_over_former_data() -> Result<()> {
    if cfg!(test) {
        return Ok(());
    }
    match (location(&FORMER_HOME), data_dir()) {
        (Some(former), Some(current)) => carry_over(&former, &current),
        _ => Ok(()),
    }
}

fn carry_over(former: &Path, current: &Path) -> Result<()> {
    if current.exists() || !former.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(current)
        .with_context(|| format!("Could not create {}", current.display()))?;
    for name in CARRIED_FILES {
        let source = former.join(name);
        if source.is_file() {
            fs::copy(&source, current.join(name))
                .with_context(|| format!("Could not copy {}", source.display()))?;
        }
    }
    Ok(())
}

fn parse(text: &str) -> Vec<RecentFile> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.splitn(3, '\t');
            let page = fields.next()?.parse().ok()?;
            let zoom = match fields.next()? {
                "" => None,
                zoom => Some(zoom.parse().ok()?),
            };
            let path = fields.next().filter(|path| !path.is_empty())?;
            Some(RecentFile {
                path: PathBuf::from(path),
                page,
                zoom,
            })
        })
        .take(MAX_RECENT)
        .collect()
}

fn serialize(entries: &[RecentFile]) -> String {
    entries
        .iter()
        .filter_map(|entry| {
            let path = entry.path.to_str()?;
            (!path.contains(['\n', '\r', '\t'])).then(|| {
                format!(
                    "{}\t{}\t{path}\n",
                    entry.page,
                    entry.zoom.map(|zoom| zoom.to_string()).unwrap_or_default()
                )
            })
        })
        .collect()
}

impl RecentFiles {
    pub fn load() -> Result<Self> {
        if cfg!(test) {
            return Ok(Self::default());
        }
        let Some(directory) = data_dir() else {
            return Ok(Self::default());
        };
        Self::load_from(directory.join(FILE_NAME))
    }

    pub fn load_from(store: PathBuf) -> Result<Self> {
        let entries = match fs::read_to_string(&store) {
            Ok(text) => parse(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Could not read recent files from {}", store.display())
                });
            }
        };
        Ok(Self {
            entries,
            store: Some(store),
        })
    }

    pub fn entries(&self) -> &[RecentFile] {
        &self.entries
    }

    pub fn get(&self, path: &Path) -> Option<&RecentFile> {
        self.entries.iter().find(|entry| entry.path == path)
    }

    pub fn touch(&mut self, path: &Path) {
        let entry = match self.entries.iter().position(|entry| entry.path == path) {
            Some(index) => self.entries.remove(index),
            None => RecentFile {
                path: path.to_path_buf(),
                page: 0,
                zoom: None,
            },
        };
        self.entries.insert(0, entry);
        self.entries.truncate(MAX_RECENT);
    }

    pub fn update(&mut self, path: &Path, page: usize, zoom: Option<u32>) -> bool {
        match self.entries.iter_mut().find(|entry| entry.path == path) {
            Some(entry) if entry.page != page || entry.zoom != zoom => {
                entry.page = page;
                entry.zoom = zoom;
                true
            }
            _ => false,
        }
    }

    pub fn remove(&mut self, path: &Path) {
        self.entries.retain(|entry| entry.path != path);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn save(&self) -> Result<()> {
        let Some(store) = &self.store else {
            return Ok(());
        };
        let directory = store
            .parent()
            .context("The recent files location has no parent folder")?;
        fs::create_dir_all(directory)
            .with_context(|| format!("Could not create {}", directory.display()))?;
        let staged = store.with_extension("tsv.tmp");
        fs::write(&staged, serialize(&self.entries))
            .with_context(|| format!("Could not write {}", staged.display()))?;
        fs::rename(&staged, store)
            .with_context(|| format!("Could not replace {}", store.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("agenticpdf-recent-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        directory.join("nested").join(FILE_NAME)
    }

    #[test]
    fn former_settings_are_copied_once_and_never_overwrite_new_ones() {
        let root = std::env::temp_dir().join(format!("agenticpdf-carry-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (former, current) = (root.join("former"), root.join("current"));
        carry_over(&former, &current).unwrap();
        assert!(!current.exists());
        fs::create_dir_all(&former).unwrap();
        fs::write(former.join("recent.tsv"), "old recent").unwrap();
        fs::write(former.join("updates.conf"), "old updates").unwrap();
        fs::write(former.join("unrelated.txt"), "left behind").unwrap();
        carry_over(&former, &current).unwrap();
        assert_eq!(
            fs::read_to_string(current.join("recent.tsv")).unwrap(),
            "old recent"
        );
        assert_eq!(
            fs::read_to_string(current.join("updates.conf")).unwrap(),
            "old updates"
        );
        assert!(!current.join("unrelated.txt").exists());
        assert!(former.join("recent.tsv").exists());
        fs::write(current.join("recent.tsv"), "new recent").unwrap();
        fs::write(former.join("recent.tsv"), "changed later").unwrap();
        carry_over(&former, &current).unwrap();
        assert_eq!(
            fs::read_to_string(current.join("recent.tsv")).unwrap(),
            "new recent"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn touch_moves_to_front_deduplicates_and_caps() {
        let mut recent = RecentFiles::default();
        for index in 0..MAX_RECENT + 5 {
            recent.touch(Path::new(&format!("/docs/{index}.pdf")));
        }
        recent.touch(Path::new("/docs/10.pdf"));
        assert_eq!(recent.entries().len(), MAX_RECENT);
        assert_eq!(recent.entries()[0].path, Path::new("/docs/10.pdf"));
        assert_eq!(
            recent
                .entries()
                .iter()
                .filter(|entry| entry.path == Path::new("/docs/10.pdf"))
                .count(),
            1
        );
        assert!(recent.get(Path::new("/docs/0.pdf")).is_none());
    }

    #[test]
    fn update_reports_changes_only_for_known_files() {
        let mut recent = RecentFiles::default();
        assert!(!recent.update(Path::new("/a.pdf"), 3, None));
        recent.touch(Path::new("/a.pdf"));
        assert!(recent.update(Path::new("/a.pdf"), 3, Some(150)));
        assert!(!recent.update(Path::new("/a.pdf"), 3, Some(150)));
        assert_eq!(
            recent.get(Path::new("/a.pdf")),
            Some(&RecentFile {
                path: PathBuf::from("/a.pdf"),
                page: 3,
                zoom: Some(150),
            })
        );
    }

    #[test]
    fn saves_and_loads_round_trip_with_unicode_paths() {
        let store = temp_store("round-trip");
        let mut recent = RecentFiles::load_from(store.clone()).unwrap();
        assert!(recent.entries().is_empty());
        recent.touch(Path::new("/docs/Café notes.pdf"));
        recent.update(Path::new("/docs/Café notes.pdf"), 41, Some(125));
        recent.touch(Path::new("/docs/plain.pdf"));
        recent.save().unwrap();
        let loaded = RecentFiles::load_from(store.clone()).unwrap();
        assert_eq!(loaded.entries(), recent.entries());
        assert!(!store.with_extension("tsv.tmp").exists());
        let _ = fs::remove_dir_all(store.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn parse_skips_malformed_lines_and_unsafe_paths_are_not_written() {
        let entries = parse(
            "2\t\t/ok.pdf\nnot a line\nx\t\t/bad-page.pdf\n3\tzoom\t/bad-zoom.pdf\n4\t90\t\n5\t110\t/also ok.pdf\n",
        );
        assert_eq!(
            entries,
            [
                RecentFile {
                    path: PathBuf::from("/ok.pdf"),
                    page: 2,
                    zoom: None
                },
                RecentFile {
                    path: PathBuf::from("/also ok.pdf"),
                    page: 5,
                    zoom: Some(110)
                },
            ]
        );
        let unsafe_entry = RecentFile {
            path: PathBuf::from("/line\nbreak.pdf"),
            page: 0,
            zoom: None,
        };
        assert_eq!(serialize(&[unsafe_entry]), "");
    }

    #[test]
    fn unreadable_store_is_an_error() {
        let store = temp_store("unreadable");
        fs::create_dir_all(&store).unwrap();
        assert!(RecentFiles::load_from(store.clone()).is_err());
        let _ = fs::remove_dir_all(store.parent().unwrap().parent().unwrap());
    }
}
