use super::Reader;
use crate::pdf::{
    Event, SearchJob,
    search::{Hit, MAX_HITS, Query},
};
use gpui::{Context, Window, point, px};
use std::{ops::Range, time::Duration};

pub(super) const SEARCH_DELAY: Duration = Duration::from_millis(150);
const REVEAL_MARGIN: f32 = 28.0;

pub(super) struct SearchState {
    pub id: u64,
    pub session: u64,
    pub query: Query,
    pub hits: Vec<(usize, Hit)>,
    pub current: Option<usize>,
    pub scanned: usize,
    pub done: bool,
    pub skipped: usize,
    pub truncated: bool,
    pub error: Option<String>,
}

impl SearchState {
    pub fn page_hits(&self, page: usize) -> Range<usize> {
        let start = self.hits.partition_point(|(hit_page, _)| *hit_page < page);
        let end = self.hits.partition_point(|(hit_page, _)| *hit_page <= page);
        start..end
    }

    pub fn summary_total(&self) -> String {
        if self.truncated {
            format!("{MAX_HITS}+")
        } else {
            self.hits.len().to_string()
        }
    }

    pub fn summary(&self) -> String {
        if self.error.is_some() {
            return "Search failed".into();
        }
        let total = self.summary_total();
        match (self.current, self.hits.is_empty(), self.done) {
            (_, true, false) => "Searching".into(),
            (_, true, true) => "No matches".into(),
            (Some(index), false, _) => format!("{} of {total}", index + 1),
            (None, false, _) => {
                if self.hits.len() == 1 && !self.truncated {
                    "1 match".into()
                } else {
                    format!("{total} matches")
                }
            }
        }
    }

    fn next_from(&self, page: usize, forward: bool) -> Option<usize> {
        if self.hits.is_empty() {
            return None;
        }
        let count = self.hits.len();
        Some(match (self.current, forward) {
            (Some(index), true) => (index + 1) % count,
            (Some(index), false) => (index + count - 1) % count,
            (None, true) => {
                let start = self.hits.partition_point(|(hit_page, _)| *hit_page < page);
                if start == count { 0 } else { start }
            }
            (None, false) => {
                let end = self.hits.partition_point(|(hit_page, _)| *hit_page <= page);
                if end == 0 { count - 1 } else { end - 1 }
            }
        })
    }
}

impl Reader {
    pub(super) fn search_query(&self) -> Option<Query> {
        let text = self.chrome.find.text.trim();
        (!text.is_empty()).then(|| Query {
            text: text.to_string(),
            match_case: self.chrome.find_case,
            whole_word: self.chrome.find_word,
        })
    }

    pub(super) fn schedule_search(&mut self, cx: &mut Context<Self>) {
        self.chrome.find_edits += 1;
        let edit = self.chrome.find_edits;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SEARCH_DELAY).await;
            let _ = this.update(cx, |this, cx| {
                if this.chrome.find_edits == edit {
                    this.restart_search(cx);
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn restart_search(&mut self, cx: &mut Context<Self>) {
        let query = self.search_query().filter(|_| self.chrome.find_open);
        let Some(tab) = self.tabs.current() else {
            self.clear_search();
            return;
        };
        let Some(query) = query else {
            self.clear_search();
            cx.notify();
            return;
        };
        if self
            .search
            .as_ref()
            .is_some_and(|state| state.session == tab.session && state.query == query)
        {
            return;
        }
        let session = tab.session;
        let id = self.worker.search(SearchJob {
            search: 0,
            session,
            path: tab.path.clone(),
            query: query.clone(),
        });
        self.search = Some(SearchState {
            id,
            session,
            query,
            hits: Vec::new(),
            current: None,
            scanned: 0,
            done: false,
            skipped: 0,
            truncated: false,
            error: None,
        });
        self.reveal = false;
        cx.notify();
    }

    pub(super) fn clear_search(&mut self) {
        if self.search.take().is_some() {
            self.worker.cancel_search();
        }
        self.reveal = false;
    }

    pub(super) fn receive_search(
        &mut self,
        event: Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page = self.tabs.current().map_or(0, |tab| tab.page);
        let Some(state) = self.search.as_mut() else {
            return;
        };
        let mut select = None;
        match event {
            Event::SearchBatch {
                search,
                scanned,
                pages,
            } if search == state.id => {
                state.scanned = scanned;
                for (hit_page, hits) in pages {
                    for hit in hits {
                        if state.current.is_none() && select.is_none() && hit_page >= page {
                            select = Some(state.hits.len());
                        }
                        state.hits.push((hit_page, hit));
                    }
                }
            }
            Event::SearchDone {
                search,
                skipped,
                truncated,
                ..
            } if search == state.id => {
                state.done = true;
                state.skipped = skipped;
                state.truncated = truncated;
                if state.current.is_none() && !state.hits.is_empty() {
                    select = Some(0);
                }
            }
            Event::SearchFailed { search, message } if search == state.id => {
                state.done = true;
                state.error = Some(message);
            }
            _ => return,
        }
        if let Some(index) = select {
            self.show_match(index, window, cx);
        }
        cx.notify();
    }

    pub(super) fn step_match(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page = self.tabs.current().map_or(0, |tab| tab.page);
        if let Some(index) = self
            .search
            .as_ref()
            .and_then(|state| state.next_from(page, forward))
        {
            self.show_match(index, window, cx);
        }
    }

    pub(super) fn show_match(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.search.as_mut() else {
            return;
        };
        let Some((page, _)) = state.hits.get(index) else {
            return;
        };
        let page = *page;
        state.current = Some(index);
        self.reveal = true;
        self.go(page, window, cx);
        self.reveal_match();
        cx.notify();
    }

    pub(super) fn reveal_match(&mut self) {
        if !self.reveal {
            return;
        }
        let page_top = self.page_top();
        let (Some(state), Some(tab), Some(Ok(text)), Some(image)) = (
            self.search.as_ref(),
            self.tabs.current(),
            self.text.as_ref(),
            self.image.as_ref(),
        ) else {
            return;
        };
        if self
            .image_key
            .as_ref()
            .is_none_or(|key| key.page != tab.page)
        {
            return;
        }
        let Some((page, hit)) = state.current.and_then(|index| state.hits.get(index)) else {
            self.reveal = false;
            return;
        };
        if *page != tab.page {
            return;
        }
        let Some(top) = text
            .glyphs
            .get(hit.glyphs.clone())
            .into_iter()
            .flatten()
            .filter_map(|glyph| glyph.bounds)
            .map(|bounds| bounds.top)
            .reduce(f32::min)
        else {
            self.reveal = false;
            return;
        };
        let viewport = f32::from(tab.scroll.bounds().size.height);
        let target = (REVEAL_MARGIN + page_top + top * image.height - viewport / 3.0).max(0.0);
        let offset = tab.scroll.offset();
        tab.scroll.set_offset(point(offset.x, px(-target)));
        self.reveal = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(start: usize) -> Hit {
        Hit {
            glyphs: start..start + 1,
            before: String::new(),
            matched: "a".into(),
            after: String::new(),
        }
    }

    fn state(pages: &[usize], current: Option<usize>, done: bool) -> SearchState {
        SearchState {
            id: 1,
            session: 1,
            query: Query {
                text: "a".into(),
                match_case: false,
                whole_word: false,
            },
            hits: pages.iter().map(|page| (*page, hit(*page))).collect(),
            current,
            scanned: 0,
            done,
            skipped: 0,
            truncated: false,
            error: None,
        }
    }

    #[test]
    fn stepping_starts_from_the_current_page_and_wraps() {
        let search = state(&[1, 3, 3, 7], None, true);
        assert_eq!(search.next_from(2, true), Some(1));
        assert_eq!(search.next_from(8, true), Some(0));
        assert_eq!(search.next_from(3, false), Some(2));
        assert_eq!(search.next_from(0, false), Some(3));
        let selected = state(&[1, 3, 3, 7], Some(3), true);
        assert_eq!(selected.next_from(0, true), Some(0));
        assert_eq!(selected.next_from(0, false), Some(2));
        assert_eq!(state(&[], None, true).next_from(0, true), None);
    }

    #[test]
    fn page_hits_selects_only_that_page() {
        let search = state(&[1, 3, 3, 7], None, true);
        assert_eq!(search.page_hits(3), 1..3);
        assert_eq!(search.page_hits(4), 3..3);
    }

    #[test]
    fn summary_reports_progress_position_and_absence() {
        assert_eq!(state(&[], None, false).summary(), "Searching");
        assert_eq!(state(&[], None, true).summary(), "No matches");
        assert_eq!(state(&[2], None, true).summary(), "1 match");
        assert_eq!(state(&[2, 4], Some(1), true).summary(), "2 of 2");
        let mut capped = state(&[2, 4], Some(0), true);
        capped.truncated = true;
        assert_eq!(capped.summary(), format!("1 of {MAX_HITS}+"));
    }
}
