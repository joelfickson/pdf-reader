use super::{Reader, actions::*};
use gpui::{Bounds, ClipboardItem, Context, Pixels, Point, Window};
use std::ops::Range;

#[derive(Clone)]
pub(super) struct Selection {
    pub anchor: usize,
    pub head: usize,
    pub dragging: bool,
}

impl Selection {
    pub fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }
}

impl Reader {
    pub(super) fn has_selection(&self) -> bool {
        self.selection
            .as_ref()
            .is_some_and(|selection| !selection.range().is_empty())
    }

    pub(super) fn select_at(
        &mut self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        start: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(Ok(text)) = &self.text else {
            return;
        };
        if bounds.size.width <= gpui::px(0.0) || bounds.size.height <= gpui::px(0.0) {
            return;
        }
        let x = f32::from(position.x - bounds.origin.x) / f32::from(bounds.size.width);
        let y = f32::from(position.y - bounds.origin.y) / f32::from(bounds.size.height);
        if start {
            self.focus.focus(window);
            self.selection = text.caret(x, y, true).map(|anchor| Selection {
                anchor,
                head: anchor,
                dragging: true,
            });
        } else if let Some(selection) = self.selection.as_mut()
            && selection.dragging
            && let Some(head) = text.caret(x, y, false)
        {
            selection.head = head;
        }
        self.copied = false;
        cx.notify();
    }

    pub(super) fn copy(&mut self, _: &CopyText, _: &mut Window, cx: &mut Context<Self>) {
        if let (Some(Ok(text)), Some(selection)) = (&self.text, &self.selection) {
            let content = text.content(selection.range());
            if !content.is_empty() {
                cx.write_to_clipboard(ClipboardItem::new_string(content));
                self.copied = true;
                cx.notify();
            }
        }
    }

    pub(super) fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(Ok(text)) = &self.text {
            self.selection = Some(Selection {
                anchor: 0,
                head: text.glyphs.len(),
                dragging: false,
            });
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

    pub(super) fn selection_status(&self) -> String {
        if let Some(notice) = &self.notice {
            return notice.clone();
        }
        if self.copied {
            return "Text copied".into();
        }
        if self.has_selection() {
            return if cfg!(target_os = "macos") {
                "Text selected · Command + C to copy"
            } else {
                "Text selected · Ctrl + C to copy"
            }
            .into();
        }
        match &self.text {
            Some(Ok(text)) if text.glyphs.is_empty() => "No selectable text on this page".into(),
            Some(Err(message)) => message.clone(),
            None if self.image.is_some() => "Preparing text selection…".into(),
            _ => "Drag across text to select · Drop PDFs to open".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reverse_drag_selects_the_same_range() {
        let selection = Selection {
            anchor: 12,
            head: 3,
            dragging: true,
        };
        assert_eq!(selection.range(), 3..12);
    }
}
