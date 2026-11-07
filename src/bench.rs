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
