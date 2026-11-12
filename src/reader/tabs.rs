use super::column::Column;
use super::{jumps::Jumps, outline::OutlineTree};
use crate::pdf::{PageSizes, markup::Markup};
use gpui::{Pixels, Point, ScrollHandle, UniformListScrollHandle};
use std::rc::Rc;
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
    pub sizes: Option<PageSizes>,
    pub column: Option<Rc<Column>>,
    pub follow_scroll: bool,
    pub pending_scroll: bool,
    pub jumps: Jumps,
    pub pending_anchor: Option<(f32, f32)>,
    pub outline: Option<OutlineTree>,
    pub outline_scroll: ScrollHandle,
    pub markups: Vec<Markup>,
    pub saved: usize,
    pub undone: Vec<Markup>,
    pub saving: bool,
    pub close_after_save: bool,
    pub notes_scroll: ScrollHandle,
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
            sizes: None,
            column: None,
            follow_scroll: false,
            pending_scroll: false,
            jumps: Jumps::default(),
            pending_anchor: None,
            outline: None,
            outline_scroll: ScrollHandle::new(),
            markups: Vec::new(),
            saved: 0,
            undone: Vec::new(),
            saving: false,
            close_after_save: false,
            notes_scroll: ScrollHandle::new(),
        }
    }

    pub fn unsaved(&self) -> &[Markup] {
        self.markups.get(self.saved..).unwrap_or_default()
    }

    pub fn modified(&self) -> bool {
        !self.unsaved().is_empty()
    }

    pub fn discard(&mut self) {
        self.markups.truncate(self.saved);
        self.undone.clear();
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
