use super::{Reader, TEXT_PAGES, TextResult, actions::*};
use crate::ui::kit::Status;
use gpui::{Bounds, ClipboardItem, Context, Pixels, Point, Window};
use std::ops::{Range, RangeInclusive};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Caret {
    pub page: usize,
    pub glyph: usize,
}

#[derive(Clone, Debug)]
pub(super) struct Selection {
    pub anchor: Caret,
    pub head: Caret,
    pub dragging: bool,
}

#[derive(Debug, PartialEq)]
pub(super) enum CopyError {
    TooLong(usize),
    Loading(usize),
    Unavailable(usize, String),
}

impl Selection {
    pub fn within(page: usize, glyphs: Range<usize>) -> Self {
        Self {
            anchor: Caret {
                page,
                glyph: glyphs.start,
            },
            head: Caret {
                page,
                glyph: glyphs.end,
            },
            dragging: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    fn ends(&self) -> (Caret, Caret) {
        (self.anchor.min(self.head), self.anchor.max(self.head))
    }

    pub fn pages(&self) -> RangeInclusive<usize> {
        let (start, end) = self.ends();
        start.page..=end.page
    }

    pub fn on_page(&self, page: usize, len: usize) -> Range<usize> {
        let (start, end) = self.ends();
        if page < start.page || page > end.page {
            return 0..0;
        }
        let from = if page == start.page {
            start.glyph.min(len)
        } else {
            0
        };
        let to = if page == end.page {
            end.glyph.min(len)
        } else {
            len
        };
        from..to.max(from)
    }

    pub fn content<'a>(
        &self,
        text: impl Fn(usize) -> Option<&'a TextResult>,
    ) -> Result<String, CopyError> {
        let pages = self.pages();
        let span = pages.end() - pages.start() + 1;
        if span > TEXT_PAGES {
            return Err(CopyError::TooLong(span));
        }
        let mut parts = Vec::with_capacity(span);
        for page in pages {
            match text(page) {
                Some(Ok(text)) => {
                    parts.push(text.content(self.on_page(page, text.glyphs.len())));
                }
                Some(Err(message)) => return Err(CopyError::Unavailable(page, message.clone())),
                None => return Err(CopyError::Loading(page)),
            }
        }
        Ok(parts.join("\n"))
    }
}

impl Reader {
    pub(super) fn has_selection(&self) -> bool {
        self.selection
            .as_ref()
            .is_some_and(|selection| !selection.is_empty())
    }

    pub(super) fn page_text(&self, page: usize) -> Option<&TextResult> {
        let tab = self.tabs.current()?;
        match &self.text {
            Some(text) if page == tab.page => Some(text),
            _ => self.texts.peek(&(tab.session, page)),
        }
    }

    pub(super) fn selected_text(&self) -> Option<Result<String, CopyError>> {
        let selection = self
            .selection
            .as_ref()
            .filter(|selection| !selection.is_empty())?;
        Some(selection.content(|page| self.page_text(page)))
    }

    pub(super) fn select_at(
        &mut self,
        page: usize,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        start: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(Ok(text)) = self.page_text(page) else {
            return;
        };
        if bounds.size.width <= gpui::px(0.0) || bounds.size.height <= gpui::px(0.0) {
            return;
        }
        let x = f32::from(position.x - bounds.origin.x) / f32::from(bounds.size.width);
        let y = f32::from(position.y - bounds.origin.y) / f32::from(bounds.size.height);
        let caret = text.caret(x, y, start).map(|glyph| Caret { page, glyph });
        if start {
            self.focus.focus(window);
            self.selection = caret.map(|anchor| Selection {
                anchor,
                head: anchor,
                dragging: true,
            });
        } else if let Some(selection) = self.selection.as_mut()
            && selection.dragging
            && let Some(head) = caret
        {
            selection.head = head;
        }
        self.copied = false;
        cx.notify();
    }

    pub(super) fn drag_selection(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let nearest = self
            .page_frames
            .borrow()
            .iter()
            .min_by(|a, b| distance(a.1, position).total_cmp(&distance(b.1, position)))
            .map(|(page, bounds)| (*page, *bounds));
        let target = nearest.or_else(|| {
            self.tabs
                .current()
                .map(|tab| (tab.page, self.page_bounds.get()))
        });
        if let Some((page, bounds)) = target {
            self.select_at(page, position, bounds, false, window, cx);
        }
    }

    pub(super) fn copy(&mut self, _: &CopyText, window: &mut Window, cx: &mut Context<Self>) {
        match self.selected_text() {
            Some(Ok(content)) if !content.is_empty() => {
                cx.write_to_clipboard(ClipboardItem::new_string(content));
                self.copied = true;
                cx.notify();
            }
            Some(Err(CopyError::Loading(page))) => {
                let missing: Vec<usize> = self
                    .selection
                    .iter()
                    .flat_map(Selection::pages)
                    .filter(|page| self.page_text(*page).is_none())
                    .collect();
                self.wanted_text = missing;
                self.last_request = None;
                self.request(window);
                self.show_toast(
                    Status::Info,
                    "Selection is still loading",
                    &format!(
                        "The text of page {} has not loaded yet. Copy again in a moment.",
                        page + 1
                    ),
                    cx,
                );
            }
            Some(Err(CopyError::Unavailable(page, message))) => self.show_toast(
                Status::Warn,
                "Selection could not be copied",
                &format!("Page {}: {message}", page + 1),
                cx,
            ),
            Some(Err(CopyError::TooLong(span))) => self.show_toast(
                Status::Warn,
                "Selection is too long to copy",
                &format!(
                    "It spans {span} pages. Copy works across at most {TEXT_PAGES} pages at a time."
                ),
                cx,
            ),
            Some(Ok(_)) | None => {}
        }
    }

    pub(super) fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        if let (Some(Ok(text)), Some(tab)) = (&self.text, self.tabs.current()) {
            self.selection = Some(Selection::within(tab.page, 0..text.glyphs.len()));
            self.copied = false;
            cx.notify();
        }
    }

    pub(super) fn clear_selection(
        &mut self,
        _: &ClearSelection,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selection = None;
        self.copied = false;
        cx.notify();
    }
}

fn distance(bounds: &Bounds<Pixels>, position: Point<Pixels>) -> f32 {
    let top = f32::from(bounds.top() - position.y).max(0.0);
    let bottom = f32::from(position.y - bounds.bottom()).max(0.0);
    let left = f32::from(bounds.left() - position.x).max(0.0);
    let right = f32::from(position.x - bounds.right()).max(0.0);
    (top + bottom) * 1_000.0 + left + right
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::text::{Geometry, Glyph, PageText};
    use std::sync::Arc;

    fn page(content: &str) -> TextResult {
        Ok(Arc::new(PageText {
            links: Vec::new(),
            geometry: Geometry::UNIT,
            glyphs: content
                .chars()
                .map(|value| Glyph {
                    value,
                    bounds: None,
                    direction: [1.0, 0.0],
                })
                .collect(),
        }))
    }

    #[test]
    fn reverse_drag_selects_the_same_range() {
        let selection = Selection {
            anchor: Caret { page: 0, glyph: 12 },
            head: Caret { page: 0, glyph: 3 },
            dragging: true,
        };
        assert_eq!(selection.on_page(0, 20), 3..12);
        assert_eq!(selection.on_page(1, 20), 0..0);
    }

    #[test]
    fn selection_across_pages_slices_each_page_in_reading_order() {
        let selection = Selection {
            anchor: Caret { page: 3, glyph: 2 },
            head: Caret { page: 1, glyph: 4 },
            dragging: false,
        };
        assert_eq!(selection.pages(), 1..=3);
        assert_eq!(selection.on_page(0, 10), 0..0);
        assert_eq!(selection.on_page(1, 10), 4..10);
        assert_eq!(selection.on_page(2, 7), 0..7);
        assert_eq!(selection.on_page(3, 10), 0..2);
        assert_eq!(selection.on_page(4, 10), 0..0);
        let pages = [
            page("ignored"),
            page("one\r\ntwo"),
            page("três"),
            page("你好"),
        ];
        assert_eq!(
            selection.content(|index| pages.get(index)),
            Ok("\ntwo\ntrês\n你好".to_string())
        );
    }

    #[test]
    fn copying_across_a_missing_or_failed_page_fails_explicitly() {
        let selection = Selection {
            anchor: Caret { page: 0, glyph: 1 },
            head: Caret { page: 2, glyph: 1 },
            dragging: false,
        };
        let failed: TextResult = Err("Text selection is unavailable on this page.".into());
        let pages = [page("ab"), failed, page("cd")];
        assert_eq!(
            selection.content(|index| (index != 1).then(|| &pages[index])),
            Err(CopyError::Loading(1))
        );
        assert_eq!(
            selection.content(|index| pages.get(index)),
            Err(CopyError::Unavailable(
                1,
                "Text selection is unavailable on this page.".into()
            ))
        );
        let long = Selection {
            anchor: Caret { page: 0, glyph: 0 },
            head: Caret {
                page: TEXT_PAGES,
                glyph: 0,
            },
            dragging: false,
        };
        assert_eq!(
            long.content(|_| None),
            Err(CopyError::TooLong(TEXT_PAGES + 1))
        );
    }
}
