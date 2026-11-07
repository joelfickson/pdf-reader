use super::{Reader, actions::*, chrome::Input};
use crate::agent::{
    catalog::{self, Availability, Detected},
    conversation::{self, Conversation, SUMMARY_REQUEST},
    session::{Event, Launch, Prompt, Session},
};
use crate::ui::field::{FieldKey, TextField};
use gpui::{AppContext, ClipboardItem, Context, Keystroke, Window};
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Connection {
    Idle,
    Detecting,
    Starting,
    Ready,
    Working,
    Failed(String),
}

pub(super) struct AgentPanel {
    pub open: bool,
    pub agents: Option<Vec<Detected>>,
    pub selected: Option<usize>,
    pub choosing: bool,
    pub name: String,
    pub connection: Connection,
    pub conversation: Conversation,
    pub composer: TextField,
    path: OsString,
    session: Option<Session>,
    epoch: u64,
}

impl Default for AgentPanel {
    fn default() -> Self {
        Self {
            open: false,
            agents: None,
            selected: None,
            choosing: false,
            name: "Agent".into(),
            connection: Connection::Idle,
            conversation: Conversation::default(),
            composer: TextField::default(),
            path: OsString::new(),
            session: None,
            epoch: 0,
        }
    }
}

impl AgentPanel {
    pub fn ready(&self) -> bool {
        self.connection == Connection::Ready
    }
}

impl Reader {
    pub(super) fn toggle_agent(
        &mut self,
        _: &ToggleAgent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.agent.open = !self.agent.open;
        if self.agent.open {
            self.chrome.inspector = None;
            self.chrome.input = Input::Agent;
            self.detect_agents(cx);
        } else if self.chrome.input == Input::Agent {
            self.chrome.input = Input::None;
        }
        self.focus.focus(window);
        self.request(window);
        cx.notify();
    }

    fn detect_agents(&mut self, cx: &mut Context<Self>) {
        if self.agent.agents.is_some() || self.agent.connection == Connection::Detecting {
            return;
        }
        self.agent.connection = Connection::Detecting;
        let scan = cx.background_spawn(async {
            let path = catalog::search_path();
            let agents = catalog::detect(&path);
            (path, agents)
        });
        cx.spawn(async move |this, cx| {
            let (path, agents) = scan.await;
            let _ = this.update(cx, |this, cx| {
                let first = agents
                    .iter()
                    .position(|agent| matches!(agent.availability, Availability::Ready(_)));
                this.agent.path = path;
                this.agent.agents = Some(agents);
                this.agent.connection = Connection::Idle;
                if let Some(index) = first {
                    this.start_agent(index, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn start_agent(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(detected) = self
            .agent
            .agents
            .as_ref()
            .and_then(|agents| agents.get(index))
        else {
            return;
        };
        let Some(args) = detected.launch_args() else {
            return;
        };
        let name = detected.spec.name.to_string();
        self.agent.session = None;
        self.agent.epoch += 1;
        self.agent.selected = Some(index);
        self.agent.choosing = false;
        self.agent.name = name;
        let mcp = match std::env::current_exe() {
            Ok(path) => path,
            Err(error) => {
                self.agent.connection =
                    Connection::Failed(format!("The reader could not locate itself. {error}"));
                return;
            }
        };
        let cwd = self
            .tabs
            .current()
            .and_then(|tab| tab.path.parent().map(PathBuf::from))
            .unwrap_or_else(std::env::temp_dir);
        let launch = Launch {
            args,
            path: self.agent.path.clone(),
            cwd,
            mcp,
        };
        match Session::start(launch) {
            Ok((session, events)) => {
                self.agent.session = Some(session);
                self.agent.connection = Connection::Starting;
                let epoch = self.agent.epoch;
                cx.spawn(async move |this, cx| {
                    while let Ok(event) = events.recv().await {
                        if this
                            .update(cx, |this, cx| this.receive_agent(epoch, event, cx))
                            .is_err()
                        {
                            break;
                        }
                    }
                })
                .detach();
            }
            Err(error) => {
                self.agent.connection = Connection::Failed(format!("{error:#}"));
            }
        }
        cx.notify();
    }

    fn receive_agent(&mut self, epoch: u64, event: Event, cx: &mut Context<Self>) {
        if epoch != self.agent.epoch {
            return;
        }
        match event {
            Event::Connected(name) => {
                self.agent.name = name;
                self.agent.connection = Connection::Ready;
                #[cfg(debug_assertions)]
                self.preview_after_agent_ready(cx);
            }
            Event::Update(update) => self.agent.conversation.apply(update),
            Event::Permission {
                request,
                title,
                choices,
            } => self.agent.conversation.permission(request, title, choices),
            Event::TurnEnded(stop) => {
                self.agent.conversation.finish_turn();
                match stop.as_str() {
                    "EndTurn" => {}
                    "Cancelled" => self.agent.conversation.notice("Stopped."),
                    other => self
                        .agent
                        .conversation
                        .notice(format!("The agent stopped early: {other}.")),
                }
                self.agent.connection = Connection::Ready;
            }
            Event::Failed(message) => {
                self.agent.session = None;
                self.agent.connection = Connection::Failed(message);
            }
        }
        cx.notify();
    }

    pub(super) fn ask_agent(&mut self, question: Option<String>, cx: &mut Context<Self>) {
        let (Some(session), Some(tab)) = (self.agent.session.as_ref(), self.tabs.current()) else {
            return;
        };
        if !self.agent.ready() {
            return;
        }
        let title = tab.title();
        let (display, text, context) = match question {
            Some(question) => (
                question.clone(),
                conversation::question_prompt(&question, &tab.path, tab.page + 1),
                format!("{title}, page {}", tab.page + 1),
            ),
            None => (
                SUMMARY_REQUEST.to_string(),
                conversation::summary_prompt(&tab.path),
                format!("{title}, {} pages", tab.count),
            ),
        };
        let prompt = Prompt {
            text,
            file: Some((title, conversation::file_uri(&tab.path))),
        };
        if session.prompt(prompt) {
            self.agent.conversation.ask(&display, &context);
            self.agent.connection = Connection::Working;
        }
        cx.notify();
    }

    pub(super) fn stop_agent(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = &self.agent.session {
            session.cancel();
        }
        cx.notify();
    }

    pub(super) fn answer_permission(
        &mut self,
        request: u64,
        choice: Option<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = &self.agent.session else {
            return;
        };
        let label = choice
            .as_ref()
            .map_or_else(|| "Rejected".to_string(), |(_, name)| name.clone());
        session.answer(request, choice.map(|(id, _)| id));
        self.agent.conversation.choose(request, label);
        cx.notify();
    }

    pub(super) fn copy_answer(&mut self, cx: &mut Context<Self>) {
        if let Some(answer) = self.agent.conversation.last_answer() {
            cx.write_to_clipboard(ClipboardItem::new_string(answer.to_string()));
        }
    }

    pub(super) fn agent_key(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) -> bool {
        match self.agent.composer.handle(keystroke) {
            FieldKey::Ignored => return false,
            FieldKey::Submit => {
                let question = self.agent.composer.text.trim().to_string();
                if !question.is_empty() && self.agent.ready() && self.tabs.current().is_some() {
                    self.agent.composer.clear();
                    self.ask_agent(Some(question), cx);
                }
            }
            FieldKey::Cancel => self.chrome.input = Input::None,
            _ => {}
        }
        cx.notify();
        true
    }
}
