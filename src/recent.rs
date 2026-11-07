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

fn data_dir() -> Option<PathBuf> {
    let name = if cfg!(debug_assertions) {
        "PDF Reader Debug"
    } else {
        "PDF Reader"
    };
    if cfg!(target_os = "macos") {
        let identifier = if cfg!(debug_assertions) {
            "org.pdfreader.desktop.debug"
        } else {
            "org.pdfreader.desktop"
        };
        std::env::var_os("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library/Application Support")
                .join(identifier)
        })
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(|data| PathBuf::from(data).join(name))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|config| config.join(name.to_lowercase().replace(' ', "-")))
    }
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
            std::env::temp_dir().join(format!("pdf-reader-recent-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        directory.join("nested").join(FILE_NAME)
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
