use super::conversation::{
    Choice, ChoiceKind, PlanStep, Status, ToolRow, Update, tool_detail, tool_label,
};
use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::v1::ConnectionTo;
use agent_client_protocol::v1::schema::{
    CancelNotification, ContentBlock, InitializeRequest, McpServer, McpServerStdio,
    NewSessionRequest, PermissionOptionKind, PlanEntryStatus, PromptRequest,
    RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse, ResourceLink,
    SelectedPermissionOutcome, SessionNotification, SessionUpdate, TextContent, ToolCallStatus,
};
use agent_client_protocol::{AcpAgent, AcpAgentConfig, Agent};
use anyhow::{Context, Result};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::oneshot;

pub struct Launch {
    pub args: Vec<String>,
    pub path: OsString,
    pub cwd: PathBuf,
    pub mcp: PathBuf,
}

pub struct Prompt {
    pub text: String,
    pub file: Option<(String, String)>,
}

pub enum Event {
    Connected(String),
    Update(Update),
    Permission {
        request: u64,
        title: String,
        choices: Vec<Choice>,
    },
    TurnEnded(String),
    Failed(String),
}

enum Command {
    Prompt(Prompt),
    Cancel,
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Option<String>>>>>;

pub struct Session {
    commands: async_channel::Sender<Command>,
    pending: Pending,
}

impl Session {
    pub fn start(launch: Launch) -> Result<(Self, async_channel::Receiver<Event>)> {
        let (commands, inbox) = async_channel::unbounded();
        let (events, receiver) = async_channel::unbounded();
        let pending: Pending = Arc::default();
        let shared = pending.clone();
        std::thread::Builder::new()
            .name("agent-session".into())
            .spawn(move || {
                let failure = events.clone();
                let result = tokio::runtime::Builder::new_current_thread()
                    .enable_time()
                    .build()
                    .context("The agent runtime could not start")
                    .and_then(|runtime| runtime.block_on(run(launch, inbox, events, shared)));
                if let Err(error) = result {
                    let _ = failure.send_blocking(Event::Failed(format!("{error:#}")));
                }
            })
            .context("The agent thread could not start")?;
        Ok((Self { commands, pending }, receiver))
    }

    pub fn prompt(&self, prompt: Prompt) -> bool {
        self.commands.try_send(Command::Prompt(prompt)).is_ok()
    }

    pub fn cancel(&self) {
        let _ = self.commands.try_send(Command::Cancel);
        for (_, reply) in self.pending.lock().drain() {
            let _ = reply.send(None);
        }
    }

    pub fn answer(&self, request: u64, choice: Option<String>) {
        if let Some(reply) = self.pending.lock().remove(&request) {
            let _ = reply.send(choice);
        }
    }
}

fn status(status: ToolCallStatus) -> Status {
    match status {
        ToolCallStatus::Pending => Status::Pending,
        ToolCallStatus::InProgress => Status::Running,
        ToolCallStatus::Completed => Status::Done,
        ToolCallStatus::Failed => Status::Failed,
        _ => Status::Running,
    }
}

fn translate(update: SessionUpdate) -> Option<Update> {
    match update {
        SessionUpdate::AgentMessageChunk(chunk) => match chunk.content {
            ContentBlock::Text(text) => Some(Update::Text(text.text)),
            _ => None,
        },
        SessionUpdate::Plan(plan) => Some(Update::Plan(
            plan.entries
                .into_iter()
                .map(|entry| PlanStep {
                    text: entry.content,
                    status: match entry.status {
                        PlanEntryStatus::Completed => Status::Done,
                        PlanEntryStatus::InProgress => Status::Running,
                        _ => Status::Pending,
                    },
                })
                .collect(),
        )),
        SessionUpdate::ToolCall(call) => Some(Update::ToolStarted(ToolRow {
            id: call.tool_call_id.to_string(),
            title: tool_label(&call.title),
            detail: call
                .raw_input
                .as_ref()
                .and_then(tool_detail)
                .unwrap_or_default(),
            status: status(call.status),
        })),
        SessionUpdate::ToolCallUpdate(update) => Some(Update::ToolChanged {
            id: update.tool_call_id.to_string(),
            title: update.fields.title.as_deref().map(tool_label),
            detail: update.fields.raw_input.as_ref().and_then(tool_detail),
            status: update.fields.status.map(status),
        }),
        _ => None,
    }
}

fn choices(request: &RequestPermissionRequest) -> Vec<Choice> {
    request
        .options
        .iter()
        .map(|option| Choice {
            id: option.option_id.to_string(),
            name: option.name.clone(),
            kind: match option.kind {
                PermissionOptionKind::AllowOnce => ChoiceKind::AllowOnce,
                PermissionOptionKind::AllowAlways => ChoiceKind::AllowAlways,
                PermissionOptionKind::RejectAlways => ChoiceKind::RejectAlways,
                _ => ChoiceKind::RejectOnce,
            },
        })
        .collect()
}

async fn run(
    launch: Launch,
    inbox: async_channel::Receiver<Command>,
    events: async_channel::Sender<Event>,
    pending: Pending,
) -> Result<()> {
    let mut program = launch.args.iter();
    let command = program.next().context("No agent command was given")?;
    let agent = AcpAgent::new(
        AcpAgentConfig::new(command)
            .args(program.cloned())
            .env("PATH", launch.path.to_string_lossy().into_owned()),
    );
    let updates = events.clone();
    let asks = events.clone();
    let counter = Arc::new(AtomicU64::new(0));
    agent_client_protocol::Client
        .v1()
        .name("pdf-reader")
        .on_receive_notification(
            async move |notification: SessionNotification, _cx| {
                if let Some(update) = translate(notification.update) {
                    let _ = updates.send(Event::Update(update)).await;
                }
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_request(
            async move |request: RequestPermissionRequest, responder, _connection| {
                let id = counter.fetch_add(1, Ordering::SeqCst) + 1;
                let (reply, chosen) = oneshot::channel();
                pending.lock().insert(id, reply);
                let title = request
                    .tool_call
                    .fields
                    .title
                    .as_deref()
                    .map(tool_label)
                    .unwrap_or_else(|| "Use a tool".into());
                let _ = asks
                    .send(Event::Permission {
                        request: id,
                        title,
                        choices: choices(&request),
                    })
                    .await;
                let outcome = match chosen.await.ok().flatten() {
                    Some(option) => {
                        RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option))
                    }
                    None => RequestPermissionOutcome::Cancelled,
                };
                responder.respond(RequestPermissionResponse::new(outcome))
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_with(agent, async move |connection: ConnectionTo<Agent>| {
            let ready = connection
                .send_request(InitializeRequest::new(ProtocolVersion::V1))
                .block_task()
                .await?;
            let name = ready
                .agent_info
                .map(|info| info.title.unwrap_or(info.name))
                .unwrap_or_else(|| "Agent".into());
            let server = McpServer::Stdio(
                McpServerStdio::new("pdf-reader", launch.mcp.clone()).args(vec!["mcp".into()]),
            );
            let session = connection
                .send_request(NewSessionRequest::new(launch.cwd.clone()).mcp_servers(vec![server]))
                .block_task()
                .await?
                .session_id;
            let _ = events.send(Event::Connected(name)).await;
            while let Ok(command) = inbox.recv().await {
                let Command::Prompt(prompt) = command else {
                    continue;
                };
                let mut content = vec![ContentBlock::Text(TextContent::new(prompt.text))];
                if let Some((name, uri)) = prompt.file {
                    content.push(ContentBlock::ResourceLink(ResourceLink::new(name, uri)));
                }
                let request = connection
                    .send_request(PromptRequest::new(session.clone(), content))
                    .block_task();
                tokio::pin!(request);
                let outcome = loop {
                    tokio::select! {
                        result = &mut request => break result,
                        command = inbox.recv() => match command {
                            Ok(Command::Cancel) => {
                                let _ = connection
                                    .send_notification(CancelNotification::new(session.clone()));
                            }
                            Ok(Command::Prompt(_)) => {}
                            Err(_) => return Ok(()),
                        },
                    }
                };
                let message = match outcome {
                    Ok(response) => format!("{:?}", response.stop_reason),
                    Err(error) => format!("Failed: {error}"),
                };
                let _ = events.send(Event::TurnEnded(message)).await;
            }
            Ok(())
        })
        .await
        .map_err(|error| anyhow::anyhow!("{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn next(events: &async_channel::Receiver<Event>) -> Event {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            match events.try_recv() {
                Ok(event) => return event,
                Err(async_channel::TryRecvError::Closed) => panic!("Agent session closed"),
                Err(async_channel::TryRecvError::Empty) => {
                    assert!(Instant::now() < deadline, "Agent session timed out");
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }

    #[test]
    fn runs_a_prompt_turn_against_a_scripted_agent() {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let python = ["python3", "python"]
            .iter()
            .find_map(|name| super::super::catalog::find(name, &path))
            .expect("Python is required for the scripted ACP agent");
        let script =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake_acp_agent.py");
        let (session, events) = Session::start(Launch {
            args: vec![
                python.to_string_lossy().into_owned(),
                script.to_string_lossy().into_owned(),
            ],
            path,
            cwd: std::env::temp_dir(),
            mcp: PathBuf::from("/opt/reader/pdf-reader"),
        })
        .unwrap();
        assert!(matches!(next(&events), Event::Connected(name) if name == "Fake Agent"));
        assert!(session.prompt(Prompt {
            text: "Summarize".into(),
            file: Some(("a.pdf".into(), "file:///tmp/a.pdf".into())),
        }));
        assert!(matches!(
            next(&events),
            Event::Update(Update::Plan(steps)) if steps[0].status == Status::Running
        ));
        assert!(matches!(
            next(&events),
            Event::Update(Update::ToolStarted(row))
                if row.title == "Read text" && row.detail == "From page 1" && row.status == Status::Pending
        ));
        assert!(matches!(
            next(&events),
            Event::Update(Update::ToolChanged {
                status: Some(Status::Done),
                ..
            })
        ));
        let Event::Permission {
            request,
            title,
            choices,
        } = next(&events)
        else {
            panic!("permission request expected")
        };
        assert_eq!(title, "highlight");
        assert_eq!(choices[0].kind, ChoiceKind::AllowOnce);
        assert_eq!(choices[1].kind, ChoiceKind::RejectOnce);
        session.answer(request, Some(choices[0].id.clone()));
        let mut text = String::new();
        let stop = loop {
            match next(&events) {
                Event::Update(Update::Text(chunk)) => text.push_str(&chunk),
                Event::TurnEnded(stop) => break stop,
                Event::Failed(message) => panic!("{message}"),
                _ => {}
            }
        };
        assert_eq!(
            text,
            "Chose yes for file:///tmp/a.pdf via pdf-reader /opt/reader/pdf-reader mcp (p. 1)."
        );
        assert_eq!(stop, "EndTurn");
    }
}

#[cfg(all(test, feature = "live-agent"))]
mod live {
    use super::*;
    use crate::agent::catalog::{self, Availability};
    use crate::agent::conversation::{Conversation, file_uri, summary_prompt};
    use std::time::{Duration, Instant};

    #[test]
    fn summarizes_a_fixture_with_the_first_installed_agent() {
        let path = catalog::search_path();
        let agent = catalog::detect(&path)
            .into_iter()
            .find(|agent| matches!(agent.availability, Availability::Ready(_)))
            .expect("an installed ACP agent");
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let pdf = root.join("tests/fixtures/outline.pdf");
        let started = Instant::now();
        let (session, events) = Session::start(Launch {
            args: agent.launch_args().unwrap(),
            path,
            cwd: root.join("tests/fixtures"),
            mcp: root.join("target/debug/pdf-reader"),
        })
        .unwrap();
        let mut conversation = Conversation::default();
        let deadline = started + Duration::from_secs(240);
        loop {
            let event = loop {
                match events.try_recv() {
                    Ok(event) => break event,
                    Err(async_channel::TryRecvError::Empty) => {
                        assert!(Instant::now() < deadline, "the agent timed out");
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    Err(error) => panic!("{error}"),
                }
            };
            match event {
                Event::Connected(name) => {
                    println!(
                        "connected to {name} after {} ms",
                        started.elapsed().as_millis()
                    );
                    assert!(session.prompt(Prompt {
                        text: summary_prompt(&pdf),
                        file: Some(("outline.pdf".into(), file_uri(&pdf))),
                    }));
                }
                Event::Update(update) => conversation.apply(update),
                Event::Permission {
                    request, choices, ..
                } => session.answer(request, choices.first().map(|choice| choice.id.clone())),
                Event::TurnEnded(stop) => {
                    conversation.finish_turn();
                    println!(
                        "turn ended ({stop}) after {} ms",
                        started.elapsed().as_millis()
                    );
                    break;
                }
                Event::Failed(message) => panic!("{message}"),
            }
        }
        for entry in &conversation.entries {
            println!("{entry:?}");
        }
        let answer = conversation.last_answer().expect("an answer");
        assert!(!crate::agent::conversation::page_references(answer).is_empty());
    }
}
