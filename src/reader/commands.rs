use super::{
    Reader,
    actions::*,
    chrome::{InspectorTab, Mode, Overlay},
};
use crate::pdf::markup::MarkupKind;
use crate::ui::icon::Icon;
use gpui::{Context, Window};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Group {
    Navigate,
    Edit,
    View,
    File,
}

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Self::Navigate => "Suggested",
            Self::Edit => "Edit",
            Self::View => "View",
            Self::File => "File",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Command {
    GoToPage,
    Find,
    Bookmark,
    Annotate,
    Highlight,
    AddSignature,
    AddStamp,
    Organize,
    RotatePage,
    ToggleSidebar,
    Inspector,
    ZoomIn,
    ZoomOut,
    FitWidth,
    TwoPages,
    SplitView,
    NightPages,
    Magnifier,
    Presentation,
    Open,
    Export,
    Combine,
    Print,
    Save,
    DocumentInfo,
    Settings,
    CheckForUpdates,
    Shortcuts,
    CloseTab,
}

pub(super) fn modifier() -> &'static str {
    if cfg!(target_os = "macos") {
        "Cmd"
    } else {
        "Ctrl"
    }
}

pub(super) fn back_forward() -> String {
    if cfg!(target_os = "macos") {
        "Cmd [ ]".into()
    } else {
        "Alt Left Right".into()
    }
}

impl Command {
    pub const ALL: [Command; 29] = [
        Self::GoToPage,
        Self::Find,
        Self::Bookmark,
        Self::Annotate,
        Self::Highlight,
        Self::AddSignature,
        Self::AddStamp,
        Self::Organize,
        Self::RotatePage,
        Self::ToggleSidebar,
        Self::Inspector,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::FitWidth,
        Self::TwoPages,
        Self::SplitView,
        Self::NightPages,
        Self::Magnifier,
        Self::Presentation,
        Self::Open,
        Self::Export,
        Self::Combine,
        Self::Print,
        Self::Save,
        Self::DocumentInfo,
        Self::Settings,
        Self::CheckForUpdates,
        Self::Shortcuts,
        Self::CloseTab,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::GoToPage => "Go to page",
            Self::Find => "Find in document",
            Self::Bookmark => "Bookmark this page",
            Self::Annotate => "Annotate",
            Self::Highlight => "Highlight selection",
            Self::AddSignature => "Add signature",
            Self::AddStamp => "Add stamp",
            Self::Organize => "Organize pages",
            Self::RotatePage => "Rotate current page",
            Self::ToggleSidebar => "Toggle sidebar",
            Self::Inspector => "Toggle inspector",
            Self::ZoomIn => "Zoom in",
            Self::ZoomOut => "Zoom out",
            Self::FitWidth => "Fit to width",
            Self::TwoPages => "Two-page view",
            Self::SplitView => "Split view",
            Self::NightPages => "Night pages",
            Self::Magnifier => "Magnifier",
            Self::Presentation => "Presentation",
            Self::Open => "Open PDF",
            Self::Export => "Export as images or text",
            Self::Combine => "Combine files into one PDF",
            Self::Print => "Print",
            Self::Save => "Save",
            Self::DocumentInfo => "Document info",
            Self::Settings => "Settings",
            Self::CheckForUpdates => "Check for updates",
            Self::Shortcuts => "Keyboard shortcuts",
            Self::CloseTab => "Close tab",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Self::GoToPage => Icon::Goto,
            Self::Find => Icon::Search,
            Self::Bookmark => Icon::Bookmark,
            Self::Annotate => Icon::Pen,
            Self::Highlight => Icon::Highlight,
            Self::AddSignature => Icon::Sign,
            Self::AddStamp => Icon::Stamp,
            Self::Organize => Icon::Grid,
            Self::RotatePage => Icon::RotateRight,
            Self::ToggleSidebar => Icon::Sidebar,
            Self::Inspector | Self::DocumentInfo => Icon::Inspector,
            Self::ZoomIn => Icon::Plus,
            Self::ZoomOut => Icon::Minus,
            Self::FitWidth => Icon::Fit,
            Self::TwoPages => Icon::Layout,
            Self::SplitView => Icon::Split,
            Self::NightPages => Icon::Moon,
            Self::Magnifier => Icon::Magnifier,
            Self::Presentation => Icon::Present,
            Self::Open => Icon::File,
            Self::Export => Icon::Export,
            Self::Combine => Icon::Layout,
            Self::Print => Icon::Print,
            Self::Save => Icon::Save,
            Self::Settings => Icon::Settings,
            Self::CheckForUpdates => Icon::Redo,
            Self::Shortcuts => Icon::Keyboard,
            Self::CloseTab => Icon::Close,
        }
    }

    pub fn shortcut(self) -> Option<String> {
        let m = modifier();
        Some(match self {
            Self::GoToPage => format!("{m} G"),
            Self::Find => format!("{m} F"),
            Self::Bookmark => format!("{m} D"),
            Self::Annotate => format!("{m} Shift A"),
            Self::Organize => format!("{m} Shift O"),
            Self::ToggleSidebar => format!("{m} B"),
            Self::Inspector | Self::DocumentInfo => format!("{m} I"),
            Self::ZoomIn => format!("{m} ="),
            Self::ZoomOut => format!("{m} -"),
            Self::FitWidth => format!("{m} 0"),
            Self::SplitView => format!("{m} \\"),
            Self::Magnifier => format!("{m} Shift M"),
            Self::Presentation => format!("{m} Shift P"),
            Self::Open => format!("{m} O"),
            Self::Export => format!("{m} E"),
            Self::Print => format!("{m} P"),
            Self::Save => format!("{m} S"),
            Self::Settings => format!("{m} ,"),
            Self::Shortcuts => "?".into(),
            Self::CloseTab => format!("{m} W"),
            _ => return None,
        })
    }

    pub fn group(self) -> Group {
        match self {
            Self::GoToPage | Self::Find | Self::Bookmark => Group::Navigate,
            Self::Annotate
            | Self::Highlight
            | Self::AddSignature
            | Self::AddStamp
            | Self::Organize
            | Self::RotatePage => Group::Edit,
            Self::ToggleSidebar
            | Self::Inspector
            | Self::ZoomIn
            | Self::ZoomOut
            | Self::FitWidth
            | Self::TwoPages
            | Self::SplitView
            | Self::NightPages
            | Self::Magnifier
            | Self::Presentation => Group::View,
            _ => Group::File,
        }
    }

    fn needs_document(self) -> bool {
        !matches!(
            self,
            Self::Open
                | Self::Settings
                | Self::CheckForUpdates
                | Self::Shortcuts
                | Self::Combine
                | Self::ToggleSidebar
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Entry {
    Page(usize),
    Tab(usize),
    Command(Command),
}

pub(super) fn filter(query: &str, page_count: usize, tabs: &[String], active: usize) -> Vec<Entry> {
    let query = query.trim().to_lowercase();
    let mut entries = Vec::new();
    if let Ok(number) = query.parse::<usize>()
        && number >= 1
        && number <= page_count
    {
        entries.push(Entry::Page(number - 1));
    }
    entries.extend(
        Command::ALL
            .iter()
            .filter(|command| query.is_empty() || command.label().to_lowercase().contains(&query))
            .map(|command| Entry::Command(*command)),
    );
    entries.extend(
        tabs.iter()
            .enumerate()
            .filter(|(index, title)| {
                *index != active && !query.is_empty() && title.to_lowercase().contains(&query)
            })
            .map(|(index, _)| Entry::Tab(index)),
    );
    entries
}

impl Reader {
    pub(super) fn palette_entries(&self) -> Vec<Entry> {
        let titles: Vec<String> = self.tabs.entries.iter().map(|tab| tab.title()).collect();
        filter(
            &self.chrome.palette.text,
            self.tabs.current().map_or(0, |tab| tab.count),
            &titles,
            self.tabs.active,
        )
    }

    pub(super) fn run_entry(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        self.chrome.overlay = Overlay::None;
        self.chrome.input = super::chrome::Input::None;
        match entry {
            Entry::Page(page) => self.jump(page, None, window, cx),
            Entry::Tab(index) => self.activate_tab(index, window, cx),
            Entry::Command(command) => self.run_command(command, window, cx),
        }
        cx.notify();
    }

    pub(super) fn run_command(
        &mut self,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if command.needs_document() && self.tabs.current().is_none_or(|tab| tab.count == 0) {
            self.unavailable_reason(
                "Open a PDF first",
                "This command works on an open document.",
                cx,
            );
            return;
        }
        match command {
            Command::GoToPage => self.start_page_input(cx),
            Command::Find => self.open_find(cx),
            Command::Annotate => self.set_mode(Mode::Annotate, window, cx),
            Command::Organize => self.set_mode(Mode::Organize, window, cx),
            Command::AddSignature => self.set_overlay(Overlay::Sign, cx),
            Command::AddStamp => {
                if self.chrome.mode != Mode::Annotate {
                    self.set_mode(Mode::Annotate, window, cx);
                }
                self.chrome.tool = super::chrome::Tool::Stamp;
                self.set_overlay(Overlay::Stamps, cx);
            }
            Command::ToggleSidebar => window.dispatch_action(Box::new(ToggleSidebar), cx),
            Command::Inspector => self.set_inspector(InspectorTab::Selection, window, cx),
            Command::DocumentInfo => self.set_inspector(InspectorTab::Document, window, cx),
            Command::ZoomIn => window.dispatch_action(Box::new(ZoomIn), cx),
            Command::ZoomOut => window.dispatch_action(Box::new(ZoomOut), cx),
            Command::FitWidth => window.dispatch_action(Box::new(Fit), cx),
            Command::Open => window.dispatch_action(Box::new(Open), cx),
            Command::CloseTab => window.dispatch_action(Box::new(Close), cx),
            Command::Export => self.set_overlay(Overlay::Export, cx),
            Command::Combine => self.set_overlay(Overlay::Combine, cx),
            Command::Print => self.set_overlay(Overlay::Print, cx),
            Command::Settings => self.set_overlay(Overlay::Settings, cx),
            Command::CheckForUpdates => window.dispatch_action(Box::new(CheckForUpdates), cx),
            Command::Shortcuts => self.set_overlay(Overlay::Shortcuts, cx),
            Command::Bookmark => self.toggle_bookmark(cx),
            Command::Highlight => {
                self.mark_selection(MarkupKind::Highlight, self.chrome.marker, cx)
            }
            Command::RotatePage => self.rotate_current_page(cx),
            Command::TwoPages => {
                let layout = if self.chrome.layout == super::chrome::Layout::TwoPages {
                    super::chrome::Layout::Continuous
                } else {
                    super::chrome::Layout::TwoPages
                };
                self.set_layout(layout, window, cx)
            }
            Command::SplitView => self.unavailable("Split view", cx),
            Command::NightPages => {
                let colour = if self.chrome.page_colour == super::chrome::PageColour::Night {
                    super::chrome::PageColour::Original
                } else {
                    super::chrome::PageColour::Night
                };
                self.set_page_colour(colour, window, cx)
            }
            Command::Magnifier => self.toggle_magnifier(cx),
            Command::Presentation => self.toggle_presentation(window, cx),
            Command::Save if self.chrome.mode == Mode::Organize => {
                self.apply_page_changes(window, cx)
            }
            Command::Save => self.save_document(cx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_query_offers_page_and_filters_commands() {
        let tabs = vec!["a.pdf".to_string(), "report.pdf".to_string()];
        let entries = filter("3", 10, &tabs, 0);
        assert_eq!(entries.first(), Some(&Entry::Page(2)));
        assert!(
            filter("99", 10, &tabs, 0)
                .iter()
                .all(|entry| !matches!(entry, Entry::Page(_)))
        );
        let zoom = filter("zoom", 10, &tabs, 0);
        assert_eq!(
            zoom,
            vec![
                Entry::Command(Command::ZoomIn),
                Entry::Command(Command::ZoomOut)
            ]
        );
        assert_eq!(filter("report", 0, &tabs, 0), vec![Entry::Tab(1)]);
        assert!(filter("report", 0, &tabs, 1).is_empty());
        assert_eq!(filter("", 0, &tabs, 0).len(), Command::ALL.len());
    }
}
