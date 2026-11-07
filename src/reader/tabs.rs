use super::jumps::Jumps;
use gpui::{Pixels, Point, ScrollHandle, UniformListScrollHandle};
use std::{collections::BTreeSet, ops::Range, path::PathBuf};

pub(super) struct DocumentTab {
    pub session: u64,
    pub path: PathBuf,
    pub page: usize,
    pub count: usize,
    pub zoom: Option<u32>,
    pub visible: Range<usize>,
    pub scroll: ScrollHandle,
    pub restore_scroll: Option<Point<Pixels>>,
    pub sidebar_scroll: UniformListScrollHandle,
    pub failed_thumbnails: BTreeSet<usize>,
    pub error: Option<String>,
    pub jumps: Jumps,
    pub pending_anchor: Option<(f32, f32)>,
}

impl DocumentTab {
    pub fn new(session: u64, path: PathBuf) -> Self {
        Self {
            session,
            path,
            page: 0,
            count: 0,
            zoom: None,
            visible: 0..4,
            scroll: ScrollHandle::new(),
            restore_scroll: None,
            sidebar_scroll: UniformListScrollHandle::new(),
            failed_thumbnails: BTreeSet::new(),
            error: None,
            jumps: Jumps::default(),
            pending_anchor: None,
        }
    }

    pub fn title(&self) -> String {
        self.path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }
}

#[derive(Default)]
pub(super) struct Tabs {
    pub entries: Vec<DocumentTab>,
    pub active: usize,
    next_session: u64,
}

impl Tabs {
    pub fn current(&self) -> Option<&DocumentTab> {
        self.entries.get(self.active)
    }
    pub fn current_mut(&mut self) -> Option<&mut DocumentTab> {
        self.entries.get_mut(self.active)
    }

    pub fn open(&mut self, path: PathBuf) -> usize {
        let path = path.canonicalize().unwrap_or(path);
        if let Some(index) = self.entries.iter().position(|tab| tab.path == path) {
            return index;
        }
        self.next_session += 1;
        self.entries.push(DocumentTab::new(self.next_session, path));
        self.entries.len() - 1
    }

    pub fn close(&mut self, index: usize) -> Option<DocumentTab> {
        if index >= self.entries.len() {
            return None;
        }
        let tab = self.entries.remove(index);
        if index < self.active {
            self.active -= 1;
        }
        self.active = self.active.min(self.entries.len().saturating_sub(1));
        Some(tab)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_paths_reuse_tabs_and_closing_preserves_active_identity() {
        let mut tabs = Tabs::default();
        let first = tabs.open(PathBuf::from("first.pdf"));
        let second = tabs.open(PathBuf::from("second.pdf"));
        assert_eq!(tabs.open(PathBuf::from("first.pdf")), first);
        tabs.active = second;
        tabs.current_mut().unwrap().page = 7;
        let session = tabs.current().unwrap().session;
        tabs.close(first);
        assert_eq!(tabs.current().unwrap().session, session);
        assert_eq!(tabs.current().unwrap().page, 7);
        tabs.close(0);
        assert!(tabs.current().is_none());
        tabs.open(PathBuf::from("first.pdf"));
        assert!(tabs.entries[0].session > session);
    }
}
