use crate::ui::theme::ThemeChoice;
use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

const PREFERENCES_FILE: &str = "preferences.conf";
const SESSION_FILE: &str = "session.tsv";
const BOOKMARKS_FILE: &str = "bookmarks.tsv";
pub(crate) const MAX_SESSION_TABS: usize = 50;
pub(crate) const MAX_BOOKMARKED_DOCUMENTS: usize = 200;
pub(crate) const MAX_BOOKMARKS: usize = 500;
const BOOKMARK_TITLE_CHARACTERS: usize = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DefaultZoom {
    FitWidth,
    FitPage,
    Actual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PageLayout {
    Single,
    Continuous,
    TwoPages,
    CoverFirst,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NightPages {
    Never,
    FollowTheme,
    Always,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PageColour {
    Original,
    Night,
    Sepia,
}

impl NightPages {
    pub fn colour(self, dark_theme: bool) -> PageColour {
        match self {
            Self::Always => PageColour::Night,
            Self::FollowTheme if dark_theme => PageColour::Night,
            _ => PageColour::Original,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Preferences {
    pub reopen_tabs: bool,
    pub last_page: bool,
    pub default_zoom: DefaultZoom,
    pub autosave: bool,
    pub layout: PageLayout,
    pub night: NightPages,
    pub theme: ThemeChoice,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            reopen_tabs: false,
            last_page: true,
            default_zoom: DefaultZoom::FitWidth,
            autosave: false,
            layout: PageLayout::Continuous,
            night: NightPages::Never,
            theme: ThemeChoice::System,
        }
    }
}

fn switch(value: &str) -> Option<bool> {
    match value {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    }
}

fn write(directory: &Path, name: &str, contents: String) -> Result<()> {
    fs::create_dir_all(directory)
        .with_context(|| format!("Could not create {}", directory.display()))?;
    let path = directory.join(name);
    let staged = directory.join(format!("{name}.tmp"));
    fs::write(&staged, contents)
        .with_context(|| format!("Could not write {}", staged.display()))?;
    fs::rename(&staged, &path).with_context(|| format!("Could not replace {}", path.display()))
}

fn read(directory: &Path, name: &str) -> Result<Option<String>> {
    let path = directory.join(name);
    match fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("Could not read {}", path.display())),
    }
}

fn storable(path: &Path) -> Option<&str> {
    path.to_str()
        .filter(|text| !text.is_empty() && !text.contains(['\n', '\r', '\t']))
}

impl Preferences {
    fn parse(text: &str) -> Self {
        let mut preferences = Self::default();
        for (key, value) in text.lines().filter_map(|line| line.split_once('=')) {
            let value = value.trim();
            match key.trim() {
                "reopen_tabs" => {
                    preferences.reopen_tabs = switch(value).unwrap_or(preferences.reopen_tabs)
                }
                "last_page" => {
                    preferences.last_page = switch(value).unwrap_or(preferences.last_page)
                }
                "autosave" => preferences.autosave = switch(value).unwrap_or(preferences.autosave),
                "layout" => {
                    preferences.layout = match value {
                        "single" => PageLayout::Single,
                        "continuous" => PageLayout::Continuous,
                        "two_pages" => PageLayout::TwoPages,
                        "cover_first" => PageLayout::CoverFirst,
                        _ => preferences.layout,
                    }
                }
                "night" => {
                    preferences.night = match value {
                        "never" => NightPages::Never,
                        "follow_theme" => NightPages::FollowTheme,
                        "always" => NightPages::Always,
                        _ => preferences.night,
                    }
                }
                "default_zoom" => {
                    preferences.default_zoom = match value {
                        "fit_width" => DefaultZoom::FitWidth,
                        "fit_page" => DefaultZoom::FitPage,
                        "actual" => DefaultZoom::Actual,
                        _ => preferences.default_zoom,
                    }
                }
                "theme" => {
                    preferences.theme = match value {
                        "system" => ThemeChoice::System,
                        "light" => ThemeChoice::Light,
                        "dark" => ThemeChoice::Dark,
                        _ => preferences.theme,
                    }
                }
                _ => {}
            }
        }
        preferences
    }

    fn serialize(self) -> String {
        let switch = |on: bool| if on { "on" } else { "off" };
        format!(
            "reopen_tabs={}\nlast_page={}\nautosave={}\nlayout={}\nnight={}\ndefault_zoom={}\ntheme={}\n",
            switch(self.reopen_tabs),
            switch(self.last_page),
            switch(self.autosave),
            match self.layout {
                PageLayout::Single => "single",
                PageLayout::Continuous => "continuous",
                PageLayout::TwoPages => "two_pages",
                PageLayout::CoverFirst => "cover_first",
            },
            match self.night {
                NightPages::Never => "never",
                NightPages::FollowTheme => "follow_theme",
                NightPages::Always => "always",
            },
            match self.default_zoom {
                DefaultZoom::FitWidth => "fit_width",
                DefaultZoom::FitPage => "fit_page",
                DefaultZoom::Actual => "actual",
            },
            match self.theme {
                ThemeChoice::System => "system",
                ThemeChoice::Light => "light",
                ThemeChoice::Dark => "dark",
            }
        )
    }

    pub fn load(store: Option<&Path>) -> Result<Self> {
        Ok(match store {
            Some(store) => read(store, PREFERENCES_FILE)?
                .map(|text| Self::parse(&text))
                .unwrap_or_default(),
            None => Self::default(),
        })
    }

    pub fn save(self, store: Option<&Path>) -> Result<()> {
        match store {
            Some(store) => write(store, PREFERENCES_FILE, self.serialize()),
            None => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Session {
    pub paths: Vec<PathBuf>,
    pub active: usize,
}

impl Session {
    fn parse(text: &str) -> Self {
        let mut session = Self::default();
        for line in text.lines() {
            match line.split_once('\t') {
                Some(("active", index)) => session.active = index.parse().unwrap_or(0),
                Some(("tab", path))
                    if !path.is_empty() && session.paths.len() < MAX_SESSION_TABS =>
                {
                    session.paths.push(PathBuf::from(path));
                }
                _ => {}
            }
        }
        session.active = session.active.min(session.paths.len().saturating_sub(1));
        session
    }

    fn serialize(&self) -> String {
        let mut text = format!("active\t{}\n", self.active);
        for path in self.paths.iter().take(MAX_SESSION_TABS) {
            if let Some(path) = storable(path) {
                text.push_str(&format!("tab\t{path}\n"));
            }
        }
        text
    }

    pub fn load(store: Option<&Path>) -> Result<Self> {
        Ok(match store {
            Some(store) => read(store, SESSION_FILE)?
                .map(|text| Self::parse(&text))
                .unwrap_or_default(),
            None => Self::default(),
        })
    }

    pub fn save(&self, store: Option<&Path>) -> Result<()> {
        match store {
            Some(store) => write(store, SESSION_FILE, self.serialize()),
            None => Ok(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Bookmark {
    pub page: usize,
    pub title: String,
}

#[derive(Debug, Default)]
pub(crate) struct Bookmarks {
    documents: Vec<(PathBuf, Vec<Bookmark>)>,
}

pub(crate) fn bookmark_title(page: usize, text: Option<&str>) -> String {
    let line = text
        .into_iter()
        .flat_map(str::lines)
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .find(|line| line.chars().any(char::is_alphanumeric));
    match line {
        Some(line) if line.chars().count() > BOOKMARK_TITLE_CHARACTERS => {
            let mut short: String = line.chars().take(BOOKMARK_TITLE_CHARACTERS).collect();
            short.push('…');
            short
        }
        Some(line) => line,
        None => format!("Page {}", page + 1),
    }
}

impl Bookmarks {
    fn parse(text: &str) -> Self {
        let mut bookmarks = Self::default();
        for line in text.lines() {
            let mut fields = line.splitn(3, '\t');
            let (Some(page), Some(title), Some(path)) = (
                fields.next().and_then(|page| page.parse::<usize>().ok()),
                fields.next(),
                fields.next().filter(|path| !path.is_empty()),
            ) else {
                continue;
            };
            let path = PathBuf::from(path);
            let index = match bookmarks
                .documents
                .iter()
                .position(|(known, _)| *known == path)
            {
                Some(index) => index,
                None if bookmarks.documents.len() < MAX_BOOKMARKED_DOCUMENTS => {
                    bookmarks.documents.push((path, Vec::new()));
                    bookmarks.documents.len() - 1
                }
                None => continue,
            };
            let pages = &mut bookmarks.documents[index].1;
            if pages.len() < MAX_BOOKMARKS && pages.iter().all(|bookmark| bookmark.page != page) {
                pages.push(Bookmark {
                    page,
                    title: title.to_string(),
                });
            }
        }
        for (_, pages) in &mut bookmarks.documents {
            pages.sort_by_key(|bookmark| bookmark.page);
        }
        bookmarks
    }

    fn serialize(&self) -> String {
        let mut text = String::new();
        for (path, pages) in &self.documents {
            let Some(path) = storable(path) else {
                continue;
            };
            for bookmark in pages {
                let title = bookmark.title.replace(['\n', '\r', '\t'], " ");
                text.push_str(&format!("{}\t{title}\t{path}\n", bookmark.page));
            }
        }
        text
    }

    pub fn load(store: Option<&Path>) -> Result<Self> {
        Ok(match store {
            Some(store) => read(store, BOOKMARKS_FILE)?
                .map(|text| Self::parse(&text))
                .unwrap_or_default(),
            None => Self::default(),
        })
    }

    pub fn save(&self, store: Option<&Path>) -> Result<()> {
        match store {
            Some(store) => write(store, BOOKMARKS_FILE, self.serialize()),
            None => Ok(()),
        }
    }

    pub fn of(&self, path: &Path) -> &[Bookmark] {
        self.documents
            .iter()
            .find(|(known, _)| known == path)
            .map_or(&[], |(_, pages)| pages)
    }

    pub fn toggle(&mut self, path: &Path, page: usize, title: String) -> Result<bool, usize> {
        let index = self.documents.iter().position(|(known, _)| known == path);
        let mut document = match index {
            Some(index) => self.documents.remove(index),
            None => (path.to_path_buf(), Vec::new()),
        };
        let added = match document.1.iter().position(|bookmark| bookmark.page == page) {
            Some(existing) => {
                document.1.remove(existing);
                Ok(false)
            }
            None if document.1.len() >= MAX_BOOKMARKS => Err(MAX_BOOKMARKS),
            None => {
                document.1.push(Bookmark { page, title });
                document.1.sort_by_key(|bookmark| bookmark.page);
                Ok(true)
            }
        };
        if !document.1.is_empty() {
            self.documents.insert(0, document);
            self.documents.truncate(MAX_BOOKMARKED_DOCUMENTS);
        }
        added
    }
}

pub(crate) fn fit_page_zoom(available: (f32, f32), page_points: (f32, f32)) -> Option<u32> {
    let (width, height) = (page_points.0 * 96.0 / 72.0, page_points.1 * 96.0 / 72.0);
    if width <= 0.0 || height <= 0.0 || available.0 <= 0.0 || available.1 <= 0.0 {
        return None;
    }
    let scale = (available.0 / width).min(available.1 / height);
    scale
        .is_finite()
        .then(|| ((scale * 100.0).round() as u32).clamp(25, 400))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("agenticpdf-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        directory
    }

    #[test]
    fn preferences_round_trip_and_ignore_what_they_do_not_know() {
        let directory = store("preferences");
        assert_eq!(
            Preferences::load(Some(&directory)).unwrap(),
            Preferences::default()
        );
        let chosen = Preferences {
            reopen_tabs: true,
            last_page: false,
            default_zoom: DefaultZoom::FitPage,
            autosave: true,
            layout: PageLayout::CoverFirst,
            night: NightPages::FollowTheme,
            theme: ThemeChoice::Dark,
        };
        chosen.save(Some(&directory)).unwrap();
        assert_eq!(Preferences::load(Some(&directory)).unwrap(), chosen);
        assert_eq!(
            Preferences::parse("autosave=maybe\nfuture=1\nno equals\ndefault_zoom=actual\n"),
            Preferences {
                default_zoom: DefaultZoom::Actual,
                ..Preferences::default()
            }
        );
        assert_eq!(Preferences::load(None).unwrap(), Preferences::default());
        chosen.save(None).unwrap();
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn night_pages_follow_the_setting_and_the_theme() {
        assert_eq!(NightPages::Never.colour(true), PageColour::Original);
        assert_eq!(NightPages::FollowTheme.colour(true), PageColour::Night);
        assert_eq!(NightPages::FollowTheme.colour(false), PageColour::Original);
        assert_eq!(NightPages::Always.colour(false), PageColour::Night);
    }

    #[test]
    fn sessions_round_trip_clamp_the_active_tab_and_stay_bounded() {
        let directory = store("session");
        let session = Session {
            paths: vec![PathBuf::from("/docs/a.pdf"), PathBuf::from("/docs/b c.pdf")],
            active: 1,
        };
        session.save(Some(&directory)).unwrap();
        assert_eq!(Session::load(Some(&directory)).unwrap(), session);
        assert_eq!(Session::parse("active\t9\ntab\t/x.pdf\nnoise\n").active, 0);
        assert_eq!(Session::parse("").paths.len(), 0);
        let many: String = (0..MAX_SESSION_TABS + 5)
            .map(|index| format!("tab\t/docs/{index}.pdf\n"))
            .collect();
        assert_eq!(Session::parse(&many).paths.len(), MAX_SESSION_TABS);
        let odd = Session {
            paths: vec![
                PathBuf::from("/docs/tab\there.pdf"),
                PathBuf::from("/ok.pdf"),
            ],
            active: 0,
        };
        assert_eq!(
            Session::parse(&odd.serialize()).paths,
            [PathBuf::from("/ok.pdf")]
        );
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn bookmarks_toggle_sort_persist_and_stay_bounded() {
        let directory = store("bookmarks");
        let (first, second) = (PathBuf::from("/docs/a.pdf"), PathBuf::from("/docs/b.pdf"));
        let mut bookmarks = Bookmarks::default();
        assert_eq!(bookmarks.toggle(&first, 4, "Later".into()), Ok(true));
        assert_eq!(bookmarks.toggle(&first, 1, "Tab\there".into()), Ok(true));
        assert_eq!(bookmarks.toggle(&second, 0, "Cover".into()), Ok(true));
        assert_eq!(
            bookmarks
                .of(&first)
                .iter()
                .map(|bookmark| bookmark.page)
                .collect::<Vec<_>>(),
            [1, 4]
        );
        bookmarks.save(Some(&directory)).unwrap();
        let loaded = Bookmarks::load(Some(&directory)).unwrap();
        assert_eq!(loaded.of(&first).len(), 2);
        assert_eq!(loaded.of(&first)[1], bookmarks.of(&first)[1]);
        assert_eq!(loaded.of(&first)[0].title, "Tab here");
        assert_eq!(loaded.of(&second).len(), 1);
        assert_eq!(bookmarks.toggle(&second, 0, String::new()), Ok(false));
        assert!(bookmarks.of(&second).is_empty());
        assert_eq!(bookmarks.documents.len(), 1);
        for page in 0..MAX_BOOKMARKS {
            let _ = bookmarks.toggle(&second, page, String::new());
        }
        assert_eq!(
            bookmarks.toggle(&second, MAX_BOOKMARKS, String::new()),
            Err(MAX_BOOKMARKS)
        );
        assert_eq!(bookmarks.of(&second).len(), MAX_BOOKMARKS);
        assert!(
            Bookmarks::parse("x\ty\t/a.pdf\n3\tOnly title\n")
                .documents
                .is_empty()
        );
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn bookmark_titles_come_from_the_first_line_with_words() {
        assert_eq!(bookmark_title(2, None), "Page 3");
        assert_eq!(bookmark_title(0, Some("  \n ---\n")), "Page 1");
        assert_eq!(
            bookmark_title(0, Some("\n  Network   capacity\nPeak traffic")),
            "Network capacity"
        );
        let long = bookmark_title(0, Some(&"word ".repeat(40)));
        assert_eq!(long.chars().count(), BOOKMARK_TITLE_CHARACTERS + 1);
    }

    #[test]
    fn fit_page_uses_the_tighter_of_width_and_height() {
        assert_eq!(fit_page_zoom((816.0, 2000.0), (612.0, 792.0)), Some(100));
        assert_eq!(fit_page_zoom((2000.0, 528.0), (612.0, 792.0)), Some(50));
        assert_eq!(fit_page_zoom((10.0, 10.0), (612.0, 792.0)), Some(25));
        assert_eq!(fit_page_zoom((90000.0, 90000.0), (612.0, 792.0)), Some(400));
        assert_eq!(fit_page_zoom((800.0, 0.0), (612.0, 792.0)), None);
        assert_eq!(fit_page_zoom((800.0, 600.0), (0.0, 792.0)), None);
    }
}
