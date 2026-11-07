use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub struct AgentSpec {
    pub name: &'static str,
    pub detect: &'static str,
    pub launch: &'static [&'static str],
}

pub const AGENTS: [AgentSpec; 3] = [
    AgentSpec {
        name: "Claude Code",
        detect: "claude",
        launch: &["npx", "-y", "@agentclientprotocol/claude-agent-acp@0.101.0"],
    },
    AgentSpec {
        name: "Codex",
        detect: "codex",
        launch: &["npx", "-y", "@zed-industries/codex-acp@0.16.0"],
    },
    AgentSpec {
        name: "Gemini CLI",
        detect: "gemini",
        launch: &["gemini", "--acp"],
    },
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Availability {
    Ready(PathBuf),
    NotInstalled,
    NeedsNode,
}

#[derive(Clone, Debug)]
pub struct Detected {
    pub spec: &'static AgentSpec,
    pub availability: Availability,
}

impl Detected {
    pub fn launch_args(&self) -> Option<Vec<String>> {
        let Availability::Ready(program) = &self.availability else {
            return None;
        };
        Some(
            std::iter::once(program.to_string_lossy().into_owned())
                .chain(self.spec.launch[1..].iter().map(|arg| (*arg).to_string()))
                .collect(),
        )
    }
}

pub fn detect(path: &OsStr) -> Vec<Detected> {
    AGENTS
        .iter()
        .map(|spec| {
            let availability = match (find(spec.detect, path), find(spec.launch[0], path)) {
                (None, _) => Availability::NotInstalled,
                (Some(_), None) => Availability::NeedsNode,
                (Some(_), Some(program)) => Availability::Ready(program),
            };
            Detected { spec, availability }
        })
        .collect()
}

pub fn find(program: &str, path: &OsStr) -> Option<PathBuf> {
    let names: Vec<String> = if cfg!(windows) {
        ["exe", "cmd", "bat"]
            .iter()
            .map(|extension| format!("{program}.{extension}"))
            .collect()
    } else {
        vec![program.to_string()]
    };
    std::env::split_paths(path).find_map(|directory| {
        names
            .iter()
            .map(|name| directory.join(name))
            .find(|candidate| is_executable(candidate))
    })
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

pub fn search_path() -> OsString {
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    match login_path() {
        Some(login) if !login.is_empty() => {
            let mut directories: Vec<PathBuf> = std::env::split_paths(&login).collect();
            for directory in std::env::split_paths(&inherited) {
                if !directories.contains(&directory) {
                    directories.push(directory);
                }
            }
            std::env::join_paths(directories).unwrap_or(inherited)
        }
        _ => inherited,
    }
}

#[cfg(unix)]
fn login_path() -> Option<OsString> {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    const LOGIN_PATH_TIMEOUT: Duration = Duration::from_secs(3);
    let shell = std::env::var_os("SHELL").unwrap_or_else(|| "/bin/zsh".into());
    let mut child = Command::new(shell)
        .args(["-l", "-c", "printenv PATH"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if started.elapsed() > LOGIN_PATH_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    let mut output = String::new();
    std::io::Read::read_to_string(&mut child.stdout.take()?, &mut output).ok()?;
    let line = output.lines().last()?.trim();
    (!line.is_empty()).then(|| OsString::from(line))
}

#[cfg(not(unix))]
fn login_path() -> Option<OsString> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install(directory: &Path, name: &str) {
        let file = if cfg!(windows) {
            directory.join(format!("{name}.exe"))
        } else {
            directory.join(name)
        };
        std::fs::write(&file, "").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    #[test]
    fn detects_agents_and_their_launchers_on_the_path() {
        let root = std::env::temp_dir().join(format!("agent-catalog-{}", std::process::id()));
        let (tools, node) = (root.join("tools"), root.join("node"));
        std::fs::create_dir_all(&tools).unwrap();
        std::fs::create_dir_all(&node).unwrap();
        install(&tools, "claude");
        install(&tools, "gemini");
        std::fs::write(tools.join("codex"), "").unwrap();

        let without_node = std::env::join_paths([&tools]).unwrap();
        let found = detect(&without_node);
        assert_eq!(found[0].availability, Availability::NeedsNode);
        assert_eq!(
            found[2].availability,
            Availability::Ready(find("gemini", &without_node).unwrap())
        );
        if cfg!(unix) {
            assert_eq!(found[1].availability, Availability::NotInstalled);
        }

        install(&node, "npx");
        let with_node = std::env::join_paths([&tools, &node]).unwrap();
        let found = detect(&with_node);
        let launch = found[0].launch_args().unwrap();
        assert!(launch[0].ends_with(if cfg!(windows) { "npx.exe" } else { "npx" }));
        assert_eq!(
            launch[1..],
            ["-y", "@agentclientprotocol/claude-agent-acp@0.101.0"]
        );
        assert_eq!(found[2].launch_args().unwrap()[1..], ["--acp"]);
        std::fs::remove_dir_all(root).unwrap();
    }
}
