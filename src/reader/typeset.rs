use super::signing::LoadedPicture;
use anyhow::{Context, Result, bail};
use gpui::RenderImage;
use pathfinder_geometry::{
    transform2d::Transform2F,
    vector::{Vector2F, Vector2I},
};
use std::sync::Arc;
use zed_font_kit::{
    canvas::{Canvas, Format, RasterizationOptions},
    family_name::FamilyName,
    hinting::HintingOptions,
    properties::Properties,
    source::SystemSource,
};

pub(super) const MAX_NAME_CHARACTERS: usize = 60;
const POINT_SIZE: f32 = 120.0;
const PADDING: f32 = 48.0;
const INK_THRESHOLD: u8 = 8;

pub(super) fn signature(name: &str, family: &str, color: u32) -> Result<LoadedPicture> {
    let name = name.trim();
    if name.is_empty() {
        bail!("Type your name first.");
    }
    let font = SystemSource::new()
        .select_best_match(
            &[FamilyName::Title(family.to_string()), FamilyName::Cursive],
            &Properties::new(),
        )
        .context("No handwriting font was found on this computer.")?
        .load()
        .context("The handwriting font could not be loaded.")?;
    let metrics = font.metrics();
    let scale = POINT_SIZE / metrics.units_per_em as f32;
    let mut glyphs = Vec::new();
    let mut cursor = PADDING;
    for character in name.chars().take(MAX_NAME_CHARACTERS) {
        let Some(glyph) = font.glyph_for_char(character) else {
            continue;
        };
        let advance = font
            .advance(glyph)
            .map_or(POINT_SIZE * 0.4, |advance| advance.x() * scale);
        glyphs.push((glyph, cursor));
        cursor += advance;
    }
    if glyphs.is_empty() {
        bail!("The handwriting font has none of these characters.");
    }
    let width = (cursor + PADDING).ceil() as i32;
    let height = ((metrics.ascent - metrics.descent) * scale + PADDING * 2.0).ceil() as i32;
    let baseline = PADDING + metrics.ascent * scale;
    let mut canvas = Canvas::new(Vector2I::new(width, height), Format::A8);
    for (glyph, x) in glyphs {
        font.rasterize_glyph(
            &mut canvas,
            glyph,
            POINT_SIZE,
            Transform2F::from_translation(Vector2F::new(x, baseline)),
            HintingOptions::None,
            RasterizationOptions::GrayscaleAa,
        )
        .ok();
    }
    let (width, height) = (width as usize, height as usize);
    let coverage = |x: usize, y: usize| canvas.pixels[y * canvas.stride + x];
    let inked = |x: usize, y: usize| coverage(x, y) > INK_THRESHOLD;
    let columns: Vec<usize> = (0..width)
        .filter(|x| (0..height).any(|y| inked(*x, y)))
        .collect();
    let rows: Vec<usize> = (0..height)
        .filter(|y| (0..width).any(|x| inked(x, *y)))
        .collect();
    let (Some(left), Some(right), Some(top), Some(bottom)) =
        (columns.first(), columns.last(), rows.first(), rows.last())
    else {
        bail!("The name could not be drawn in the handwriting font.");
    };
    let (out_width, out_height) = (right - left + 1, bottom - top + 1);
    let channels = [(color >> 16) as u8, (color >> 8) as u8, color as u8];
    let mut rgba = Vec::with_capacity(out_width * out_height * 4);
    for y in *top..=*bottom {
        for x in *left..=*right {
            rgba.extend([channels[0], channels[1], channels[2], coverage(x, y)]);
        }
    }
    let mut shown = rgba.clone();
    for pixel in shown.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let frame = image::RgbaImage::from_raw(out_width as u32, out_height as u32, shown)
        .context("The signature could not be prepared for display.")?;
    Ok(LoadedPicture {
        pixels: Arc::new(rgba),
        width: out_width as u32,
        height: out_height as u32,
        preview: Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
            frame
        )])),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_drawn_as_cropped_ink_on_a_clear_background() {
        if !cfg!(target_os = "macos") {
            return;
        }
        let family = crate::ui::theme::Theme::script_font();
        let short = signature("Al", family, 0x1c3d8f).unwrap();
        let long = signature("  Alexandra Morgan ", family, 0x1c3d8f).unwrap();
        assert!(long.width > short.width * 2);
        assert_eq!(long.pixels.len(), (long.width * long.height * 4) as usize);
        let pixels = long.pixels.as_chunks::<4>().0;
        assert!(pixels.iter().all(|pixel| pixel[..3] == [0x1c, 0x3d, 0x8f]));
        let solid = pixels.iter().filter(|pixel| pixel[3] > 200).count();
        let clear = pixels.iter().filter(|pixel| pixel[3] == 0).count();
        assert!(solid > 200 && clear > solid);
        let edge = |x: u32, y: u32| pixels[(y * long.width + x) as usize][3] > INK_THRESHOLD;
        assert!((0..long.height).any(|y| edge(0, y)));
        assert!((0..long.width).any(|x| edge(x, 0)));
        assert!(signature("   ", family, 0).is_err());
    }
}
