mod library;
pub(crate) mod markup;
pub(crate) mod search;
pub(crate) mod text;
pub(crate) mod tools;
mod worker;

use anyhow::{Result, bail};
pub(crate) use library::bind;
use pdfium_render::prelude::*;
use std::{
    path::{Path, PathBuf},
    time::Instant,
};
pub(crate) use worker::Worker;

pub struct Raster {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub logical_width: f32,
    pub logical_height: f32,
    pub zoom_percent: f32,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ViewKey {
    pub session: u64,
    pub page: usize,
    pub available_width: u32,
    pub zoom: Option<u32>,
    pub scale: u32,
}

pub struct Job {
    pub generation: u64,
    pub path: PathBuf,
    pub view: ViewKey,
    pub render_page: bool,
    pub extract_text: bool,
    pub thumbnails: Vec<usize>,
    pub outline: bool,
    pub sizes: bool,
    pub nearby: Vec<ViewKey>,
    pub nearby_text: Vec<usize>,
}

pub type PageSizes = std::sync::Arc<Vec<(f32, f32)>>;

pub struct SearchJob {
    pub search: u64,
    pub session: u64,
    pub path: PathBuf,
    pub query: search::Query,
}

pub struct SaveJob {
    pub session: u64,
    pub path: PathBuf,
    pub edits: Vec<markup::Edit>,
}

pub enum Event {
    Saved {
        session: u64,
        result: Result<(), String>,
    },
    SearchBatch {
        search: u64,
        scanned: usize,
        pages: Vec<(usize, Vec<search::Hit>)>,
    },
    SearchDone {
        search: u64,
        skipped: usize,
        truncated: bool,
    },
    SearchFailed {
        search: u64,
        message: String,
    },
    Text {
        generation: u64,
        key: (u64, usize),
        text: Result<std::sync::Arc<text::PageText>, String>,
    },
    Metadata {
        generation: u64,
        count: usize,
        sizes: Option<PageSizes>,
    },
    Outline {
        session: u64,
        outline: std::sync::Arc<tools::Outline>,
    },
    Page {
        generation: u64,
        view: ViewKey,
        raster: Raster,
    },
    Thumbnail {
        generation: u64,
        page: usize,
        raster: Raster,
    },
    ThumbnailFailed {
        generation: u64,
        page: usize,
    },
    Error {
        generation: u64,
        message: String,
    },
}

impl Event {
    pub fn generation(&self) -> Option<u64> {
        match self {
            Self::Saved { .. }
            | Self::SearchBatch { .. }
            | Self::SearchDone { .. }
            | Self::SearchFailed { .. }
            | Self::Outline { .. } => None,
            Self::Text { generation, .. }
            | Self::Metadata { generation, .. }
            | Self::Page { generation, .. }
            | Self::Thumbnail { generation, .. }
            | Self::ThumbnailFailed { generation, .. }
            | Self::Error { generation, .. } => Some(*generation),
        }
    }
}

fn render(page: &PdfPage<'_>, logical_width: f32, scale: f32, limit: i32) -> Result<Raster> {
    let width = page.width().value;
    let height = page.height().value;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        bail!("This page has invalid dimensions.");
    }
    let target = (logical_width * scale).clamp(1.0, limit as f32) as i32;
    let bitmap = page.render_with_config(
        &PdfRenderConfig::new()
            .set_target_width(target)
            .set_maximum_width(limit)
            .set_maximum_height(limit),
    )?;
    let mut bytes = bitmap.as_rgba_bytes();
    for pixel in bytes.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    Ok(Raster {
        bytes,
        width: bitmap.width() as u32,
        height: bitmap.height() as u32,
        logical_width,
        logical_height: logical_width * height / width,
        zoom_percent: logical_width / (width * 96.0 / 72.0) * 100.0,
    })
}

pub fn inspect(path: &Path) -> Result<()> {
    let started = Instant::now();
    let engine = bind()?;
    let initialized = started.elapsed();
    let document = engine.load_pdf_from_file(path, None)?;
    let opened = started.elapsed();
    let count = document.pages().len();
    let raster = render(&document.pages().get(0)?, 660.0, 2.0, 4096)?;
    println!(
        "pages={count} engine_ms={:.2} open_ms={:.2} first_page_ms={:.2} bitmap_bytes={} dimensions={}x{}",
        initialized.as_secs_f64() * 1000.0,
        (opened - initialized).as_secs_f64() * 1000.0,
        started.elapsed().as_secs_f64() * 1000.0,
        raster.bytes.len(),
        raster.width,
        raster.height
    );
    Ok(())
}
