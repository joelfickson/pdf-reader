use super::{
    Reader,
    chrome::{Mode, Tool},
};
use crate::pdf::{
    markup::{Edit, Signature, SketchKind},
    text::TextRect,
};
use crate::ui::theme::Marker;
use gpui::{Bounds, Context, Pixels, Point};
use std::sync::Arc;

pub(super) const INK_WEIGHT: f32 = 0.004;
const MAX_INK_POINTS: usize = 5_000;
const MIN_POINT_STEP: f32 = 0.002;
const MIN_SHAPE_SIDE: f32 = 0.01;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Sketch {
    Ink {
        page: usize,
        points: Vec<[f32; 2]>,
    },
    Rectangle {
        page: usize,
        from: [f32; 2],
        to: [f32; 2],
    },
}

fn on_page(frame: Bounds<Pixels>, position: Point<Pixels>) -> Option<[f32; 2]> {
    let (width, height) = (f32::from(frame.size.width), f32::from(frame.size.height));
    (width > 0.0 && height > 0.0).then(|| {
        [
            (f32::from(position.x - frame.origin.x) / width).clamp(0.0, 1.0),
            (f32::from(position.y - frame.origin.y) / height).clamp(0.0, 1.0),
        ]
    })
}

impl Sketch {
    fn page(&self) -> usize {
        match self {
            Self::Ink { page, .. } | Self::Rectangle { page, .. } => *page,
        }
    }

    pub(super) fn drawing(&self, color: u32) -> Option<Signature> {
        match self {
            Self::Ink { page, points } => (!points.is_empty()).then(|| Signature {
                page: *page,
                bounds: TextRect {
                    left: 0.0,
                    top: 0.0,
                    right: 1.0,
                    bottom: 1.0,
                },
                color,
                strokes: Arc::new(vec![points.clone()]),
                kind: SketchKind::Ink,
                weight: Some(INK_WEIGHT),
            }),
            Self::Rectangle { page, from, to } => {
                let bounds = TextRect {
                    left: from[0].min(to[0]),
                    top: from[1].min(to[1]),
                    right: from[0].max(to[0]),
                    bottom: from[1].max(to[1]),
                };
                (bounds.right - bounds.left >= MIN_SHAPE_SIDE
                    && bounds.bottom - bounds.top >= MIN_SHAPE_SIDE)
                    .then(|| Signature {
                        page: *page,
                        bounds,
                        color,
                        strokes: Arc::new(vec![vec![
                            [0.0, 0.0],
                            [1.0, 0.0],
                            [1.0, 1.0],
                            [0.0, 1.0],
                            [0.0, 0.0],
                        ]]),
                        kind: SketchKind::Rectangle,
                        weight: Some(INK_WEIGHT),
                    })
            }
        }
    }
}

impl Reader {
    fn sketch_color(&self) -> u32 {
        Marker::ALL[self.chrome.marker % Marker::ALL.len()]
    }

    pub(super) fn live_sketch(&self, page: usize) -> Option<Signature> {
        self.sketch
            .as_ref()
            .filter(|sketch| sketch.page() == page)
            .and_then(|sketch| sketch.drawing(self.sketch_color()))
    }

    pub(super) fn sketch_press(
        &mut self,
        page: usize,
        position: Point<Pixels>,
        frame: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.chrome.mode != Mode::Annotate
            || !matches!(self.chrome.tool, Tool::Draw | Tool::Shape)
        {
            return false;
        }
        let Some(point) = on_page(frame, position) else {
            return false;
        };
        if !self.can_edit(cx) {
            return true;
        }
        self.selection = None;
        self.sketch = Some(if self.chrome.tool == Tool::Draw {
            Sketch::Ink {
                page,
                points: vec![point],
            }
        } else {
            Sketch::Rectangle {
                page,
                from: point,
                to: point,
            }
        });
        cx.notify();
        true
    }

    pub(super) fn sketch_drag(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(page) = self.sketch.as_ref().map(Sketch::page) else {
            return;
        };
        let frame = self.page_frames.borrow().get(&page).copied();
        let Some(point) = frame.and_then(|frame| on_page(frame, position)) else {
            return;
        };
        match self.sketch.as_mut() {
            Some(Sketch::Ink { points, .. }) => {
                let far = points.last().is_none_or(|last| {
                    (last[0] - point[0]).hypot(last[1] - point[1]) >= MIN_POINT_STEP
                });
                if far && points.len() < MAX_INK_POINTS {
                    points.push(point);
                    cx.notify();
                }
            }
            Some(Sketch::Rectangle { to, .. }) => {
                *to = point;
                cx.notify();
            }
            None => {}
        }
    }

    pub(super) fn sketch_release(&mut self, cx: &mut Context<Self>) {
        let Some(sketch) = self.sketch.take() else {
            return;
        };
        cx.notify();
        let Some(drawing) = sketch.drawing(self.sketch_color()) else {
            return;
        };
        if let Some(tab) = self.tabs.current_mut() {
            tab.edits.push(Edit::Signature(drawing));
            tab.undone.clear();
            self.schedule_autosave(cx);
        }
    }

    pub(super) fn look_up(&mut self, cx: &mut Context<Self>) {
        let Some(Ok(text)) = self.selected_text() else {
            return;
        };
        let term: String = text
            .split_whitespace()
            .take(6)
            .collect::<Vec<_>>()
            .join(" ");
        if term.is_empty() {
            return;
        }
        if cfg!(target_os = "macos") {
            cx.open_url(&format!("dict://{}", encode(&term)));
        } else {
            self.unavailable("Look up on this platform", cx);
        }
    }
}

pub(super) fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sketches_become_drawings_only_when_they_have_substance() {
        let ink = Sketch::Ink {
            page: 2,
            points: vec![[0.1, 0.2], [0.3, 0.4]],
        };
        let drawing = ink.drawing(0xf2c230).unwrap();
        assert_eq!((drawing.page, drawing.kind), (2, SketchKind::Ink));
        assert_eq!(drawing.strokes[0], [[0.1, 0.2], [0.3, 0.4]]);
        assert_eq!(drawing.weight, Some(INK_WEIGHT));
        assert!(
            Sketch::Ink {
                page: 0,
                points: vec![]
            }
            .drawing(0)
            .is_none()
        );

        let shape = Sketch::Rectangle {
            page: 0,
            from: [0.6, 0.5],
            to: [0.2, 0.3],
        }
        .drawing(0)
        .unwrap();
        assert_eq!(
            (
                shape.bounds.left,
                shape.bounds.top,
                shape.bounds.right,
                shape.bounds.bottom
            ),
            (0.2, 0.3, 0.6, 0.5)
        );
        assert_eq!(shape.strokes[0].len(), 5);
        assert!(
            Sketch::Rectangle {
                page: 0,
                from: [0.2, 0.2],
                to: [0.205, 0.6]
            }
            .drawing(0)
            .is_none()
        );
    }

    #[test]
    fn look_up_terms_are_percent_encoded() {
        assert_eq!(encode("naïve café"), "na%C3%AFve%20caf%C3%A9");
        assert_eq!(encode("plain-word_1.2~"), "plain-word_1.2~");
    }
}
