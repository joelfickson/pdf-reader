use super::{Event, Job, library::bind, render};
use anyhow::{Context, Result, bail};
use parking_lot::{Condvar, Mutex};
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
};

struct Queue {
    pending: Mutex<Option<Job>>,
    closed: Mutex<Vec<u64>>,
    ready: Condvar,
    generation: AtomicU64,
    stopped: AtomicBool,
}

pub struct Worker(Arc<Queue>);

impl Worker {
    #[cfg(test)]
    pub(crate) fn idle() -> Self {
        Self(Arc::new(Queue {
            pending: Mutex::new(None),
            closed: Mutex::new(Vec::new()),
            ready: Condvar::new(),
            generation: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
        }))
    }

    pub fn start() -> Result<(Self, async_channel::Receiver<Event>)> {
        let queue = Arc::new(Queue {
            pending: Mutex::new(None),
            closed: Mutex::new(Vec::new()),
            ready: Condvar::new(),
            generation: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
        });
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
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _pending = self.0.pending.lock();
        self.0.stopped.store(true, Ordering::SeqCst);
        self.0.ready.notify_one();
    }
}

fn run(queue: Arc<Queue>, sender: async_channel::Sender<Event>) {
    let engine = bind();
    let mut documents = VecDeque::new();
    loop {
        let (job, closed) = {
            let mut pending = queue.pending.lock();
            while pending.is_none()
                && queue.closed.lock().is_empty()
                && !queue.stopped.load(Ordering::SeqCst)
            {
                queue.ready.wait(&mut pending);
            }
            if queue.stopped.load(Ordering::SeqCst) {
                return;
            }
            let closed = std::mem::take(&mut *queue.closed.lock());
            (pending.take(), closed)
        };
        documents.retain(|(session, _)| !closed.contains(session));
        let Some(job) = job else { continue };
        let generation = job.generation;
        let current = || {
            !queue.stopped.load(Ordering::SeqCst)
                && queue.generation.load(Ordering::SeqCst) == generation
        };
        let result = (|| -> Result<()> {
            let engine = engine
                .as_ref()
                .map_err(|error| anyhow::anyhow!("{error:#}"))?;
            if let Some(index) = documents
                .iter()
                .position(|(session, _)| *session == job.view.session)
            {
                if let Some(document) = documents.remove(index) {
                    documents.push_back(document);
                }
            } else {
                let document = engine.load_pdf_from_file(&job.path, None).context(
                    "Could not open this PDF. It may be damaged, password-protected, or inaccessible.",
                )?;
                documents.push_back((job.view.session, document));
                while documents.len() > 4 {
                    documents.pop_front();
                }
            }
            if !current() {
                return Ok(());
            }
            let (_, document) = documents.back().context("No PDF document is open")?;
            let count = document.pages().len() as usize;
            if count == 0 {
                bail!("This PDF has no pages to display.");
            }
            sender.send_blocking(Event::Metadata { generation, count })?;
            if job.render_page && current() {
                let page = document.pages().get(job.view.page as i32)?;
                let base_width = page.width().value * 96.0 / 72.0;
                let logical_width = job
                    .view
                    .zoom
                    .map_or(job.view.available_width as f32, |zoom| {
                        base_width * zoom as f32 / 100.0
                    });
                let raster = render(&page, logical_width, job.view.scale as f32 / 100.0, 4096)?;
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
        })();
        if let Err(error) = result
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::ViewKey;
    use std::path::PathBuf;

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
        let queue = Arc::new(Queue {
            pending: Mutex::new(None),
            closed: Mutex::new(Vec::new()),
            ready: Condvar::new(),
            generation: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
        });
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
