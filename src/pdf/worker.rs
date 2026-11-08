use super::{
    Event, Job, SearchJob,
    library::bind,
    render,
    search::{self, MAX_HITS},
};
use anyhow::{Context, Result, bail};
use parking_lot::{Condvar, Mutex};
use pdfium_render::prelude::*;
use std::{
    collections::VecDeque,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const SEARCH_SLICE: Duration = Duration::from_millis(30);
const OPEN_DOCUMENTS: usize = 4;

struct Queue {
    pending: Mutex<Option<Job>>,
    search: Mutex<Option<SearchJob>>,
    closed: Mutex<Vec<u64>>,
    ready: Condvar,
    generation: AtomicU64,
    search_generation: AtomicU64,
    stopped: AtomicBool,
}

impl Queue {
    fn new() -> Self {
        Self {
            pending: Mutex::new(None),
            search: Mutex::new(None),
            closed: Mutex::new(Vec::new()),
            ready: Condvar::new(),
            generation: AtomicU64::new(0),
            search_generation: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
        }
    }

    fn running(&self) -> bool {
        !self.stopped.load(Ordering::SeqCst)
    }
}

pub struct Worker(Arc<Queue>);

impl Worker {
    #[cfg(test)]
    pub(crate) fn idle() -> Self {
        Self(Arc::new(Queue::new()))
    }

    pub fn start() -> Result<(Self, async_channel::Receiver<Event>)> {
        let queue = Arc::new(Queue::new());
        let (sender, receiver) = async_channel::bounded(2);
        let worker_queue = queue.clone();
        thread::Builder::new()
            .name("pdf-renderer".into())
            .spawn(move || run(worker_queue, sender))
            .context("Could not start PDF renderer thread")?;
        Ok((Self(queue), receiver))
    }

    pub fn cancel(&self) -> u64 {
        let mut pending = self.0.pending.lock();
        *pending = None;
        self.0.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn close(&self, session: u64) {
        let _pending = self.0.pending.lock();
        self.0.closed.lock().push(session);
        self.0.ready.notify_one();
    }

    pub fn submit(&self, mut job: Job) -> u64 {
        let mut pending = self.0.pending.lock();
        let generation = self.0.generation.fetch_add(1, Ordering::SeqCst) + 1;
        job.generation = generation;
        *pending = Some(job);
        self.0.ready.notify_one();
        generation
    }

    pub fn search(&self, mut job: SearchJob) -> u64 {
        let _pending = self.0.pending.lock();
        let search = self.0.search_generation.fetch_add(1, Ordering::SeqCst) + 1;
        job.search = search;
        *self.0.search.lock() = Some(job);
        self.0.ready.notify_one();
        search
    }

    pub fn cancel_search(&self) -> u64 {
        let _pending = self.0.pending.lock();
        *self.0.search.lock() = None;
        self.0.search_generation.fetch_add(1, Ordering::SeqCst) + 1
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _pending = self.0.pending.lock();
        self.0.stopped.store(true, Ordering::SeqCst);
        self.0.ready.notify_one();
    }
}

type Documents<'a> = VecDeque<(u64, PdfDocument<'a>)>;

fn document<'a, 'b>(
    engine: &'a Pdfium,
    documents: &'b mut Documents<'a>,
    session: u64,
    path: &Path,
) -> Result<&'b PdfDocument<'a>> {
    if let Some(index) = documents.iter().position(|(open, _)| *open == session) {
        if let Some(entry) = documents.remove(index) {
            documents.push_back(entry);
        }
    } else {
        let document = engine.load_pdf_from_file(path, None).context(
            "Could not open this PDF. It may be damaged, password-protected, or inaccessible.",
        )?;
        documents.push_back((session, document));
        while documents.len() > OPEN_DOCUMENTS {
            documents.pop_front();
        }
    }
    documents
        .back()
        .map(|(_, document)| document)
        .context("No PDF document is open")
}

struct ActiveSearch {
    job: SearchJob,
    page: usize,
    hits: usize,
    skipped: usize,
}

fn run(queue: Arc<Queue>, sender: async_channel::Sender<Event>) {
    let engine = bind();
    let mut documents = VecDeque::new();
    let mut active: Option<ActiveSearch> = None;
    loop {
        let (job, closed, search) = {
            let mut pending = queue.pending.lock();
            while pending.is_none()
                && active.is_none()
                && queue.search.lock().is_none()
                && queue.closed.lock().is_empty()
                && queue.running()
            {
                queue.ready.wait(&mut pending);
            }
            if !queue.running() {
                return;
            }
            let closed = std::mem::take(&mut *queue.closed.lock());
            (pending.take(), closed, queue.search.lock().take())
        };
        documents.retain(|(session, _)| !closed.contains(session));
        if let Some(job) = search {
            active = Some(ActiveSearch {
                job,
                page: 0,
                hits: 0,
                skipped: 0,
            });
        }
        if active
            .as_ref()
            .is_some_and(|state| closed.contains(&state.job.session))
        {
            active = None;
        }
        let engine = match engine.as_ref() {
            Ok(engine) => engine,
            Err(error) => {
                let message = format!("{error:#}");
                if let Some(job) = job
                    && sender
                        .send_blocking(Event::Error {
                            generation: job.generation,
                            message: message.clone(),
                        })
                        .is_err()
                {
                    return;
                }
                if let Some(state) = active.take()
                    && sender
                        .send_blocking(Event::SearchFailed {
                            search: state.job.search,
                            message,
                        })
                        .is_err()
                {
                    return;
                }
                continue;
            }
        };
        if let Some(job) = job {
            let generation = job.generation;
            let current =
                || queue.running() && queue.generation.load(Ordering::SeqCst) == generation;
            if let Err(error) = render_job(engine, &mut documents, job, &current, &sender)
                && current()
                && sender
                    .send_blocking(Event::Error {
                        generation,
                        message: format!("{error:#}"),
                    })
                    .is_err()
            {
                return;
            }
            continue;
        }
        if let Some(state) = active.as_mut() {
            let search = state.job.search;
            let current =
                || queue.running() && queue.search_generation.load(Ordering::SeqCst) == search;
            let yielded = || queue.pending.lock().is_some() || queue.search.lock().is_some();
            match search_slice(engine, &mut documents, state, &current, &yielded, &sender) {
                Ok(true) => {}
                Ok(false) => active = None,
                Err(error) => {
                    active = None;
                    if current()
                        && sender
                            .send_blocking(Event::SearchFailed {
                                search,
                                message: format!("{error:#}"),
                            })
                            .is_err()
                    {
                        return;
                    }
                }
            }
        }
    }
}

fn render_view(document: &PdfDocument<'_>, view: &super::ViewKey) -> Result<super::Raster> {
    let page = document.pages().get(view.page as i32)?;
    let base_width = page.width().value * 96.0 / 72.0;
    let logical_width = view.zoom.map_or(view.available_width as f32, |zoom| {
        base_width * zoom as f32 / 100.0
    });
    render(&page, logical_width, view.scale as f32 / 100.0, 4096)
}

fn page_sizes(document: &PdfDocument<'_>) -> Result<Vec<(f32, f32)>> {
    let pages = document.pages();
    (0..pages.len())
        .map(|index| {
            let size = pages.page_size(index)?;
            Ok((size.width().value, size.height().value))
        })
        .collect()
}

fn render_job<'a>(
    engine: &'a Pdfium,
    documents: &mut Documents<'a>,
    job: Job,
    current: &dyn Fn() -> bool,
    sender: &async_channel::Sender<Event>,
) -> Result<()> {
    let generation = job.generation;
    let document = document(engine, documents, job.view.session, &job.path)?;
    crate::bench::mark("document_open");
    if !current() {
        return Ok(());
    }
    let count = document.pages().len() as usize;
    if count == 0 {
        bail!("This PDF has no pages to display.");
    }
    let sizes = if job.sizes {
        Some(Arc::new(page_sizes(document)?))
    } else {
        None
    };
    sender.send_blocking(Event::Metadata {
        generation,
        count,
        sizes,
    })?;
    if job.render_page && current() {
        crate::bench::mark("render_start");
        let raster = render_view(document, &job.view)?;
        crate::bench::mark("render_done");
        if current() {
            sender.send_blocking(Event::Page {
                generation,
                view: job.view.clone(),
                raster,
            })?;
        }
    }
    if job.extract_text && current() {
        let text = document
            .pages()
            .get(job.view.page as i32)
            .map_err(anyhow::Error::from)
            .and_then(|page| super::text::extract(&page, current))
            .map(Arc::new)
            .map_err(|_| "Text selection is unavailable on this page.".to_string());
        if current() {
            sender.send_blocking(Event::Text {
                generation,
                key: (job.view.session, job.view.page),
                text,
            })?;
        }
    }
    if job.outline && current() {
        sender.send_blocking(Event::Outline {
            session: job.view.session,
            outline: Arc::new(super::tools::outline(document)),
        })?;
    }
    for view in &job.nearby {
        if !current() {
            break;
        }
        if view.page >= count {
            continue;
        }
        if let Ok(raster) = render_view(document, view) {
            sender.send_blocking(Event::Page {
                generation,
                view: view.clone(),
                raster,
            })?;
        }
    }
    for &index in &job.nearby_text {
        if !current() {
            break;
        }
        if index >= count {
            continue;
        }
        let text = document
            .pages()
            .get(index as i32)
            .map_err(anyhow::Error::from)
            .and_then(|page| super::text::extract(&page, current))
            .map(Arc::new)
            .map_err(|_| "Text selection is unavailable on this page.".to_string());
        if current() {
            sender.send_blocking(Event::Text {
                generation,
                key: (job.view.session, index),
                text,
            })?;
        }
    }
    for index in job.thumbnails {
        if !current() {
            break;
        }
        if index >= count {
            continue;
        }
        let result = document
            .pages()
            .get(index as i32)
            .map_err(anyhow::Error::from)
            .and_then(|page| render(&page, 132.0, 2.0, 342));
        if current() {
            let event = match result {
                Ok(raster) => Event::Thumbnail {
                    generation,
                    page: index,
                    raster,
                },
                Err(_) => Event::ThumbnailFailed {
                    generation,
                    page: index,
                },
            };
            sender.send_blocking(event)?;
        }
    }
    Ok(())
}

fn search_slice<'a>(
    engine: &'a Pdfium,
    documents: &mut Documents<'a>,
    state: &mut ActiveSearch,
    current: &dyn Fn() -> bool,
    yielded: &dyn Fn() -> bool,
    sender: &async_channel::Sender<Event>,
) -> Result<bool> {
    let document = document(engine, documents, state.job.session, &state.job.path)?;
    let count = document.pages().len() as usize;
    let started = Instant::now();
    let mut pages = Vec::new();
    while state.page < count && state.hits < MAX_HITS && current() {
        match super::text::characters(&document.pages().get(state.page as i32)?) {
            Some(characters) => {
                let hits = search::find(&characters, &state.job.query, MAX_HITS - state.hits);
                state.hits += hits.len();
                if !hits.is_empty() {
                    pages.push((state.page, hits));
                }
            }
            None => state.skipped += 1,
        }
        state.page += 1;
        if started.elapsed() >= SEARCH_SLICE || yielded() {
            break;
        }
    }
    if !current() {
        return Ok(false);
    }
    sender.send_blocking(Event::SearchBatch {
        search: state.job.search,
        scanned: state.page,
        pages,
    })?;
    let truncated = state.hits >= MAX_HITS && state.page < count;
    if state.page < count && !truncated {
        return Ok(true);
    }
    sender.send_blocking(Event::SearchDone {
        search: state.job.search,
        skipped: state.skipped,
        truncated,
    })?;
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::{ViewKey, search::Query};
    use std::{collections::BTreeSet, path::PathBuf};

    fn next_event(events: &async_channel::Receiver<Event>) -> Event {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match events.try_recv() {
                Ok(event) => return event,
                Err(async_channel::TryRecvError::Closed) => panic!("Renderer exited unexpectedly"),
                Err(async_channel::TryRecvError::Empty) => {
                    assert!(Instant::now() < deadline, "Renderer timed out");
                    thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }

    fn search_fixture(
        worker: &Worker,
        events: &async_channel::Receiver<Event>,
        text: &str,
        match_case: bool,
    ) -> Vec<(usize, search::Hit)> {
        let id = worker.search(SearchJob {
            search: 0,
            session: 7,
            path: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reading.pdf"),
            query: Query {
                text: text.into(),
                match_case,
                whole_word: true,
            },
        });
        let mut hits = Vec::new();
        loop {
            match next_event(events) {
                Event::SearchBatch { search, pages, .. } if search == id => {
                    for (page, page_hits) in pages {
                        hits.extend(page_hits.into_iter().map(|hit| (page, hit)));
                    }
                }
                Event::SearchDone {
                    search,
                    skipped,
                    truncated,
                } if search == id => {
                    assert_eq!(skipped, 0);
                    assert!(!truncated);
                    return hits;
                }
                Event::SearchFailed { message, .. } => panic!("search failed: {message}"),
                _ => {}
            }
        }
    }

    #[test]
    #[ignore = "requires PDFium installed by scripts/setup_pdfium.py"]
    fn searches_fixture_across_lines_and_survives_page_renders() {
        let (worker, events) = Worker::start().unwrap();
        let phrase = search_fixture(&worker, &events, "step aside", false);
        assert_eq!(phrase.len(), 1);
        assert_eq!(phrase[0].0, 0);
        assert_eq!(phrase[0].1.matched, "step aside");

        let pages = |hits: &[(usize, search::Hit)]| {
            hits.iter().map(|(page, _)| *page).collect::<BTreeSet<_>>()
        };
        let folded = search_fixture(&worker, &events, "find", false);
        assert_eq!(pages(&folded), BTreeSet::from([0, 1, 2]));
        let exact = search_fixture(&worker, &events, "Find", true);
        assert_eq!(pages(&exact), BTreeSet::from([1, 2]));

        let id = worker.search(SearchJob {
            search: 0,
            session: 8,
            path: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reading.pdf"),
            query: Query {
                text: "page".into(),
                match_case: false,
                whole_word: false,
            },
        });
        let generation = worker.submit(Job {
            generation: 0,
            path: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reading.pdf"),
            view: ViewKey {
                session: 8,
                page: 2,
                available_width: 400,
                zoom: None,
                scale: 100,
            },
            render_page: true,
            extract_text: false,
            thumbnails: vec![],
            outline: false,
            sizes: false,
            nearby: vec![],
            nearby_text: vec![],
        });
        let (mut rendered, mut finished) = (false, false);
        while !(rendered && finished) {
            match next_event(&events) {
                Event::Page {
                    generation: page, ..
                } if page == generation => rendered = true,
                Event::SearchDone { search, .. } if search == id => finished = true,
                Event::SearchFailed { message, .. } | Event::Error { message, .. } => {
                    panic!("{message}")
                }
                _ => {}
            }
        }
    }

    #[test]
    fn newer_search_replaces_pending_search() {
        let queue = Arc::new(Queue::new());
        let worker = Worker(queue.clone());
        let job = |text: &str| SearchJob {
            search: 0,
            session: 1,
            path: PathBuf::from("sample.pdf"),
            query: Query {
                text: text.into(),
                match_case: false,
                whole_word: false,
            },
        };
        assert_eq!(worker.search(job("first")), 1);
        assert_eq!(worker.search(job("second")), 2);
        assert_eq!(queue.search.lock().as_ref().unwrap().query.text, "second");
        assert_eq!(worker.cancel_search(), 3);
        assert!(queue.search.lock().is_none());
        assert_eq!(queue.generation.load(Ordering::SeqCst), 0);
    }

    #[test]
    #[ignore = "requires PDFium installed by scripts/setup_pdfium.py"]
    fn renders_fixture_and_recovers_from_invalid_documents() {
        let (worker, events) = Worker::start().unwrap();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let make_job = |path, session| Job {
            generation: 0,
            path,
            view: ViewKey {
                session,
                page: 1,
                available_width: 660,
                zoom: None,
                scale: 100,
            },
            render_page: true,
            extract_text: true,
            thumbnails: vec![0, 2],
            outline: false,
            sizes: false,
            nearby: vec![],
            nearby_text: vec![],
        };
        let receive = || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                match events.try_recv() {
                    Ok(event) => return event,
                    Err(async_channel::TryRecvError::Closed) => {
                        panic!("Renderer exited unexpectedly")
                    }
                    Err(async_channel::TryRecvError::Empty) => {
                        assert!(std::time::Instant::now() < deadline, "Renderer timed out");
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                }
            }
        };
        let failed = worker.submit(make_job(root.join("README.md"), 1));
        assert!(matches!(receive(), Event::Error { generation, .. } if generation == failed));
        let recovered = worker.submit(make_job(root.join("tests/fixtures/reading.pdf"), 2));
        match receive() {
            Event::Metadata {
                generation,
                count: 3,
                ..
            } => assert_eq!(generation, recovered),
            Event::Error { message, .. } => panic!("Recovery failed: {message}"),
            _ => panic!("Expected document metadata"),
        }
        match receive() {
            Event::Page {
                generation,
                view,
                raster,
            } => {
                assert_eq!(generation, recovered);
                assert_eq!(view.page, 1);
                assert_raster(&raster, 4096);
            }
            _ => panic!("Expected the main page before thumbnails"),
        }
        match receive() {
            Event::Text {
                key,
                text: Ok(text),
                ..
            } => {
                assert_eq!(key, (2, 1));
                assert!(
                    text.content(0..text.glyphs.len())
                        .contains("Leave room in the margins")
                );
                assert!(text.glyphs.iter().any(|glyph| glyph.bounds.is_some()));
            }
            _ => panic!("Expected selectable text after the page"),
        }
        for expected in [0, 2] {
            match receive() {
                Event::Thumbnail { page, raster, .. } => {
                    assert_eq!(page, expected);
                    assert_raster(&raster, 342);
                }
                _ => panic!("Expected sidebar thumbnail"),
            }
        }
        let mut invalid_page = make_job(root.join("tests/fixtures/reading.pdf"), 2);
        invalid_page.view.page = 3;
        worker.submit(invalid_page);
        assert!(matches!(receive(), Event::Metadata { .. }));
        assert!(matches!(receive(), Event::Error { .. }));
        let mut retained = make_job(root.join("missing-file.pdf"), 2);
        retained.render_page = false;
        retained.thumbnails.clear();
        worker.submit(retained);
        assert!(matches!(receive(), Event::Metadata { count: 3, .. }));
        assert!(matches!(receive(), Event::Text { text: Ok(_), .. }));
        worker.close(2);
        worker.submit(make_job(root.join("missing-file.pdf"), 2));
        assert!(matches!(receive(), Event::Error { .. }));
        for session in 3..8 {
            let mut metadata_only = make_job(root.join("tests/fixtures/reading.pdf"), session);
            metadata_only.render_page = false;
            metadata_only.extract_text = false;
            metadata_only.thumbnails.clear();
            worker.submit(metadata_only);
            assert!(matches!(receive(), Event::Metadata { count: 3, .. }));
        }
        worker.submit(make_job(root.join("missing-file.pdf"), 3));
        assert!(matches!(receive(), Event::Error { .. }));
        let mut sized = make_job(root.join("tests/fixtures/selection.pdf"), 9);
        sized.render_page = false;
        sized.extract_text = false;
        sized.thumbnails.clear();
        sized.sizes = true;
        worker.submit(sized);
        match receive() {
            Event::Metadata {
                count: 3,
                sizes: Some(sizes),
                ..
            } => {
                assert_eq!(sizes[0], (612.0, 792.0));
                assert_eq!(sizes[1], (792.0, 612.0));
            }
            _ => panic!("Expected page sizes"),
        }
        let mut nearby = make_job(root.join("tests/fixtures/selection.pdf"), 9);
        nearby.render_page = false;
        nearby.extract_text = false;
        nearby.thumbnails.clear();
        nearby.nearby = vec![ViewKey {
            session: 9,
            page: 1,
            available_width: 400,
            zoom: None,
            scale: 100,
        }];
        nearby.nearby_text = vec![1];
        worker.submit(nearby);
        assert!(matches!(receive(), Event::Metadata { sizes: None, .. }));
        match receive() {
            Event::Page { view, raster, .. } => {
                assert_eq!(view.page, 1);
                assert_eq!(raster.logical_width, 400.0);
                assert!(raster.width > raster.height);
            }
            _ => panic!("Expected the nearby page"),
        }
        assert!(matches!(
            receive(),
            Event::Text {
                key: (9, 1),
                text: Ok(_),
                ..
            }
        ));
        for page in 0..3 {
            let mut job = make_job(root.join("tests/fixtures/selection.pdf"), 9);
            job.view.page = page;
            job.thumbnails.clear();
            worker.submit(job);
            assert!(matches!(receive(), Event::Metadata { count: 3, .. }));
            let raster = match receive() {
                Event::Page { raster, .. } => raster,
                _ => panic!("Expected fixture raster"),
            };
            let text = match receive() {
                Event::Text { text: Ok(text), .. } => text,
                _ => panic!("Expected fixture text"),
            };
            let content = text.content(0..text.glyphs.len());
            match page {
                0 => assert!(
                    content.contains("Café and naïve text keep their accents.\nCopy these lines")
                ),
                1 => {
                    assert!(content.contains("Rotated page"));
                    assert!(raster.width > raster.height);
                    let glyph = text
                        .glyphs
                        .iter()
                        .find(|glyph| glyph.bounds.is_some())
                        .unwrap();
                    assert!(glyph.direction[1] > 0.0);
                    assert!(glyph.direction[0].abs() < 0.00001);
                }
                _ => assert!(text.glyphs.is_empty()),
            }
            for glyph in &text.glyphs {
                let Some(bounds) = glyph.bounds else { continue };
                let left = (bounds.left * raster.width as f32).floor() as u32;
                let right = (bounds.right * raster.width as f32).ceil() as u32;
                let top = (bounds.top * raster.height as f32).floor() as u32;
                let bottom = (bounds.bottom * raster.height as f32).ceil() as u32;
                if !glyph.value.is_whitespace() {
                    assert!(
                        (top..bottom.min(raster.height)).any(|y| {
                            (left..right.min(raster.width)).any(|x| {
                                let offset = ((y * raster.width + x) * 4) as usize;
                                raster.bytes[offset..offset + 3]
                                    .iter()
                                    .any(|channel| *channel < 180)
                            })
                        }),
                        "Text geometry must overlap visible ink for {:?} on page {}",
                        glyph.value,
                        page
                    );
                }
            }
        }
    }

    fn assert_raster(raster: &super::super::Raster, limit: u32) {
        assert!(raster.width > 0 && raster.width <= limit);
        assert!(raster.height > 0 && raster.height <= limit);
        assert_eq!(
            raster.bytes.len(),
            (raster.width * raster.height * 4) as usize
        );
        assert!(
            raster
                .bytes
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[0] < 100)
        );
    }

    #[test]
    fn pending_render_is_replaced_by_newest_request() {
        let queue = Arc::new(Queue::new());
        let worker = Worker(queue.clone());
        for page in 0..100 {
            worker.submit(Job {
                generation: 0,
                path: PathBuf::from("sample.pdf"),
                view: ViewKey {
                    session: 1,
                    page,
                    available_width: 660,
                    zoom: None,
                    scale: 200,
                },
                render_page: true,
                extract_text: true,
                thumbnails: vec![],
                outline: false,
                sizes: false,
                nearby: vec![],
                nearby_text: vec![],
            });
        }
        let pending = queue.pending.lock();
        assert_eq!(pending.as_ref().unwrap().view.page, 99);
        assert_eq!(pending.as_ref().unwrap().generation, 100);
        drop(pending);
        assert_eq!(worker.cancel(), 101);
        assert!(queue.pending.lock().is_none());
    }
}
