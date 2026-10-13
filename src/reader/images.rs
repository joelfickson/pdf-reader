use crate::pdf::Raster;
use anyhow::{Context, Result};
use gpui::{RenderImage, Window};
use std::sync::Arc;

pub(super) struct PageImage {
    pub(super) image: Arc<RenderImage>,
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) zoom_percent: f32,
    bytes: usize,
}

impl PageImage {
    pub(super) fn byte_len(&self) -> usize {
        self.bytes
    }

    pub(super) fn release(self, window: &mut Window) {
        if let Err(error) = window.drop_image(self.image) {
            eprintln!("Could not release page texture: {error}");
        }
    }
    pub(super) fn new(raster: Raster) -> Result<Self> {
        let bytes = raster.bytes.len();
        let buffer = image::RgbaImage::from_raw(raster.width, raster.height, raster.bytes)
            .context("PDF renderer returned an invalid pixel buffer")?;
        Ok(Self {
            bytes,
            image: Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
                buffer
            )])),
            width: raster.logical_width,
            height: raster.logical_height,
            zoom_percent: raster.zoom_percent,
        })
    }
}
