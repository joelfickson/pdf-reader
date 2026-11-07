use super::{Reader, actions::*};
use crate::pdf::text::{Link, LinkTarget};
use crate::ui::kit::Status;
use gpui::{Context, Pixels, Point, ScrollStrategy, Window, point, px};

const MAX_JUMPS: usize = 100;
const PAGE_TOP: f32 = 28.0;
const ANCHOR_MARGIN: f32 = 16.0;
const OPENABLE_SCHEMES: [&str; 3] = ["http", "https", "mailto"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Place {
    pub page: usize,
    pub offset: Point<Pixels>,
}

#[derive(Default)]
pub(super) struct Jumps {
    back: Vec<Place>,
    forward: Vec<Place>,
}

impl Jumps {
    pub fn record(&mut self, from: Place) {
        self.forward.clear();
        if self.back.last() != Some(&from) {
            self.back.push(from);
        }
        if self.back.len() > MAX_JUMPS {
            self.back.remove(0);
        }
    }

    pub fn back(&mut self, current: Place) -> Option<Place> {
        let target = self.back.pop()?;
        self.forward.push(current);
        Some(target)
    }

    pub fn forward(&mut self, current: Place) -> Option<Place> {
        let target = self.forward.pop()?;
        self.back.push(current);
        Some(target)
    }
}

pub(super) fn openable(uri: &str) -> bool {
    uri.split_once(':').is_some_and(|(scheme, rest)| {
        !rest.is_empty()
            && OPENABLE_SCHEMES
                .iter()
                .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
    })
}

impl Reader {
    fn place(&self) -> Option<Place> {
        let tab = self.tabs.current()?;
        Some(Place {
            page: tab.page,
            offset: tab.restore_scroll.unwrap_or_else(|| tab.scroll.offset()),
        })
    }

    pub(super) fn jump(
        &mut self,
        page: usize,
        anchor: Option<(f32, f32)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(from) = self.place() else {
            return;
        };
        let Some(tab) = self.tabs.current_mut() else {
            return;
        };
        if page >= tab.count || (page == tab.page && anchor.is_none()) {
            return;
        }
        tab.jumps.record(from);
        tab.pending_anchor = anchor;
        if page == tab.page {
            self.reveal_anchor();
            cx.notify();
        } else {
            self.go(page, window, cx);
        }
    }

    fn return_to(&mut self, place: Place, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.current_mut() else {
            return;
        };
        tab.pending_anchor = None;
        if place.page == tab.page {
            tab.scroll.set_offset(place.offset);
            cx.notify();
            return;
        }
        if place.page >= tab.count {
            return;
        }
        self.reset_request(window);
        if let Some(tab) = self.tabs.current_mut() {
            tab.page = place.page;
            tab.error = None;
            tab.scroll.set_offset(point(px(0.0), px(0.0)));
            tab.restore_scroll = Some(place.offset);
            tab.sidebar_scroll
                .scroll_to_item(place.page, ScrollStrategy::Center);
        }
        self.request(window);
        cx.notify();
    }

    pub(super) fn go_back(&mut self, _: &GoBack, window: &mut Window, cx: &mut Context<Self>) {
        let Some(current) = self.place() else {
            return;
        };
        if let Some(target) = self
            .tabs
            .current_mut()
            .and_then(|tab| tab.jumps.back(current))
        {
            self.return_to(target, window, cx);
        }
    }

    pub(super) fn go_forward(
        &mut self,
        _: &GoForward,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(current) = self.place() else {
            return;
        };
        if let Some(target) = self
            .tabs
            .current_mut()
            .and_then(|tab| tab.jumps.forward(current))
        {
            self.return_to(target, window, cx);
        }
    }

    pub(super) fn reveal_anchor(&mut self) {
        let (Some(tab), Some(Ok(text)), Some(image), Some(key)) = (
            self.tabs.current_mut(),
            self.text.as_ref(),
            self.image.as_ref(),
            self.image_key.as_ref(),
        ) else {
            return;
        };
        if key.page != tab.page {
            return;
        }
        let Some(anchor) = tab.pending_anchor.take() else {
            return;
        };
        if let Some(top) = text.anchor_top(anchor) {
            let target = (PAGE_TOP + top * image.height - ANCHOR_MARGIN).max(0.0);
            let offset = tab.scroll.offset();
            tab.scroll.set_offset(point(offset.x, px(-target)));
        }
    }

    pub(super) fn link_at(&self, position: Point<Pixels>) -> Option<Link> {
        let Some(Ok(text)) = &self.text else {
            return None;
        };
        let bounds = self.page_bounds.get();
        if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
            return None;
        }
        let x = f32::from(position.x - bounds.origin.x) / f32::from(bounds.size.width);
        let y = f32::from(position.y - bounds.origin.y) / f32::from(bounds.size.height);
        text.link_at(x, y).cloned()
    }

    pub(super) fn follow_link(&mut self, link: Link, window: &mut Window, cx: &mut Context<Self>) {
        match link.target {
            LinkTarget::Page { index, anchor } => {
                let count = self.tabs.current().map_or(0, |tab| tab.count);
                if index < count {
                    self.jump(index, anchor, window, cx);
                } else {
                    self.show_toast(
                        Status::Warn,
                        "This link could not be followed",
                        "It points to a page that is not in this document.",
                        cx,
                    );
                }
            }
            LinkTarget::Uri(uri) if openable(&uri) => cx.open_url(&uri),
            LinkTarget::Uri(_) => self.show_toast(
                Status::Warn,
                "This link was not opened",
                "Only web and email links open from a document.",
                cx,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(page: usize, y: f32) -> Place {
        Place {
            page,
            offset: point(px(0.0), px(y)),
        }
    }

    #[test]
    fn back_and_forward_retrace_jumps_in_order() {
        let mut jumps = Jumps::default();
        jumps.record(place(0, -40.0));
        jumps.record(place(4, 0.0));
        assert_eq!(jumps.back(place(9, -10.0)), Some(place(4, 0.0)));
        assert_eq!(jumps.back(place(4, 0.0)), Some(place(0, -40.0)));
        assert_eq!(jumps.back(place(0, -40.0)), None);
        assert_eq!(jumps.forward(place(0, -40.0)), Some(place(4, 0.0)));
        assert_eq!(jumps.forward(place(4, 0.0)), Some(place(9, -10.0)));
        assert_eq!(jumps.forward(place(9, -10.0)), None);
    }

    #[test]
    fn a_new_jump_clears_forward_history_and_skips_duplicates() {
        let mut jumps = Jumps::default();
        jumps.record(place(0, 0.0));
        jumps.record(place(0, 0.0));
        assert_eq!(jumps.back(place(3, 0.0)), Some(place(0, 0.0)));
        assert_eq!(jumps.back(place(0, 0.0)), None);
        jumps.record(place(0, 0.0));
        assert_eq!(jumps.forward(place(5, 0.0)), None);
    }

    #[test]
    fn history_is_bounded() {
        let mut jumps = Jumps::default();
        for page in 0..MAX_JUMPS + 20 {
            jumps.record(place(page, 0.0));
        }
        let mut count = 0;
        let mut current = place(usize::MAX, 0.0);
        while let Some(previous) = jumps.back(current) {
            current = previous;
            count += 1;
        }
        assert_eq!(count, MAX_JUMPS);
        assert_eq!(current.page, 20);
    }

    #[test]
    fn only_web_and_email_links_open() {
        assert!(openable("https://example.com/"));
        assert!(openable("HTTP://example.com"));
        assert!(openable("mailto:someone@example.com"));
        assert!(!openable("file:///etc/passwd"));
        assert!(!openable("javascript:alert(1)"));
        assert!(!openable("https:"));
        assert!(!openable("example.com"));
    }
}
