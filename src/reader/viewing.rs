use super::{
    Reader,
    chrome::{InspectorTab, Layout, Mode, Overlay},
};
use crate::preferences::fit_page_zoom;
use gpui::{Context, Window};

const PRESENTATION_MARGIN: f32 = 48.0;

pub(super) struct Presenting {
    session: u64,
    layout: Layout,
    zoom: Option<u32>,
    sidebar: bool,
    inspector: Option<InspectorTab>,
    agent: bool,
    entered_fullscreen: bool,
}

impl Reader {
    fn relayout(&mut self, layout: Layout, window: &mut Window) {
        self.chrome.layout = layout;
        self.reset_request(window);
        for tab in &mut self.tabs.entries {
            tab.column = None;
            tab.pending_scroll = true;
            tab.restore_scroll = None;
        }
    }

    pub(super) fn toggle_presentation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(saved) = self.presenting.take() {
            self.sidebar = saved.sidebar;
            self.chrome.inspector = saved.inspector;
            self.agent.open = saved.agent;
            if let Some(tab) = self
                .tabs
                .entries
                .iter_mut()
                .find(|tab| tab.session == saved.session)
            {
                tab.zoom = saved.zoom;
            }
            self.relayout(saved.layout, window);
            if saved.entered_fullscreen && window.is_fullscreen() {
                window.toggle_fullscreen();
            }
            self.request(window);
            cx.notify();
            return;
        }
        let Some(tab) = self.tabs.current().filter(|tab| tab.count > 0) else {
            return;
        };
        let (session, zoom) = (tab.session, tab.zoom);
        if self.chrome.mode == Mode::Organize {
            self.cancel_page_changes(cx);
        }
        self.finish_placement(cx);
        self.chrome.mode = Mode::Read;
        self.chrome.overlay = Overlay::None;
        self.chrome.magnifier = false;
        if self.chrome.find_open {
            self.close_find(cx);
        }
        let entered_fullscreen = !window.is_fullscreen();
        self.presenting = Some(Presenting {
            session,
            layout: self.chrome.layout,
            zoom,
            sidebar: self.sidebar,
            inspector: self.chrome.inspector.take(),
            agent: self.agent.open,
            entered_fullscreen,
        });
        self.sidebar = false;
        self.agent.open = false;
        self.selection = None;
        self.relayout(Layout::Single, window);
        if entered_fullscreen {
            window.toggle_fullscreen();
        }
        self.request(window);
        cx.notify();
    }

    pub(super) fn fit_presentation(&mut self, window: &Window) {
        if self.presenting.is_none() {
            return;
        }
        let size = window.viewport_size();
        let available = (
            f32::from(size.width) - PRESENTATION_MARGIN,
            f32::from(size.height) - PRESENTATION_MARGIN,
        );
        let fitted = self
            .page_points()
            .and_then(|page| fit_page_zoom(available, page));
        if let (Some(zoom), Some(tab)) = (fitted, self.tabs.current_mut())
            && tab.zoom != Some(zoom)
        {
            tab.zoom = Some(zoom);
        }
    }

    pub(super) fn toggle_magnifier(&mut self, cx: &mut Context<Self>) {
        if self.tabs.current().is_none_or(|tab| tab.count == 0) || self.presenting.is_some() {
            return;
        }
        self.chrome.magnifier = !self.chrome.magnifier;
        self.chrome.overlay = Overlay::None;
        if !self.chrome.magnifier {
            self.pointer = None;
        }
        cx.notify();
    }
}
