use super::{
    Event, Job, OrganizeJob, SaveJob, SearchJob,
    export::{self, FileJob, FileTask},
    library::bind,
    markup,
    pages::{self, PageOp},
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
    saves: Mutex<VecDeque<SaveJob>>,
    organizing: Mutex<VecDeque<OrganizeJob>>,
    filing: Mutex<VecDeque<FileJob>>,
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
            saves: Mutex::new(VecDeque::new()),
            organizing: Mutex::new(VecDeque::new()),
            filing: Mutex::new(VecDeque::new()),
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

    pub fn save(&self, job: SaveJob) {
        let _pending = self.0.pending.lock();
        self.0.saves.lock().push_back(job);
        self.0.ready.notify_one();
    }

    pub fn file(&self, job: FileJob) {
        let _pending = self.0.pending.lock();
        self.0.filing.lock().push_back(job);
        self.0.ready.notify_one();
    }

    #[cfg(test)]
    pub(crate) fn queued_files(&self) -> Vec<FileTask> {
        self.0
            .filing
            .lock()
            .iter()
            .map(|job| job.task.clone())
            .collect()
    }

    pub fn organize(&self, job: OrganizeJob) {
        let _pending = self.0.pending.lock();
        self.0.organizing.lock().push_back(job);
        self.0.ready.notify_one();
    }

    #[cfg(test)]
    pub(crate) fn queued_page_ops(&self) -> Vec<PageOp> {
        self.0
            .organizing
            .lock()
            .iter()
            .map(|job| job.op.clone())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn queued_saves(&self) -> Vec<usize> {
        self.0
            .saves
            .lock()
            .iter()
            .map(|job| job.edits.len())
            .collect()
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

#[derive(Default)]
struct Open<'a> {
    cached: Documents<'a>,
    edited: Documents<'a>,
}

impl<'a> Open<'a> {
    fn forget(&mut self, session: u64) {
        self.cached.retain(|(open, _)| *open != session);
        self.edited.retain(|(open, _)| *open != session);
    }

    fn edit(
        &mut self,
        engine: &'a Pdfium,
        session: u64,
        path: &Path,
    ) -> Result<&mut PdfDocument<'a>> {
        if !self.edited.iter().any(|(open, _)| *open == session) {
            let entry = match self.cached.iter().position(|(open, _)| *open == session) {
                Some(index) => self.cached.remove(index),
                None => None,
            };
            let entry = match entry {
                Some(entry) => entry,
                None => (
                    session,
                    engine.load_pdf_from_file(path, None).context(
                        "Could not open this PDF. It may be damaged, password-protected, or inaccessible.",
                    )?,
                ),
            };
            self.edited.push_back(entry);
        }
        self.edited
            .iter_mut()
            .find(|(open, _)| *open == session)
            .map(|(_, document)| document)
            .context("No PDF document is open")
    }
}

fn document<'a, 'b>(
    engine: &'a Pdfium,
    open: &'b mut Open<'a>,
    session: u64,
    path: &Path,
) -> Result<&'b PdfDocument<'a>> {
    if let Some(index) = open.edited.iter().position(|(known, _)| *known == session) {
        return Ok(&open.edited[index].1);
    }
    let documents = &mut open.cached;
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
    let mut documents = Open::default();
    let mut active: Option<ActiveSearch> = None;
    let mut exporting: Option<export::Active<'_>> = None;
    let mut backlog: VecDeque<FileJob> = VecDeque::new();
    loop {
        let (job, closed, search, saves, organizing) = {
            let mut pending = queue.pending.lock();
            while pending.is_none()
                && active.is_none()
                && queue.search.lock().is_none()
                && queue.saves.lock().is_empty()
                && queue.organizing.lock().is_empty()
                && queue.filing.lock().is_empty()
                && exporting.is_none()
                && backlog.is_empty()
                && queue.closed.lock().is_empty()
                && queue.running()
            {
                queue.ready.wait(&mut pending);
            }
            if !queue.running() {
                return;
            }
            let closed = std::mem::take(&mut *queue.closed.lock());
            let saves = std::mem::take(&mut *queue.saves.lock());
            let organizing = std::mem::take(&mut *queue.organizing.lock());
            backlog.extend(std::mem::take(&mut *queue.filing.lock()));
            (
                pending.take(),
                closed,
                queue.search.lock().take(),
                saves,
                organizing,
            )
        };
        for session in &closed {
            documents.forget(*session);
        }
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
                for save in saves {
                    if sender
                        .send_blocking(Event::Saved {
                            session: save.session,
                            result: Err(message.clone()),
                        })
                        .is_err()
                    {
                        return;
                    }
                }
                for job in organizing {
                    if sender
                        .send_blocking(Event::Organized {
                            session: job.session,
                            result: Err(message.clone()),
                        })
                        .is_err()
                    {
                        return;
                    }
                }
                for job in backlog.drain(..) {
                    if sender
                        .send_blocking(Event::Filed {
                            id: job.id,
                            result: Err(message.clone()),
                        })
                        .is_err()
                    {
                        return;
                    }
                }
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
        for job in organizing {
            let result =
                organize(engine, &mut documents, &job).map_err(|error| format!("{error:#}"));
            if sender
                .send_blocking(Event::Organized {
                    session: job.session,
                    result,
                })
                .is_err()
            {
                return;
            }
        }
        for save in saves {
            documents.forget(save.session);
            let result =
                markup::save(engine, &save.path, &save.edits).map_err(|error| format!("{error:#}"));
            if sender
                .send_blocking(Event::Saved {
                    session: save.session,
                    result,
                })
                .is_err()
            {
                return;
            }
        }
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
        if exporting.is_none()
            && let Some(job) = backlog.pop_front()
        {
            let started = match job.task {
                FileTask::Combine { files, to } => Err(export::combine(engine, &files, &to)),
                FileTask::Export(export) => {
                    export::Active::start(engine, job.id, export).map_err(Err)
                }
            };
            match started {
                Ok(started) => exporting = Some(started),
                Err(result) => {
                    if sender
                        .send_blocking(Event::Filed {
                            id: job.id,
                            result: result.map_err(|error| format!("{error:#}")),
                        })
                        .is_err()
                    {
                        return;
                    }
                }
            }
        }
        if let Some(running) = exporting.as_mut() {
            let id = running.id;
            let finished = match running.step() {
                Ok(None) => None,
                Ok(Some(written)) => Some(Ok(written)),
                Err(error) => Some(Err(format!("{error:#}"))),
            };
            if let Some(result) = finished {
                if let Some(stopped) = exporting.take()
                    && result.is_err()
                {
                    stopped.abandon();
                }
                if sender.send_blocking(Event::Filed { id, result }).is_err() {
                    return;
                }
            }
        }
    }
}

fn organize<'a>(engine: &'a Pdfium, open: &mut Open<'a>, job: &OrganizeJob) -> Result<usize> {
    let session = job.session;
    match &job.op {
        PageOp::Cancel => open.edited.retain(|(known, _)| *known != session),
        PageOp::Apply => {
            let index = open
                .edited
                .iter()
                .position(|(known, _)| *known == session)
                .context("There are no page changes to apply.")?;
            let staged = pages::write(&open.edited[index].1, &job.path)?;
            open.forget(session);
            staged.commit(&job.path)?;
        }
        PageOp::Extract { pages, to } => {
            let document = document(engine, open, session, &job.path)?;
            pages::extract(engine, document, pages, to)?;
        }
        PageOp::Rotate { pages, turns } => {
            pages::rotate(open.edit(engine, session, &job.path)?, pages, *turns)?
        }
        PageOp::Delete { pages } => pages::delete(open.edit(engine, session, &job.path)?, pages)?,
        PageOp::Insert { file } => {
            pages::insert(engine, open.edit(engine, session, &job.path)?, file)?
        }
    }
    Ok(document(engine, open, session, &job.path)?.pages().len() as usize)
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
    documents: &mut Open<'a>,
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
    documents: &mut Open<'a>,
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
    #[ignore = "requires PDFium installed by scripts/setup_pdfium.py"]
    fn save_writes_marks_and_the_next_render_shows_them() {
        use crate::pdf::{
            SaveJob,
            markup::{Edit, Markup, MarkupKind, PageMarks, lines},
        };

        let directory =
            std::env::temp_dir().join(format!("agenticpdf-worker-save-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("saved.pdf");
        std::fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reading.pdf"),
            &path,
        )
        .unwrap();
        let (worker, events) = Worker::start().unwrap();
        let job = || Job {
            generation: 0,
            path: path.clone(),
            view: ViewKey {
                session: 4,
                page: 0,
                available_width: 600,
                zoom: None,
                scale: 100,
            },
            render_page: true,
            extract_text: true,
            thumbnails: vec![],
            outline: false,
            sizes: false,
            nearby: vec![],
            nearby_text: vec![],
        };
        let load = |generation: u64| loop {
            match next_event(&events) {
                Event::Page {
                    generation: page,
                    raster,
                    ..
                } if page == generation => loop {
                    match next_event(&events) {
                        Event::Text {
                            generation: page,
                            text: Ok(text),
                            ..
                        } if page == generation => return (raster, text),
                        Event::Error { message, .. } => panic!("{message}"),
                        _ => {}
                    }
                },
                Event::Error { message, .. } => panic!("{message}"),
                _ => {}
            }
        };
        let chromatic = |raster: &crate::pdf::Raster, rect: &crate::pdf::text::TextRect| {
            let columns = (rect.left * raster.width as f32).ceil() as u32
                ..(rect.right * raster.width as f32).floor() as u32;
            let rows = (rect.top * raster.height as f32).ceil() as u32
                ..(rect.bottom * raster.height as f32).floor() as u32;
            let total = (columns.len() * rows.len()).max(1);
            let tinted = rows
                .flat_map(|y| columns.clone().map(move |x| (x, y)))
                .filter(|(x, y)| {
                    let offset = ((y * raster.width + x) * 4) as usize;
                    let pixel = &raster.bytes[offset..offset + 3];
                    pixel.iter().max().unwrap() - pixel.iter().min().unwrap() > 60
                })
                .count();
            tinted as f32 / total as f32
        };
        let (before, text) = load(worker.submit(job()));
        let rects = lines(&text.glyphs[..text.glyphs.len().min(20)]);
        assert!(!rects.is_empty());
        assert!(chromatic(&before, &rects[0]) < 0.1);
        worker.save(SaveJob {
            session: 4,
            path: path.clone(),
            edits: vec![Edit::Markup(Markup {
                kind: MarkupKind::Highlight,
                color: 0xf2c230,
                quote: text.content(0..20),
                pages: vec![PageMarks {
                    page: 0,
                    rects: rects.clone(),
                }],
            })],
        });
        loop {
            match next_event(&events) {
                Event::Saved { session, result } => {
                    assert_eq!(session, 4);
                    result.unwrap();
                    break;
                }
                Event::Error { message, .. } => panic!("{message}"),
                _ => {}
            }
        }
        let (after, _) = load(worker.submit(job()));
        assert!(chromatic(&after, &rects[0]) > 0.5);
        worker.save(SaveJob {
            session: 4,
            path: directory.join("missing.pdf"),
            edits: vec![],
        });
        loop {
            if let Event::Saved { result, .. } = next_event(&events) {
                assert!(result.is_err());
                break;
            }
        }
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    #[ignore = "requires PDFium installed by scripts/setup_pdfium.py"]
    fn page_changes_stay_in_memory_until_applied_and_cancel_discards_them() {
        use crate::pdf::OrganizeJob;

        let directory =
            std::env::temp_dir().join(format!("agenticpdf-organize-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let path = directory.join("pages.pdf");
        std::fs::copy(fixtures.join("reading.pdf"), &path).unwrap();
        let original = std::fs::read(&path).unwrap();
        let (worker, events) = Worker::start().unwrap();
        let run = |op: PageOp| {
            worker.organize(OrganizeJob {
                session: 6,
                path: path.clone(),
                op,
            });
            loop {
                if let Event::Organized { session, result } = next_event(&events) {
                    assert_eq!(session, 6);
                    return result;
                }
            }
        };
        let shape = |page: usize| {
            let generation = worker.submit(Job {
                generation: 0,
                path: path.clone(),
                view: ViewKey {
                    session: 6,
                    page,
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
            loop {
                match next_event(&events) {
                    Event::Page {
                        generation: page,
                        raster,
                        ..
                    } if page == generation => return raster.width > raster.height,
                    Event::Error { message, .. } => panic!("{message}"),
                    _ => {}
                }
            }
        };
        assert!(!shape(0));
        assert_eq!(
            run(PageOp::Rotate {
                pages: vec![0],
                turns: 1
            }),
            Ok(3)
        );
        assert!(shape(0));
        assert!(!shape(1));
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(run(PageOp::Cancel), Ok(3));
        assert!(!shape(0));

        assert!(run(PageOp::Apply).is_err());
        assert!(
            run(PageOp::Delete {
                pages: vec![0, 1, 2]
            })
            .is_err()
        );
        assert!(run(PageOp::Delete { pages: vec![7] }).is_err());
        assert_eq!(run(PageOp::Cancel), Ok(3));

        assert_eq!(run(PageOp::Delete { pages: vec![1] }), Ok(2));
        assert_eq!(
            run(PageOp::Insert {
                file: fixtures.join("selection.pdf")
            }),
            Ok(5)
        );
        assert!(
            run(PageOp::Insert {
                file: fixtures.join("missing.pdf")
            })
            .is_err()
        );
        assert_eq!(
            run(PageOp::Rotate {
                pages: vec![4, 0],
                turns: 3
            }),
            Ok(5)
        );
        let extracted = directory.join("extracted.pdf");
        assert_eq!(
            run(PageOp::Extract {
                pages: vec![3, 0],
                to: extracted.clone()
            }),
            Ok(5)
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(run(PageOp::Apply), Ok(5));
        assert_ne!(std::fs::read(&path).unwrap(), original);
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 2);
        assert!(shape(0));
        drop(worker);
        let engine = bind().unwrap();
        assert_eq!(
            engine
                .load_pdf_from_file(&path, None)
                .unwrap()
                .pages()
                .len(),
            5
        );
        let pulled = engine.load_pdf_from_file(&extracted, None).unwrap();
        assert_eq!(pulled.pages().len(), 2);
        let first = pulled.pages().get(0).unwrap();
        assert!(first.width().value > first.height().value);
        drop(pulled);
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    #[ignore = "requires PDFium installed by scripts/setup_pdfium.py"]
    fn exports_and_combines_files_without_blocking_renders() {
        use crate::pdf::{
            export::{Export, FileJob, FileTask, Format},
            markup::{Edit, Signature},
            text::TextRect,
        };

        let directory =
            std::env::temp_dir().join(format!("agenticpdf-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let source = fixtures.join("reading.pdf");
        let (worker, events) = Worker::start().unwrap();
        let mut next = 0;
        let mut file = |task: FileTask| {
            next += 1;
            worker.file(FileJob { id: next, task });
            loop {
                if let Event::Filed { id, result } = next_event(&events) {
                    assert_eq!(id, next);
                    return result;
                }
            }
        };
        let signature = Edit::Signature(Signature {
            page: 0,
            bounds: TextRect {
                left: 0.3,
                top: 0.8,
                right: 0.6,
                bottom: 0.9,
            },
            color: 0x1c3d8f,
            strokes: Arc::new(vec![vec![[0.0, 0.0], [1.0, 1.0]]]),
            kind: crate::pdf::markup::SketchKind::Signature,
            weight: None,
        });
        let export = |format: Format, page: Option<usize>, marks: bool, name: &str| Export {
            source: source.clone(),
            format,
            page,
            dpi: 72,
            edits: vec![signature.clone()],
            marks,
            to: directory.join(name),
        };
        let original = std::fs::read(&source).unwrap();

        let copy = export(Format::Pdf, None, true, "copy.pdf");
        assert_eq!(
            file(FileTask::Export(copy.clone())),
            Ok(vec![copy.to.clone()])
        );
        assert!(file(FileTask::Export(copy.clone())).is_err());
        let flat = export(Format::Flattened, Some(0), true, "flat.pdf");
        assert_eq!(
            file(FileTask::Export(flat.clone())),
            Ok(vec![flat.to.clone()])
        );
        let plain = export(Format::Pdf, Some(0), false, "plain.pdf");
        file(FileTask::Export(plain.clone())).unwrap();

        let images = export(Format::Png, None, true, "pages.png");
        let written = file(FileTask::Export(images)).unwrap();
        assert_eq!(
            written,
            (1..=3)
                .map(|page| directory.join(format!("pages {page}.png")))
                .collect::<Vec<_>>()
        );
        let single = export(Format::Jpeg, Some(1), false, "second.jpg");
        assert_eq!(
            file(FileTask::Export(single.clone())),
            Ok(vec![single.to.clone()])
        );
        let png = std::fs::read(&written[0]).unwrap();
        assert_eq!(png[..8], [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 612);
        assert_eq!(std::fs::read(&single.to).unwrap()[..3], [0xff, 0xd8, 0xff]);

        let text = export(Format::Text, None, true, "all.txt");
        file(FileTask::Export(text.clone())).unwrap();
        let content = std::fs::read_to_string(&text.to).unwrap();
        assert!(content.contains("Leave room in the margins"));
        assert!(content.contains("\n\n"));
        assert!(
            file(FileTask::Export(export(
                Format::Text,
                Some(9),
                true,
                "missing.txt"
            )))
            .is_err()
        );

        let combined = directory.join("combined.pdf");
        assert_eq!(
            file(FileTask::Combine {
                files: vec![
                    source.clone(),
                    fixtures.join("selection.pdf"),
                    copy.to.clone()
                ],
                to: combined.clone()
            }),
            Ok(vec![combined.clone()])
        );
        assert!(
            file(FileTask::Combine {
                files: vec![source.clone()],
                to: directory.join("one.pdf")
            })
            .is_err()
        );
        assert!(
            file(FileTask::Combine {
                files: vec![source.clone(), fixtures.join("missing.pdf")],
                to: directory.join("broken.pdf")
            })
            .is_err()
        );
        assert_eq!(std::fs::read(&source).unwrap(), original);
        drop(worker);

        let engine = bind().unwrap();
        let kinds = |path: &PathBuf| {
            let document = engine.load_pdf_from_file(path, None).unwrap();
            let count = document.pages().len();
            let first = document.pages().get(0).unwrap();
            let kinds: Vec<_> = first
                .annotations()
                .iter()
                .map(|annotation| annotation.annotation_type())
                .collect();
            (count, kinds)
        };
        assert_eq!(kinds(&copy.to), (3, vec![PdfPageAnnotationType::Stamp]));
        assert_eq!(kinds(&flat.to), (1, vec![]));
        assert_eq!(kinds(&plain.to), (1, vec![]));
        assert_eq!(kinds(&combined).0, 9);
        let flattened = engine.load_pdf_from_file(&flat.to, None).unwrap();
        let raster = render(&flattened.pages().get(0).unwrap(), 800.0, 1.0, 4096).unwrap();
        let inked = (0..raster.width * raster.height)
            .filter(|index| {
                let x = index % raster.width;
                let y = index / raster.width;
                let inside = (0.3..0.6).contains(&(x as f32 / raster.width as f32))
                    && (0.8..0.9).contains(&(y as f32 / raster.height as f32));
                let pixel = &raster.bytes[(*index * 4) as usize..(*index * 4 + 3) as usize];
                inside && pixel.iter().max().unwrap() - pixel.iter().min().unwrap() > 60
            })
            .count();
        assert!(
            inked > 20,
            "the flattened signature must still be drawn: {inked}"
        );
        drop(flattened);
        std::fs::remove_dir_all(&directory).unwrap();
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
