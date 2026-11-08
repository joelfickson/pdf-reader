use gpui::App;
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

const VARIABLE: &str = "PDF_READER_BENCH";
const FIRST_PAGE: &str = "first-page";

static STARTED: OnceLock<Instant> = OnceLock::new();
static REPORTED: AtomicBool = AtomicBool::new(false);

pub fn start() {
    STARTED.get_or_init(Instant::now);
}

fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var(VARIABLE).is_ok_and(|value| value == FIRST_PAGE))
}

fn elapsed_ms() -> f64 {
    STARTED
        .get()
        .map_or(0.0, |started| started.elapsed().as_secs_f64() * 1000.0)
}

pub fn mark(stage: &str) {
    if enabled() && !REPORTED.load(Ordering::SeqCst) {
        println!("{stage}_ms={:.2}", elapsed_ms());
    }
}

pub fn page_painted(cx: &mut App) {
    if !enabled() || REPORTED.swap(true, Ordering::SeqCst) {
        return;
    }
    println!("first_page_painted_ms={:.2}", elapsed_ms());
    cx.defer(|cx| cx.quit());
}

const SCROLL: &str = "scroll";
pub const SCROLL_STEP_PX: f32 = 120.0;

pub fn scrolling() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var(VARIABLE).is_ok_and(|value| value == SCROLL))
}

#[derive(Default)]
pub struct ScrollProbe {
    frames: Vec<Instant>,
    blank_frames: usize,
}

impl ScrollProbe {
    pub fn frame(&mut self, blank: bool) {
        self.frames.push(Instant::now());
        self.blank_frames += usize::from(blank);
    }

    pub fn report(&self, pages: usize, cx: &mut App) {
        let mut gaps: Vec<f64> = self
            .frames
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).as_secs_f64() * 1000.0)
            .collect();
        gaps.sort_by(f64::total_cmp);
        let at = |fraction: f64| {
            gaps.get(((gaps.len() as f64 - 1.0) * fraction).round() as usize)
                .copied()
                .unwrap_or(0.0)
        };
        let total = match (self.frames.first(), self.frames.last()) {
            (Some(first), Some(last)) => (*last - *first).as_secs_f64(),
            _ => 0.0,
        };
        println!(
            "scroll_pages={pages} frames={} seconds={total:.2} frame_ms_median={:.2} frame_ms_p95={:.2} frame_ms_max={:.2} frames_over_33ms={} frames_with_unrendered_visible_page={}",
            self.frames.len(),
            at(0.5),
            at(0.95),
            at(1.0),
            gaps.iter().filter(|gap| **gap > 33.3).count(),
            self.blank_frames,
        );
        cx.defer(|cx| cx.quit());
    }
}
