use crate::pdf::Raster;
use crate::preferences::PageColour;
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

const SEPIA: [f32; 3] = [
    0xd8 as f32 / 255.0,
    0xec as f32 / 255.0,
    0xf4 as f32 / 255.0,
];

pub(super) fn tint(bytes: &mut [u8], colour: PageColour) {
    match colour {
        PageColour::Original => {}
        PageColour::Sepia => {
            for pixel in bytes.as_chunks_mut::<4>().0 {
                for (channel, scale) in pixel.iter_mut().zip(SEPIA) {
                    *channel = (*channel as f32 * scale).round() as u8;
                }
            }
        }
        PageColour::Night => {
            for pixel in bytes.as_chunks_mut::<4>().0 {
                let [blue, green, red] =
                    [pixel[0], pixel[1], pixel[2]].map(|channel| 229.5 - 0.8 * channel as f32);
                let turned = [
                    0.426 * red + 1.430 * green - 0.856 * blue,
                    0.426 * red + 0.430 * green + 0.144 * blue,
                    -0.574 * red + 1.430 * green + 0.144 * blue,
                ];
                for (channel, value) in pixel.iter_mut().zip(turned) {
                    *channel = value.round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn night_darkens_paper_and_lightens_ink_and_sepia_warms_paper() {
        let page = [255, 255, 255, 255, 0, 0, 0, 255, 200, 40, 40, 255];
        let mut original = page;
        tint(&mut original, PageColour::Original);
        assert_eq!(original, page);

        let mut night = page;
        tint(&mut night, PageColour::Night);
        assert!(night[..3].iter().all(|channel| *channel < 40));
        assert!(night[4..7].iter().all(|channel| *channel > 215));
        assert_eq!((night[3], night[7], night[11]), (255, 255, 255));
        assert!(night[8] > night[9] && night[8] > night[10]);

        let mut sepia = page;
        tint(&mut sepia, PageColour::Sepia);
        assert_eq!(sepia[..4], [0xd8, 0xec, 0xf4, 255]);
        assert_eq!(sepia[4..8], [0, 0, 0, 255]);
    }
}
