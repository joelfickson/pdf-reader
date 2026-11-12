use super::{CheckForUpdates, Reader, chrome::Overlay};
use crate::ui::kit::Status;
use crate::update::{self, Build, Channel, Outcome, Progress, Release, Settings};
use gpui::{AppContext, Context, Window};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub(super) const UPDATES_SECTION: usize = 4;
pub(super) const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(10);

pub(super) enum Phase {
    Unavailable(&'static str),
    Idle,
    Checking,
    Current {
        checked: Instant,
    },
    Downloading {
        version: String,
        received: u64,
        total: u64,
    },
    Ready {
        release: Box<Release>,
        staged: PathBuf,
    },
    Installing {
        version: String,
    },
    Failed {
        message: String,
        checked: Instant,
    },
}

enum Message {
    Progress(Progress),
    Done(anyhow::Result<Outcome>),
}

pub(super) struct Updates {
    build: Result<Build, &'static str>,
    store: Option<PathBuf>,
    pub settings: Settings,
    pub phase: Phase,
    pub dismissed: bool,
    running: bool,
}

impl Updates {
    pub fn load() -> (Self, Option<String>) {
        let store = crate::recent::data_dir();
        let build = update::build(store.clone());
        let prerelease = build
            .as_ref()
            .is_ok_and(|build| build.version.is_prerelease());
        let defaults = Settings::defaults(prerelease);
        let (settings, error) =
            match Settings::load(store.clone().filter(|_| build.is_ok()), defaults) {
                Ok(settings) => (settings, None),
                Err(error) => (defaults, Some(format!("{error:#}"))),
            };
        let phase = match &build {
            Ok(_) => Phase::Idle,
            Err(reason) => Phase::Unavailable(reason),
        };
        (
            Self {
                build,
                store,
                settings,
                phase,
                dismissed: false,
                running: false,
            },
            error,
        )
    }

    pub fn ready(&self) -> Option<&Release> {
        match &self.phase {
            Phase::Ready { release, .. } => Some(release),
            _ => None,
        }
    }

    pub fn shows_control(&self) -> bool {
        matches!(self.phase, Phase::Installing { .. })
            || (self.ready().is_some() && !self.dismissed)
    }

    pub fn install_on_quit(&mut self) {
        let (Ok(build), Phase::Ready { staged, .. }) = (&self.build, &self.phase) else {
            return;
        };
        if let Err(error) = update::install(build, staged, None) {
            eprintln!("Agentic PDF could not install the downloaded update. {error:#}");
        }
    }
}

impl Reader {
    pub(super) fn start_update_schedule(&mut self, cx: &mut Context<Self>) {
        if self.updates.build.is_err() {
            return;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FIRST_CHECK_DELAY).await;
            cx.background_spawn(async {
                if let Err(error) = update::clean_leftovers() {
                    eprintln!(
                        "Agentic PDF could not remove files left by an earlier update. {error:#}"
                    );
                }
            })
            .detach();
            loop {
                let alive = this.update(cx, |this, cx| {
                    let waiting = this.updates.ready().is_some()
                        || matches!(this.updates.phase, Phase::Installing { .. });
                    if this.updates.settings.automatic && !waiting {
                        this.check_for_updates_now(cx);
                    }
                });
                if alive.is_err() {
                    return;
                }
                cx.background_executor().timer(CHECK_INTERVAL).await;
            }
        })
        .detach();
    }

    pub(super) fn check_for_updates_now(&mut self, cx: &mut Context<Self>) {
        let Ok(build) = self.updates.build.clone() else {
            return;
        };
        if self.updates.running || matches!(self.updates.phase, Phase::Installing { .. }) {
            return;
        }
        self.updates.running = true;
        if self.updates.ready().is_none() {
            self.updates.phase = Phase::Checking;
        }
        let channel = self.updates.settings.channel;
        let (sender, receiver) = async_channel::unbounded();
        let spawned = std::thread::Builder::new()
            .name("updates".into())
            .spawn(move || {
                let outcome = update::check(&build, channel, |progress| {
                    let _ = sender.send_blocking(Message::Progress(progress));
                });
                let _ = sender.send_blocking(Message::Done(outcome));
            });
        if let Err(error) = spawned {
            self.finish_check(Err(error.into()), cx);
            return;
        }
        cx.spawn(async move |this, cx| {
            while let Ok(message) = receiver.recv().await {
                let delivered = this.update(cx, |this, cx| match message {
                    Message::Progress(progress) => {
                        this.updates.phase = Phase::Downloading {
                            version: progress.version.to_string(),
                            received: progress.received,
                            total: progress.total,
                        };
                        cx.notify();
                    }
                    Message::Done(outcome) => this.finish_check(outcome, cx),
                });
                if delivered.is_err() {
                    return;
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn finish_check(&mut self, outcome: anyhow::Result<Outcome>, cx: &mut Context<Self>) {
        self.updates.running = false;
        let checked = Instant::now();
        match outcome {
            Ok(Outcome::Current) => self.updates.phase = Phase::Current { checked },
            Ok(Outcome::Ready { release, staged }) => {
                self.updates.phase = Phase::Ready { release, staged }
            }
            Err(_) if self.updates.ready().is_some() => {}
            Err(error) => {
                self.updates.phase = Phase::Failed {
                    message: format!("{error:#}"),
                    checked,
                }
            }
        }
        cx.notify();
    }

    pub(super) fn check_for_updates(
        &mut self,
        _: &CheckForUpdates,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.chrome.settings_section = UPDATES_SECTION;
        if self.chrome.overlay != Overlay::Settings {
            self.set_overlay(Overlay::Settings, cx);
        }
        self.check_for_updates_now(cx);
    }

    pub(super) fn set_automatic_updates(&mut self, automatic: bool, cx: &mut Context<Self>) {
        self.updates.settings.automatic = automatic;
        self.save_update_settings(cx);
        if automatic && matches!(self.updates.phase, Phase::Idle) {
            self.check_for_updates_now(cx);
        }
    }

    pub(super) fn set_update_channel(&mut self, channel: Channel, cx: &mut Context<Self>) {
        if self.updates.settings.channel == channel {
            return;
        }
        self.updates.settings.channel = channel;
        self.save_update_settings(cx);
    }

    fn save_update_settings(&mut self, cx: &mut Context<Self>) {
        let store = self
            .updates
            .store
            .clone()
            .filter(|_| self.updates.build.is_ok());
        if let Err(error) = self.updates.settings.save(store) {
            self.show_toast(
                Status::Warn,
                "Update settings could not be saved",
                &format!("{error:#}"),
                cx,
            );
        }
        cx.notify();
    }

    pub(super) fn dismiss_update(&mut self, cx: &mut Context<Self>) {
        self.updates.dismissed = true;
        if self.chrome.overlay == Overlay::Update {
            self.set_overlay(Overlay::None, cx);
        }
        cx.notify();
    }

    pub(super) fn restart_to_update(&mut self, cx: &mut Context<Self>) {
        let (Ok(build), Phase::Ready { release, staged }) =
            (&self.updates.build, &self.updates.phase)
        else {
            return;
        };
        let build = build.clone();
        let staged = staged.clone();
        let version = release.version.to_string();
        self.persist_recent(cx);
        let documents: Vec<PathBuf> = self
            .tabs
            .entries
            .iter()
            .map(|tab| tab.path.clone())
            .collect();
        self.updates.phase = Phase::Installing {
            version: version.clone(),
        };
        cx.notify();
        let task =
            cx.background_spawn(async move { update::install(&build, &staged, Some(&documents)) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(()) => cx.quit(),
                Err(error) => {
                    this.updates.phase = Phase::Failed {
                        message: format!("Agentic PDF {version} could not be installed. {error:#}"),
                        checked: Instant::now(),
                    };
                    this.chrome.settings_section = UPDATES_SECTION;
                    this.set_overlay(Overlay::Settings, cx);
                }
            });
        })
        .detach();
    }

    pub(super) fn open_release_notes(&mut self, cx: &mut Context<Self>) {
        if let Some(release) = self.updates.ready() {
            cx.open_url(&release.page_url);
        }
    }
}

pub(super) fn elapsed(since: Instant) -> String {
    let minutes = since.elapsed().as_secs() / 60;
    match minutes {
        0 => "Checked just now".into(),
        1 => "Checked 1 minute ago".into(),
        2..60 => format!("Checked {minutes} minutes ago"),
        60..120 => "Checked 1 hour ago".into(),
        _ => format!("Checked {} hours ago", minutes / 60),
    }
}

pub(super) fn megabytes(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_000_000.0)
}
