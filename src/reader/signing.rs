use super::{Reader, chrome::Overlay};
use crate::pdf::{
    markup::{Edit, Picture, Signature, SketchKind, Stamp},
    text::TextRect,
};
use crate::ui::kit::Status;
use gpui::{Bounds, Context, PathPromptOptions, Pixels, Point, RenderImage, Window, px};
use std::path::Path;
use std::{cell::Cell, rc::Rc, sync::Arc};

pub(super) const INKS: [u32; 3] = [0x1c3d8f, 0x1d2024, 0xb42318];
pub(super) const OUTLINE_OFFSET: f32 = 6.0;
const MAX_PAD_POINTS: usize = 20_000;
const MIN_POINT_DISTANCE: f32 = 1.5;
const MIN_INK_EXTENT: f32 = 12.0;
const PLACED_WIDTH: f32 = 0.32;
const PLACED_MAX_HEIGHT: f32 = 0.25;
const MIN_PLACED_WIDTH: f32 = 0.04;
const STAMP_WIDTH: f32 = 0.24;
const STAMP_SHAPE: f32 = 3.2;
const MAX_PICTURE_SIDE: u32 = 1_600;
const MAX_PLACED_PICTURES: usize = 8;
const HANDLE_REACH: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Drag {
    Move { grab: [f32; 2] },
    Resize { anchor: [f32; 2], toward: [f32; 2] },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Placing {
    pub session: u64,
    pub index: usize,
    pub drag: Option<Drag>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Placed {
    pub page: usize,
    pub bounds: TextRect,
}

#[derive(Clone)]
pub(super) struct LoadedPicture {
    pub pixels: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub preview: Arc<RenderImage>,
}

pub(super) fn load_picture(path: &Path) -> anyhow::Result<LoadedPicture> {
    use anyhow::Context as _;
    let decoded = image::open(path)
        .with_context(|| format!("{} is not a PNG or JPEG that can be read.", path.display()))?
        .to_rgba8();
    let longest = decoded.width().max(decoded.height());
    let pixels = if longest > MAX_PICTURE_SIDE {
        let scale = MAX_PICTURE_SIDE as f32 / longest as f32;
        image::imageops::thumbnail(
            &decoded,
            ((decoded.width() as f32 * scale).round() as u32).max(1),
            ((decoded.height() as f32 * scale).round() as u32).max(1),
        )
    } else {
        decoded
    };
    if pixels.width() == 0 || pixels.height() == 0 {
        anyhow::bail!("The image is empty.");
    }
    let (width, height) = pixels.dimensions();
    let rgba = pixels.into_raw();
    let mut shown = rgba.clone();
    for pixel in shown.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let frame = image::RgbaImage::from_raw(width, height, shown)
        .context("The image could not be prepared for display.")?;
    Ok(LoadedPicture {
        pixels: Arc::new(rgba),
        width,
        height,
        preview: Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
            frame
        )])),
    })
}

#[derive(Default)]
pub(super) struct Signing {
    pub picture: Option<LoadedPicture>,
    pub placed_pictures: Vec<LoadedPicture>,
    pub strokes: Vec<Vec<[f32; 2]>>,
    pub drawing: bool,
    pub pad: Rc<Cell<Bounds<Pixels>>>,
    pub placing: Option<Placing>,
}

impl Signing {
    fn points(&self) -> usize {
        self.strokes.iter().map(Vec::len).sum()
    }
}

pub(super) fn normalize(strokes: &[Vec<[f32; 2]>]) -> Option<(Vec<Vec<[f32; 2]>>, f32)> {
    let mut points = strokes.iter().flatten();
    let first = points.next()?;
    let (mut left, mut top, mut right, mut bottom) = (first[0], first[1], first[0], first[1]);
    for point in points {
        left = left.min(point[0]);
        top = top.min(point[1]);
        right = right.max(point[0]);
        bottom = bottom.max(point[1]);
    }
    let widen = |low: &mut f32, high: &mut f32| {
        let missing = MIN_INK_EXTENT - (*high - *low);
        if missing > 0.0 {
            *low -= missing / 2.0;
            *high += missing / 2.0;
        }
    };
    widen(&mut left, &mut right);
    widen(&mut top, &mut bottom);
    let (width, height) = (right - left, bottom - top);
    let unit = strokes
        .iter()
        .filter(|stroke| !stroke.is_empty())
        .map(|stroke| {
            stroke
                .iter()
                .map(|point| [(point[0] - left) / width, (point[1] - top) / height])
                .collect()
        })
        .collect();
    Some((unit, width / height))
}

pub(super) fn resized(bounds: TextRect, anchor: [f32; 2], toward: [f32; 2], x: f32) -> TextRect {
    let shape = (bounds.bottom - bounds.top) / (bounds.right - bounds.left);
    let room = |anchor: f32, toward: f32| if toward > 0.0 { 1.0 - anchor } else { anchor };
    let mut width = ((x - anchor[0]) * toward[0])
        .max(MIN_PLACED_WIDTH)
        .min(room(anchor[0], toward[0]));
    let mut height = width * shape;
    let tallest = room(anchor[1], toward[1]);
    if height > tallest {
        height = tallest;
        width = height / shape;
    }
    let span = |anchor: f32, toward: f32, extent: f32| {
        if toward > 0.0 {
            (anchor, anchor + extent)
        } else {
            (anchor - extent, anchor)
        }
    };
    let (left, right) = span(anchor[0], toward[0], width);
    let (top, bottom) = span(anchor[1], toward[1], height);
    TextRect {
        left,
        top,
        right,
        bottom,
    }
}

pub(super) fn moved(bounds: TextRect, grab: [f32; 2], x: f32, y: f32) -> TextRect {
    let (width, height) = (bounds.right - bounds.left, bounds.bottom - bounds.top);
    let left = (x - grab[0]).clamp(0.0, (1.0 - width).max(0.0));
    let top = (y - grab[1]).clamp(0.0, (1.0 - height).max(0.0));
    TextRect {
        left,
        top,
        right: left + width,
        bottom: top + height,
    }
}

fn within(frame: Bounds<Pixels>, position: Point<Pixels>) -> Option<[f32; 2]> {
    let (width, height) = (f32::from(frame.size.width), f32::from(frame.size.height));
    (width > 0.0 && height > 0.0).then(|| {
        [
            f32::from(position.x - frame.origin.x) / width,
            f32::from(position.y - frame.origin.y) / height,
        ]
    })
}

impl Reader {
    fn pad_point(&self, position: Point<Pixels>) -> Option<[f32; 2]> {
        let pad = self.signing.pad.get();
        let (width, height) = (f32::from(pad.size.width), f32::from(pad.size.height));
        (width > 0.0 && height > 0.0).then(|| {
            [
                f32::from(position.x - pad.origin.x).clamp(0.0, width),
                f32::from(position.y - pad.origin.y).clamp(0.0, height),
            ]
        })
    }

    pub(super) fn begin_stroke(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(point) = self.pad_point(position) else {
            return;
        };
        if self.signing.points() >= MAX_PAD_POINTS {
            self.show_toast(
                Status::Info,
                "The signature is too detailed",
                "Clear the pad to draw it again.",
                cx,
            );
            return;
        }
        self.signing.strokes.push(vec![point]);
        self.signing.drawing = true;
        cx.notify();
    }

    pub(super) fn extend_stroke(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(point) = self.pad_point(position) else {
            return;
        };
        if self.signing.points() >= MAX_PAD_POINTS {
            self.signing.drawing = false;
            return;
        }
        let Some(stroke) = self.signing.strokes.last_mut() else {
            return;
        };
        let far = stroke.last().is_none_or(|last| {
            (last[0] - point[0]).hypot(last[1] - point[1]) >= MIN_POINT_DISTANCE
        });
        if far {
            stroke.push(point);
            cx.notify();
        }
    }

    pub(super) fn clear_pad(&mut self, cx: &mut Context<Self>) {
        self.signing.strokes.clear();
        self.signing.drawing = false;
        cx.notify();
    }

    fn placing(&self) -> Option<Placing> {
        let placing = self.signing.placing?;
        let tab = self.tabs.current()?;
        (tab.session == placing.session
            && placing.index + 1 == tab.edits.len()
            && placing.index >= tab.saved
            && !tab.saving
            && tab
                .edits
                .last()
                .is_some_and(|edit| edit.placement().is_some()))
        .then_some(placing)
    }

    pub(super) fn placing_signature(&self) -> Option<Placed> {
        self.placing()?;
        let (page, bounds) = self.tabs.current()?.edits.last()?.placement()?;
        Some(Placed { page, bounds })
    }

    fn begin_placing(&mut self, edit: Edit, cx: &mut Context<Self>) -> bool {
        if !self.can_edit(cx) {
            return false;
        }
        self.finish_placement(cx);
        let Some(tab) = self.tabs.current_mut() else {
            return false;
        };
        tab.edits.push(edit);
        tab.undone.clear();
        self.signing.placing = Some(Placing {
            session: tab.session,
            index: tab.edits.len() - 1,
            drag: None,
        });
        self.selection = None;
        self.chrome.overlay = Overlay::None;
        if self.chrome.input == super::chrome::Input::SignName {
            self.chrome.input = super::chrome::Input::None;
        }
        true
    }

    pub(super) fn place_stamp(&mut self, label: &str, color: u32, cx: &mut Context<Self>) {
        let Some(page) = self.tabs.current().map(|tab| tab.page) else {
            return;
        };
        let (page_shape, center) = self.placement_center(page);
        let width = STAMP_WIDTH;
        let height = (width * page_shape / STAMP_SHAPE).min(PLACED_MAX_HEIGHT);
        let left = (1.0 - width) / 2.0;
        let top = (center - height / 2.0).clamp(0.0, 1.0 - height);
        let placed = self.begin_placing(
            Edit::Stamp(Stamp {
                page,
                bounds: TextRect {
                    left,
                    top,
                    right: left + width,
                    bottom: top + height,
                },
                color,
                label: label.to_string(),
            }),
            cx,
        );
        if placed {
            self.show_toast(
                Status::Info,
                &format!("Stamp added to page {}", page + 1),
                "Drag it into place or drag a corner to resize it. Press Enter when it is right.",
                cx,
            );
        }
    }

    fn placement_center(&self, page: usize) -> (f32, f32) {
        let shown = self
            .image
            .as_ref()
            .filter(|_| self.tabs.current().is_some_and(|tab| tab.page == page))
            .filter(|image| image.width > 0.0 && image.height > 0.0)
            .map_or(1.0, |image| image.width / image.height);
        let Some(frame) = self.page_frames.borrow().get(&page).copied() else {
            return (shown, 0.5);
        };
        let (width, height) = (f32::from(frame.size.width), f32::from(frame.size.height));
        if width <= 0.0 || height <= 0.0 {
            return (shown, 0.5);
        }
        let viewport = self
            .tabs
            .current()
            .map(|tab| tab.scroll.bounds())
            .filter(|viewport| viewport.size.height > px(0.0));
        let center = viewport.map_or(0.5, |viewport| {
            let top = viewport.top().max(frame.top());
            let bottom = viewport.bottom().min(frame.bottom());
            if bottom > top {
                f32::from((top + bottom) / 2.0 - frame.top()) / height
            } else {
                0.5
            }
        });
        (width / height, center)
    }

    pub(super) fn preview_of(&self, pixels: &Arc<Vec<u8>>) -> Option<Arc<RenderImage>> {
        self.signing
            .placed_pictures
            .iter()
            .chain(&self.signing.picture)
            .find(|picture| Arc::ptr_eq(&picture.pixels, pixels))
            .map(|picture| picture.preview.clone())
    }

    pub(super) fn use_signature_image(&mut self, path: &Path, cx: &mut Context<Self>) {
        match load_picture(path) {
            Ok(picture) => self.signing.picture = Some(picture),
            Err(error) => self.show_toast(
                Status::Warn,
                "The image could not be used",
                &format!("{error:#}"),
                cx,
            ),
        }
        cx.notify();
    }

    pub(super) fn choose_signature_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.picker_open {
            return;
        }
        self.picker_open = true;
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Use image".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = chosen.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.picker_open = false;
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first() {
                            this.use_signature_image(path, cx);
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => this.show_toast(
                        Status::Warn,
                        "The file picker could not be opened",
                        "Please try again.",
                        cx,
                    ),
                }
                this.focus.focus(window);
            });
        })
        .detach();
    }

    pub(super) fn place_typed_signature(&mut self, cx: &mut Context<Self>) {
        let ink = INKS[self.chrome.sign_ink % INKS.len()];
        match super::typeset::signature(
            &self.chrome.sign_name.text,
            crate::ui::theme::Theme::script_font(),
            ink,
        ) {
            Ok(picture) => {
                self.signing.picture = Some(picture);
                self.place_signature_image(cx);
            }
            Err(error) => self.show_toast(
                Status::Info,
                "The signature could not be made",
                &format!("{error:#}"),
                cx,
            ),
        }
    }

    pub(super) fn place_signature_image(&mut self, cx: &mut Context<Self>) {
        let Some(picture) = self.signing.picture.clone() else {
            self.show_toast(
                Status::Info,
                "Choose an image first",
                "Click the pad to pick a PNG or JPEG of your signature.",
                cx,
            );
            return;
        };
        if self.signing.placed_pictures.len() >= MAX_PLACED_PICTURES
            && !self
                .signing
                .placed_pictures
                .iter()
                .any(|placed| Arc::ptr_eq(&placed.pixels, &picture.pixels))
        {
            self.show_toast(
                Status::Warn,
                "Too many signature images",
                &format!("At most {MAX_PLACED_PICTURES} different images can be placed at once."),
                cx,
            );
            return;
        }
        let Some(page) = self.tabs.current().map(|tab| tab.page) else {
            return;
        };
        let (page_shape, center) = self.placement_center(page);
        let shape = picture.width as f32 / picture.height as f32;
        let mut width = PLACED_WIDTH;
        let mut height = width * page_shape / shape;
        if height > PLACED_MAX_HEIGHT {
            width *= PLACED_MAX_HEIGHT / height;
            height = PLACED_MAX_HEIGHT;
        }
        let left = (1.0 - width) / 2.0;
        let top = (center - height / 2.0).clamp(0.0, 1.0 - height);
        let placed = self.begin_placing(
            Edit::Picture(Picture {
                page,
                bounds: TextRect {
                    left,
                    top,
                    right: left + width,
                    bottom: top + height,
                },
                pixels: picture.pixels.clone(),
                width: picture.width,
                height: picture.height,
            }),
            cx,
        );
        if placed {
            if !self
                .signing
                .placed_pictures
                .iter()
                .any(|known| Arc::ptr_eq(&known.pixels, &picture.pixels))
            {
                self.signing.placed_pictures.push(picture);
            }
            self.show_toast(
                Status::Info,
                &format!("Signature added to page {}", page + 1),
                "Drag it into place or drag a corner to resize it. Press Enter when it is right.",
                cx,
            );
        }
    }

    pub(super) fn place_signature(&mut self, cx: &mut Context<Self>) {
        let Some((strokes, shape)) = normalize(&self.signing.strokes) else {
            self.show_toast(
                Status::Info,
                "Draw your signature first",
                "Drag inside the pad to sign, then place it on the page.",
                cx,
            );
            return;
        };
        if !self.can_edit(cx) {
            return;
        }
        self.finish_placement(cx);
        let Some(tab) = self.tabs.current() else {
            return;
        };
        let page = tab.page;
        let (page_shape, center) = self.placement_center(page);
        let mut width = PLACED_WIDTH;
        let mut height = width * page_shape / shape;
        if height > PLACED_MAX_HEIGHT {
            width *= PLACED_MAX_HEIGHT / height;
            height = PLACED_MAX_HEIGHT;
        }
        let left = (1.0 - width) / 2.0;
        let top = (center - height / 2.0).clamp(0.0, 1.0 - height);
        let color = INKS[self.chrome.sign_ink % INKS.len()];
        let Some(tab) = self.tabs.current_mut() else {
            return;
        };
        tab.edits.push(Edit::Signature(Signature {
            page,
            bounds: TextRect {
                left,
                top,
                right: left + width,
                bottom: top + height,
            },
            color,
            strokes: Arc::new(strokes),
            kind: SketchKind::Signature,
            weight: None,
        }));
        tab.undone.clear();
        self.signing.placing = Some(Placing {
            session: tab.session,
            index: tab.edits.len() - 1,
            drag: None,
        });
        self.selection = None;
        self.chrome.overlay = Overlay::None;
        self.show_toast(
            Status::Info,
            &format!("Signature added to page {}", page + 1),
            "Drag it into place or drag a corner to resize it. Press Enter when it is right.",
            cx,
        );
    }

    pub(super) fn placement_press(
        &mut self,
        page: usize,
        position: Point<Pixels>,
        frame: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(signature) = self.placing_signature() else {
            return false;
        };
        let bounds = signature.bounds;
        let point = (signature.page == page)
            .then(|| within(frame, position))
            .flatten();
        let Some([x, y]) = point else {
            self.finish_placement(cx);
            return false;
        };
        let (width, height) = (f32::from(frame.size.width), f32::from(frame.size.height));
        let reach = [
            (OUTLINE_OFFSET + HANDLE_REACH) / width,
            (OUTLINE_OFFSET + HANDLE_REACH) / height,
        ];
        let corners = [
            ([bounds.left, bounds.top], [-1.0, -1.0]),
            ([bounds.right, bounds.top], [1.0, -1.0]),
            ([bounds.left, bounds.bottom], [-1.0, 1.0]),
            ([bounds.right, bounds.bottom], [1.0, 1.0]),
        ];
        let handle = corners.into_iter().find(|(corner, _)| {
            (x - corner[0]).abs() <= reach[0] && (y - corner[1]).abs() <= reach[1]
        });
        let drag = if let Some((corner, toward)) = handle {
            Drag::Resize {
                anchor: [
                    bounds.left + bounds.right - corner[0],
                    bounds.top + bounds.bottom - corner[1],
                ],
                toward,
            }
        } else if bounds.contains(x, y) {
            Drag::Move {
                grab: [x - bounds.left, y - bounds.top],
            }
        } else {
            self.finish_placement(cx);
            return false;
        };
        if let Some(placing) = self.signing.placing.as_mut() {
            placing.drag = Some(drag);
        }
        cx.notify();
        true
    }

    pub(super) fn placement_drag(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(Placing {
            drag: Some(drag), ..
        }) = self.placing()
        else {
            return;
        };
        let Some(signature) = self.placing_signature() else {
            return;
        };
        let frame = self.page_frames.borrow().get(&signature.page).copied();
        let Some([x, y]) = frame.and_then(|frame| within(frame, position)) else {
            return;
        };
        let bounds = match drag {
            Drag::Move { grab } => moved(signature.bounds, grab, x, y),
            Drag::Resize { anchor, toward } => resized(signature.bounds, anchor, toward, x),
        };
        if let Some(edit) = self.tabs.current_mut().and_then(|tab| tab.edits.last_mut()) {
            edit.place(bounds);
            cx.notify();
        }
    }

    pub(super) fn release_pointer(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.signing.drawing) {
            cx.notify();
        }
        self.sketch_release(cx);
        if let Some(placing) = self.signing.placing.as_mut()
            && placing.drag.take().is_some()
        {
            cx.notify();
        }
    }

    pub(super) fn finish_placement(&mut self, cx: &mut Context<Self>) -> bool {
        let finished = self.signing.placing.take().is_some();
        if finished {
            self.schedule_autosave(cx);
            cx.notify();
        }
        finished
    }

    pub(super) fn cancel_placement(&mut self, cx: &mut Context<Self>) -> bool {
        if self.placing().is_none() {
            self.signing.placing = None;
            return false;
        }
        if let Some(tab) = self.tabs.current_mut() {
            tab.edits.pop();
        }
        self.signing.placing = None;
        cx.notify();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strokes_normalize_to_a_unit_box_with_their_shape() {
        let (unit, shape) = normalize(&[
            vec![[10.0, 20.0], [110.0, 70.0]],
            vec![],
            vec![[60.0, 45.0]],
        ])
        .unwrap();
        assert_eq!(unit, [vec![[0.0, 0.0], [1.0, 1.0]], vec![[0.5, 0.5]]]);
        assert_eq!(shape, 2.0);
        let (dot, shape) = normalize(&[vec![[50.0, 50.0]]]).unwrap();
        assert_eq!(dot, [vec![[0.5, 0.5]]]);
        assert_eq!(shape, 1.0);
        assert!(normalize(&[]).is_none());
        assert!(normalize(&[vec![]]).is_none());
    }

    #[test]
    fn moving_and_resizing_keep_the_signature_on_the_page_and_in_shape() {
        let bounds = TextRect {
            left: 0.3,
            top: 0.4,
            right: 0.7,
            bottom: 0.5,
        };
        let shifted = moved(bounds, [0.1, 0.05], 0.95, -0.2);
        assert!((shifted.left - 0.6).abs() < 1e-6 && (shifted.right - 1.0).abs() < 1e-6);
        assert!((shifted.top - 0.0).abs() < 1e-6 && (shifted.bottom - 0.1).abs() < 1e-6);

        let grown = resized(bounds, [0.3, 0.4], [1.0, 1.0], 0.9);
        assert!((grown.left - 0.3).abs() < 1e-6 && (grown.right - 0.9).abs() < 1e-6);
        assert!((grown.bottom - 0.55).abs() < 1e-6);

        let capped = resized(bounds, [0.7, 0.5], [-1.0, -1.0], -3.0);
        assert!((capped.right - 0.7).abs() < 1e-6 && (capped.bottom - 0.5).abs() < 1e-6);
        assert!((capped.left - 0.0).abs() < 1e-6);
        assert!((capped.top - 0.325).abs() < 1e-6);

        let tiny = resized(bounds, [0.3, 0.4], [1.0, 1.0], 0.31);
        assert!((tiny.right - tiny.left - MIN_PLACED_WIDTH).abs() < 1e-6);
        let ratio = (tiny.bottom - tiny.top) / (tiny.right - tiny.left);
        assert!((ratio - 0.25).abs() < 1e-5);
    }
}
