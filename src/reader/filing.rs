use super::{Reader, chrome::Overlay};
use crate::pdf::{
    export::{Export, FileJob, FileTask, Format},
    markup::Edit,
};
use crate::ui::kit::Status;
use gpui::{Context, PathPromptOptions, Window};
use std::path::{Path, PathBuf};

pub(super) const MAX_COMBINED_FILES: usize = 100;
const DPI: [u32; 3] = [72, 150, 300];

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Filing {
    Export,
    Combine,
    Print(PrintOptions),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PrintOptions {
    pub fit: bool,
    pub both_sides: bool,
}

pub(super) fn downloads() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|home| PathBuf::from(home).join("Downloads"))
}

pub(super) fn vacant(folder: &Path, name: &str, extension: &str) -> PathBuf {
    let mut candidate = folder.join(format!("{name}.{extension}"));
    let mut attempt = 2;
    while candidate.exists() || numbered_sibling_exists(&candidate) {
        candidate = folder.join(format!("{name} {attempt}.{extension}"));
        attempt += 1;
    }
    candidate
}

fn numbered_sibling_exists(candidate: &Path) -> bool {
    let stem = candidate.file_stem().unwrap_or_default().to_string_lossy();
    let extension = candidate.extension().unwrap_or_default().to_string_lossy();
    candidate
        .with_file_name(format!("{stem} 1.{extension}"))
        .exists()
}

pub(super) fn notes_summary(title: &str, edits: &[Edit]) -> String {
    let mut summary = format!("# Notes on {title}\n\n");
    if edits.is_empty() {
        summary.push_str("No highlights, marks, or signatures were added in this session.\n");
    }
    for edit in edits {
        summary.push_str(&format!(
            "- p. {}, {}: {}\n",
            edit.first_page() + 1,
            edit.label(),
            edit.summary()
        ));
    }
    summary
}

pub(super) fn print_arguments(file: &Path, options: &PrintOptions) -> Vec<String> {
    let mut arguments = Vec::new();
    if options.fit {
        arguments.extend(["-o".to_string(), "fit-to-page".to_string()]);
    }
    arguments.extend([
        "-o".to_string(),
        if options.both_sides {
            "sides=two-sided-long-edge".to_string()
        } else {
            "sides=one-sided".to_string()
        },
    ]);
    arguments.push(file.to_string_lossy().into_owned());
    arguments
}

impl Reader {
    fn file(&mut self, task: FileTask, filing: Filing) {
        self.next_file += 1;
        self.filings.insert(self.next_file, filing);
        self.worker.file(FileJob {
            id: self.next_file,
            task,
        });
    }

    fn export_of(
        &self,
        format: Format,
        to: PathBuf,
        page: Option<usize>,
        marks: bool,
    ) -> Option<Export> {
        let tab = self.tabs.current()?;
        Some(Export {
            source: tab.path.clone(),
            format,
            page,
            dpi: DPI[self.chrome.export_dpi.min(DPI.len() - 1)],
            edits: if marks {
                tab.unsaved().to_vec()
            } else {
                Vec::new()
            },
            marks,
            to,
        })
    }

    pub(super) fn export_document(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.current().filter(|tab| tab.count > 0) else {
            return;
        };
        let Some(folder) = downloads().filter(|folder| folder.is_dir()) else {
            self.show_toast(
                Status::Warn,
                "The Downloads folder was not found",
                "Exports are written to Downloads in your home folder.",
                cx,
            );
            return;
        };
        let stem = tab
            .path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let page = (self.chrome.export_pages == 1).then_some(tab.page);
        let marks = self.chrome.include_notes;
        let format = match self.chrome.export_format {
            0 => Format::Pdf,
            1 => Format::Flattened,
            2 => Format::Png,
            3 => Format::Jpeg,
            4 => Format::Text,
            _ => {
                let to = vacant(&folder, &format!("{stem} notes"), "md");
                let summary = notes_summary(&tab.title(), &tab.edits);
                self.chrome.overlay = Overlay::None;
                match std::fs::write(&to, summary) {
                    Ok(()) => self.show_toast(
                        Status::Info,
                        "Notes summary exported",
                        &format!("Written to {}.", to.display()),
                        cx,
                    ),
                    Err(error) => self.show_toast(
                        Status::Warn,
                        "The notes summary could not be exported",
                        &format!("{}: {error}", to.display()),
                        cx,
                    ),
                }
                return;
            }
        };
        let name = match (format, page) {
            (Format::Flattened, _) => format!("{stem} flattened"),
            (_, Some(page)) => format!("{stem} page {}", page + 1),
            (Format::Pdf, None) => format!("{stem} copy"),
            (_, None) => stem,
        };
        let to = vacant(&folder, &name, format.extension());
        if let Some(export) = self.export_of(format, to, page, marks) {
            self.file(FileTask::Export(export), Filing::Export);
            self.chrome.overlay = Overlay::None;
            self.show_toast(
                Status::Info,
                "Exporting",
                "The files appear in Downloads when the export finishes.",
                cx,
            );
        }
    }

    pub(super) fn combine_sources(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = self
            .tabs
            .entries
            .iter()
            .map(|tab| tab.path.clone())
            .collect();
        for file in &self.combine_extra {
            if !files.contains(file) {
                files.push(file.clone());
            }
        }
        files
    }

    pub(super) fn add_combine_files(&mut self, files: Vec<PathBuf>, cx: &mut Context<Self>) {
        let mut skipped = false;
        for file in files {
            let pdf = file
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"));
            let file = file.canonicalize().unwrap_or(file);
            if !pdf || self.combine_sources().len() >= MAX_COMBINED_FILES {
                skipped = true;
            } else if !self.combine_sources().contains(&file) {
                self.combine_extra.push(file);
            }
        }
        if skipped {
            self.show_toast(
                Status::Info,
                "Some files were not added",
                &format!("Only PDFs can be combined, up to {MAX_COMBINED_FILES} at a time."),
                cx,
            );
        }
        cx.notify();
    }

    pub(super) fn choose_combine_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.picker_open {
            return;
        }
        self.picker_open = true;
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Add PDFs".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = chosen.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.picker_open = false;
                match result {
                    Ok(Ok(Some(files))) => this.add_combine_files(files, cx),
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

    pub(super) fn combine_to(&mut self, to: PathBuf, cx: &mut Context<Self>) {
        let files = self.combine_sources();
        if files.len() < 2 {
            self.show_toast(
                Status::Info,
                "Add another PDF first",
                "Combining needs at least two files.",
                cx,
            );
            return;
        }
        self.file(FileTask::Combine { files, to }, Filing::Combine);
        self.chrome.overlay = Overlay::None;
        cx.notify();
    }

    pub(super) fn choose_combine_destination(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.combine_sources().len() < 2 {
            self.show_toast(
                Status::Info,
                "Add another PDF first",
                "Combining needs at least two files.",
                cx,
            );
            return;
        }
        if self.picker_open {
            return;
        }
        self.picker_open = true;
        let folder = downloads().unwrap_or_default();
        let chosen = cx.prompt_for_new_path(&folder, Some("Combined.pdf"));
        cx.spawn_in(window, async move |this, cx| {
            let result = chosen.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.picker_open = false;
                match result {
                    Ok(Ok(Some(to))) => {
                        if to.exists() {
                            let _ = std::fs::remove_file(&to);
                        }
                        this.combine_to(to, cx)
                    }
                    Ok(Ok(None)) => {}
                    _ => this.show_toast(
                        Status::Warn,
                        "The save dialog could not be opened",
                        "Please try again.",
                        cx,
                    ),
                }
                this.focus.focus(window);
            });
        })
        .detach();
    }

    pub(super) fn print_document(&mut self, cx: &mut Context<Self>) {
        if !cfg!(target_os = "macos") {
            self.chrome.overlay = Overlay::None;
            self.unavailable("Printing on this platform", cx);
            return;
        }
        let Some(tab) = self.tabs.current().filter(|tab| tab.count > 0) else {
            return;
        };
        let page = (self.chrome.print_pages == 1).then_some(tab.page);
        let options = PrintOptions {
            fit: self.chrome.print_scale == 0,
            both_sides: self.chrome.print_sides == 1,
        };
        let spool = std::env::temp_dir().join(format!("agenticpdf-print-{}", std::process::id()));
        if let Err(error) = std::fs::create_dir_all(&spool) {
            self.show_toast(
                Status::Warn,
                "The document could not be prepared for printing",
                &error.to_string(),
                cx,
            );
            return;
        }
        let to = vacant(&spool, "print", "pdf");
        let marks = self.chrome.include_notes;
        if let Some(export) = self.export_of(Format::Pdf, to, page, marks) {
            self.file(FileTask::Export(export), Filing::Print(options));
            self.chrome.overlay = Overlay::None;
            cx.notify();
        }
    }

    fn send_to_printer(&mut self, file: PathBuf, options: PrintOptions, cx: &mut Context<Self>) {
        let arguments = print_arguments(&file, &options);
        cx.spawn(async move |this, cx| {
            let outcome = cx
                .background_executor()
                .spawn(async move {
                    let output = std::process::Command::new("lpr").args(arguments).output();
                    let _ = std::fs::remove_file(&file);
                    output
                })
                .await;
            let _ = this.update(cx, |this, cx| match outcome {
                Ok(output) if output.status.success() => this.show_toast(
                    Status::Info,
                    "Sent to the printer",
                    "The document was queued on your default printer.",
                    cx,
                ),
                Ok(output) => this.show_toast(
                    Status::Warn,
                    "The document could not be printed",
                    String::from_utf8_lossy(&output.stderr).trim(),
                    cx,
                ),
                Err(error) => this.show_toast(
                    Status::Warn,
                    "The document could not be printed",
                    &format!("The print command could not be started. {error}"),
                    cx,
                ),
            });
        })
        .detach();
    }

    pub(super) fn finish_file(
        &mut self,
        id: u64,
        result: Result<Vec<PathBuf>, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(filing) = self.filings.remove(&id) else {
            return;
        };
        match (filing, result) {
            (Filing::Print(options), Ok(written)) => {
                if let Some(file) = written.into_iter().next() {
                    self.send_to_printer(file, options, cx);
                }
            }
            (Filing::Print(_), Err(message)) => self.show_toast(
                Status::Warn,
                "The document could not be prepared for printing",
                &message,
                cx,
            ),
            (Filing::Export, Ok(written)) => {
                let place = written
                    .first()
                    .map(|file| file.display().to_string())
                    .unwrap_or_default();
                let (title, message) = if written.len() == 1 {
                    (
                        "Export finished".to_string(),
                        format!("Written to {place}."),
                    )
                } else {
                    (
                        format!("Exported {} files", written.len()),
                        format!("The first is {place}."),
                    )
                };
                self.show_toast(Status::Info, &title, &message, cx);
            }
            (Filing::Export, Err(message)) => {
                self.show_toast(Status::Warn, "The export failed", &message, cx)
            }
            (Filing::Combine, Ok(written)) => {
                self.combine_extra.clear();
                self.show_toast(
                    Status::Info,
                    "Files combined",
                    &format!(
                        "Written to {}.",
                        written
                            .first()
                            .map(|file| file.display().to_string())
                            .unwrap_or_default()
                    ),
                    cx,
                );
            }
            (Filing::Combine, Err(message)) => self.show_toast(
                Status::Warn,
                "The files could not be combined",
                &message,
                cx,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::markup::{Markup, MarkupKind};

    #[test]
    fn export_names_never_reuse_an_existing_file_or_numbered_set() {
        let folder = std::env::temp_dir().join(format!("agenticpdf-vacant-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        assert_eq!(vacant(&folder, "Report", "png"), folder.join("Report.png"));
        std::fs::write(folder.join("Report.png"), "").unwrap();
        assert_eq!(
            vacant(&folder, "Report", "png"),
            folder.join("Report 2.png")
        );
        std::fs::write(folder.join("Report 2 1.png"), "").unwrap();
        assert_eq!(
            vacant(&folder, "Report", "png"),
            folder.join("Report 3.png")
        );
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn notes_summaries_list_each_mark_with_its_page() {
        let empty = notes_summary("a.pdf", &[]);
        assert!(empty.starts_with("# Notes on a.pdf\n\n"));
        assert!(empty.contains("No highlights"));
        let summary = notes_summary(
            "a.pdf",
            &[Edit::Markup(Markup {
                kind: MarkupKind::Underline,
                color: 0,
                quote: "the margin was smaller".into(),
                pages: vec![crate::pdf::markup::PageMarks {
                    page: 1,
                    rects: Vec::new(),
                }],
            })],
        );
        assert!(summary.ends_with("- p. 2, Underline: the margin was smaller\n"));
    }

    #[test]
    fn print_arguments_carry_scale_and_sides_before_the_file() {
        assert_eq!(
            print_arguments(
                Path::new("/tmp/print.pdf"),
                &PrintOptions {
                    fit: true,
                    both_sides: true
                }
            ),
            [
                "-o",
                "fit-to-page",
                "-o",
                "sides=two-sided-long-edge",
                "/tmp/print.pdf"
            ]
        );
        assert_eq!(
            print_arguments(
                Path::new("/tmp/print.pdf"),
                &PrintOptions {
                    fit: false,
                    both_sides: false
                }
            ),
            ["-o", "sides=one-sided", "/tmp/print.pdf"]
        );
    }
}
